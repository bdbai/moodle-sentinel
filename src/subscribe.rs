use crate::error::Error;
use crate::moodle::{get_course_content, CourseSection};
use crate::tenant::Tenant;
use crate::user::get_user_moodle_token;
use crate::CONN;
use chrono::Utc;
use rusqlite::{params, Error as DbError, OptionalExtension, Row, Transaction};

fn save_course_modules(
    tx: &Transaction,
    user_id: u32,
    course_id: u32,
    course_content: &Vec<CourseSection>,
) -> Result<(), Error> {
    let mut stmt = tx.prepare(
        "INSERT INTO `user_course_module`\
        (`user_id`, `course_id`, `module_id`, `updated_at`)\
        VALUES (?1, ?2, ?3, ?4)",
    )?;
    // Use current UTC time instead of lastupdate from module info
    // to avoid inconsistency
    let updated_at = Utc::now().naive_utc();
    for section in course_content {
        for module in &section.modules {
            let module_id = match module.get_id() {
                Some(id) => id,
                None => continue,
            };
            stmt.execute(params![user_id, course_id, module_id, updated_at])?;
        }
    }
    Ok(())
}

pub async fn add_subscribe(user_id: u32, course_id: u32, tenant: Tenant) -> Result<(), Error> {
    // TODO: renew moodle token
    let mut conn = CONN.lock().await;

    static ROW_MATCHER: fn(&Row) -> Result<u32, DbError> = |row: &Row| Ok(row.get(0)?);
    let existing = match tenant {
        Tenant::SenderSelf => conn.query_row(
            "SELECT `id` FROM `user_course_self`\
            WHERE `user_id` = ?1 AND `course_id` = ?2 LIMIT 1",
            params![user_id, course_id],
            ROW_MATCHER,
        ),
        Tenant::Group(group_qq) => conn.query_row(
            "SELECT `id` FROM `user_course_group`\
            WHERE `group_qq` = ?1 AND `course_id` = ?2 LIMIT 1",
            params![group_qq, course_id],
            ROW_MATCHER,
        ),
    }
    .optional()?;
    if let Some(_) = existing {
        return Err(Error::Duplicated);
    }
    // Still holding the lock to avoid races
    // Check user token
    let token = get_user_moodle_token(&conn, user_id)?;
    // Check if user can get course content
    let course_content = get_course_content(token, course_id).await?;

    let tx = conn.transaction()?;
    save_course_modules(&tx, user_id, course_id, &course_content)?;

    // Save user-course
    let affected = match tenant {
        Tenant::SenderSelf => tx.execute(
            "INSERT INTO `user_course_self` (`user_id`, `course_id`, `created_at`) VALUES (?1, ?2, ?3)",
            params![user_id, course_id, Utc::now().naive_utc()]
        ),
        Tenant::Group(group_qq) => tx.execute(
            "INSERT INTO `user_course_group` (`user_id`, `course_id`, `group_qq`, `created_at`) VALUES (?1, ?2, ?3, ?4)",
            params![user_id, course_id, group_qq, Utc::now().naive_utc()]
        )
    }?;
    if affected == 0 {
        return Err(Error::Other("无法添加记录".to_string()));
    }
    tx.commit()?;
    Ok(())
}

pub async fn remove_subscribe(user_id: u32, course_id: u32, tenant: Tenant) -> Result<(), Error> {
    let conn = CONN.lock().await;

    let affected = match tenant {
        Tenant::SenderSelf => conn.execute(
            "DELETE FROM `user_course_self` WHERE `user_id` = ?1 AND `course_id` = ?2",
            params![user_id, course_id]
        ),
        Tenant::Group(group_qq) => conn.execute(
            "DELETE FROM `user_course_group` WHERE `user_id` = ?1 AND `course_id` = ?2 AND `group_qq` = ?3",
            params![user_id, course_id, group_qq]
        )
    }?;

    match affected {
        1 => Ok(()),
        _ => Err(Error::NotExist),
    }
}

#[tokio::test]
async fn test_add_remove_self_subscribe() -> Result<(), Error> {
    let conn = CONN.lock().await;
    conn.execute("DELETE FROM `user_course_module`", params![])?;
    conn.execute("DELETE FROM `user_course_self`", params![])?;
    drop(conn);

    let tenant = Tenant::SenderSelf;
    let user_id = crate::user::get_user_id_from_qq(env!("CQMS_QQ").parse().unwrap())
        .await
        .unwrap();
    let course_id = env!("CQMS_COURSE_ID").parse::<u32>().unwrap();
    add_subscribe(user_id, course_id, tenant).await?;
    println!("Added course to self");
    if !matches!(
        add_subscribe(user_id, course_id, tenant).await.unwrap_err(),
        Error::Duplicated
    ) {
        panic!("Duplicated add does not throw error");
    }
    remove_subscribe(user_id, course_id, tenant).await?;
    println!("Removed course to self");
    add_subscribe(user_id, course_id, tenant).await?;
    println!("Added course to self again");
    remove_subscribe(user_id, course_id, tenant).await?;
    println!("Removed course to self again");
    if !matches!(
        remove_subscribe(user_id, course_id, tenant)
            .await
            .unwrap_err(),
        Error::NotExist
    ) {
        panic!("Remote non-exist record does not throw");
    }
    Ok(())
}

#[tokio::test]
async fn test_add_remove_group_subscribe() -> Result<(), Error> {
    let conn = CONN.lock().await;
    conn.execute("DELETE FROM `user_course_module`", params![])?;
    conn.execute("DELETE FROM `user_course_group`", params![])?;
    drop(conn);

    let user_id = crate::user::get_user_id_from_qq(env!("CQMS_QQ").parse().unwrap())
        .await
        .unwrap();
    let tenant = Tenant::Group(env!("CQMS_QQ_GROUP").parse().unwrap());
    let course_id = env!("CQMS_COURSE_ID").parse::<u32>().unwrap();
    add_subscribe(user_id, course_id, tenant).await?;
    println!("Added course to group");
    if !matches!(
        add_subscribe(user_id, course_id, tenant).await.unwrap_err(),
        Error::Duplicated
    ) {
        panic!("Duplicated add does not throw error");
    }
    remove_subscribe(user_id, course_id, tenant).await?;
    println!("Removed course to group");
    add_subscribe(user_id, course_id, tenant).await?;
    println!("Added course to group again");
    remove_subscribe(user_id, course_id, tenant).await?;
    println!("Removed course to group again");
    if !matches!(
        remove_subscribe(user_id, course_id, tenant)
            .await
            .unwrap_err(),
        Error::NotExist
    ) {
        panic!("Remote non-exist record does not throw");
    }
    Ok(())
}
