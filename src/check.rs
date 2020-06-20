use crate::error::Error;
use crate::moodle::{get_course_content, get_course_public_information, CourseModule, ModuleType};
use crate::tenant::Tenant;
use crate::CONN;
use chrono::{NaiveDateTime, Utc};
use coolq_sdk_rust::api::{add_log, send_group_msg, send_private_msg, CQLogLevel};
use futures::lock::Mutex;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use lazy_static::lazy_static;
use rusqlite::params;
use std::collections::HashMap;
use std::mem::replace;
use std::ops::Deref;
use std::sync::Arc;
use tokio::time::{delay_for, Duration};

#[derive(Debug)]
struct Notification<'a> {
    tenant: Tenant,
    user_qq: i64,
    course_name: String,
    modules: &'a Result<Vec<Update>, Error>,
}

pub async fn start_check_loop() {
    // Initial check
    // Avoid msg spam at startup
    match run_check(|_| {}).await {
        Ok(()) => {
            add_log(CQLogLevel::INFO, "check", "初始课程内容更新检查完成").expect("Cannot send log")
        }
        Err(e) => add_log(
            CQLogLevel::ERROR,
            "check",
            format!("初始课程内容更新检查失败：{:#?}", e),
        )
        .expect("Cannot send log"),
    };
    loop {
        // TODO: 时间间隔？
        delay_for(Duration::from_secs(60 * 5)).await;
        if let Err(e) = run_check(|update| {
            let msg = match update.modules.as_ref().map(|v| v.as_slice()) {
                Ok([]) => return,
                Ok([m]) => format!(
                    "{} 发布了一个{}{} {}，快去看看吧",
                    update.course_name,
                    if m.module.user_visible {
                        ""
                    } else {
                        "隐藏的"
                    },
                    match &m.module.content {
                        ModuleType::Mediasite => "视频",
                        ModuleType::Resource { .. } => "文件",
                        ModuleType::Url { .. } => "链接",
                        ModuleType::Folder { .. } => "文件夹",
                        ModuleType::Page { .. } => "页面",
                        ModuleType::Assignment => "作业",
                        ModuleType::Other => return,
                    },
                    if let ModuleType::Url { contents } = &m.module.content {
                        contents
                            .as_ref()
                            .and_then(|c| c.first())
                            .map(|c| c.name.as_str())
                            .unwrap_or(m.module.name.as_str())
                    } else {
                        m.module.name.as_str()
                    }
                ),
                Ok(n) => format!(
                    "{} 发布了 {} 个内容，快去看看吧",
                    update.course_name,
                    n.len()
                ),
                Err(e) => {
                    let _ = add_log(
                        CQLogLevel::ERROR,
                        "check",
                        format!("更新 {:#?} 出错：{:#?}", update, e),
                    );
                    format!(
                        "更新 {} 时出错了，将停止后续更新\n{}",
                        update.course_name, e
                    )
                }
            };
            match update.tenant {
                Tenant::Group(group_qq) => {
                    send_group_msg(group_qq, msg).expect("无法发送群消息");
                }
                Tenant::SenderSelf => {
                    send_private_msg(update.user_qq, msg).expect("无法发送消息");
                }
            }
        })
        .await
        {
            add_log(CQLogLevel::ERROR, "update", format!("无法更新，{:#?}", e))
                .expect("Cannot send cq log");
        }
    }
}

#[derive(Debug)]
struct GroupData {
    token: String,
    group_id: u32,
    course_id: u32,
    group_qq: i64,
    user_id: u32,
}

#[derive(Clone, Copy, Debug)]
enum UpdateType {
    Insert,
    Update(u32),
}

#[derive(Debug)]
struct Update {
    update_type: UpdateType,
    user_id: u32,
    course_id: u32,
    module: CourseModule,
}

async fn save_group_updates(updates: impl Iterator<Item = Update>) -> Result<(), Error> {
    let mut conn = CONN.lock().await;
    let tx = conn.transaction()?;
    let mut update_stmt =
        tx.prepare_cached("UPDATE `user_course_module` SET `updated_at` = ?1 WHERE `id` = ?2")?;
    let mut insert_stmt = tx.prepare_cached("INSERT INTO `user_course_module` (`user_id`, `course_id`, `module_id`, `updated_at`) VALUES (?1, ?2, ?3, ?4)")?;
    // TODO: check the last update timestamp from response
    let now = Utc::now().naive_utc();
    lazy_static! {
        static ref EXPIRATION: time::Duration = time::Duration::minutes(1);
    }
    for ((user_id, module_id), (update_type, course_id)) in updates
        // Dedup
        .map(|u| ((u.user_id, u.module.id), (u.update_type, u.course_id)))
        .collect::<HashMap<_, _>>()
        .into_iter()
    {
        match update_type {
            UpdateType::Insert => {
                insert_stmt.execute(params![user_id, course_id, module_id, now])?;
            }
            UpdateType::Update(record_id) => {
                update_stmt.execute(params![now, record_id])?;
            }
        }
    }
    drop(update_stmt);
    drop(insert_stmt);
    tx.commit()?;
    Ok(())
}

async fn run_check(mut on_new_message: impl FnMut(Notification)) -> Result<(), Error> {
    // TODO: Check self subscription
    let (grouped_updates, course_names, mut group_course_futures) = {
        let conn = CONN.lock().await;
        // TODO: pagination
        let mut stmt = conn.prepare_cached(
            "SELECT `u`.`moodle_token`, `g`.`id`, `g`.`course_id`, `g`.`group_qq`, `g`.`user_id`\
            FROM `user_course_group` AS 'g'\
            INNER JOIN `user` AS 'u' ON `u`.`id` = `g`.`user_id`\
            WHERE `g`.`failure_count` < 3",
        )?;
        let mut grouped_updates = HashMap::new();
        let mut course_names = HashMap::new();
        let group_course_futures: FuturesUnordered<_> = stmt
            .query_map(params![], |row| {
                let group_qq = row.get(3)?;
                let updates = grouped_updates
                    .entry(group_qq)
                    .or_insert(Arc::new(Mutex::new(Vec::new())))
                    .clone();
                let course_id: u32 = row.get(2)?;
                let course_name = course_names
                    .entry(course_id)
                    .or_insert(Arc::new(Mutex::new(None)))
                    .clone();
                Ok(check_group_course(
                    GroupData {
                        token: row.get(0)?,
                        group_id: row.get(1)?,
                        course_id,
                        group_qq,
                        user_id: row.get(4)?,
                    },
                    updates,
                    course_name,
                ))
            })?
            .collect::<Result<_, _>>()?;
        (grouped_updates, course_names, group_course_futures)
    };
    while let Some(()) = group_course_futures.next().await {}
    for (&group_qq, updates) in &grouped_updates {
        let updates = updates.lock().await;
        for (course_id, result) in &*updates {
            let course_name = course_names[course_id].try_lock().unwrap();
            let course_name = match course_name.deref() {
                Some(s) => s,
                None => continue,
            };
            on_new_message(Notification {
                tenant: Tenant::Group(group_qq),
                user_qq: 0,
                course_name: course_name.clone(),
                modules: result,
            });
        }
    }
    Ok(save_group_updates(
        grouped_updates
            .into_iter()
            .flat_map(|(_, course_update)| {
                replace(&mut *course_update.try_lock().unwrap(), Vec::new())
            })
            .map(|(_course_id, result)| result)
            .filter(|u| u.is_ok())
            .flat_map(|u| u.unwrap()),
    )
    .await?)
}
async fn check_group_course(
    group_data: GroupData,
    updates: Arc<Mutex<Vec<(u32, Result<Vec<Update>, Error>)>>>,
    course_name: Arc<Mutex<Option<String>>>,
) {
    let ret = try_check_group_course(&group_data, course_name).await;
    let mut updates = updates.lock().await;
    // Increase failure count
    let conn = CONN.lock().await;
    if let Err(e) = conn.execute(
        ret.as_ref()
            .map(|_| {
                "UPDATE `user_course_group` SET `failure_count` = 0 \
                WHERE `group_qq` = ?1 AND `course_id` = ?2"
            })
            .unwrap_or(
                "UPDATE `user_course_group` SET `failure_count` = `failure_count` + 1 \
                WHERE `group_qq` = ?1 AND `course_id` = ?2",
            ),
        params![group_data.group_qq, group_data.course_id],
    ) {
        dbg!(&e);
        add_log(
            CQLogLevel::ERROR,
            "inc_fail_cnt",
            format!("无法处理失败次数，{:#?}", e),
        )
        .expect("Cannot send cq msg");
    }
    updates.push((group_data.course_id, ret));
}

async fn try_check_group_course(
    group_data: &GroupData,
    course_name: Arc<Mutex<Option<String>>>,
) -> Result<Vec<Update>, Error> {
    let course_id = group_data.course_id;
    // Try to get the course name
    if let Some(mut c) = course_name.try_lock() {
        if c.is_none() {
            // Get course name
            match get_course_public_information(group_data.token.as_str(), course_id).await {
                Ok(mut info) => {
                    *c = Some(
                        info.courses
                            .pop()
                            .map(|c| c.full_name)
                            .unwrap_or(format!("未知课程 {}", course_id)),
                    );
                }
                Err(e) => {
                    // Must assign a course name otherwise error notification
                    // will be muted
                    *c = Some(format!("未知课程 {}", course_id));
                    add_log(
                        CQLogLevel::ERROR,
                        "course_name",
                        format!("获取课程名称错误 {:#?}", e),
                    )
                    .expect("Cannot send course_name error message to cq");
                }
            }
        }
    }
    // else if we cannot acquire course_name lock, somewhere else is fetching
    // the course name

    // Get course modules to check updates
    let modules = get_course_content(group_data.token.as_str(), course_id)
        .await?
        .into_iter()
        .flat_map(|s| s.modules);
    let conn = CONN.lock().await;
    let module_records: HashMap<u32, (u32, NaiveDateTime)> = conn
        .prepare_cached(
            "SELECT `id`, `module_id`, `updated_at` FROM `user_course_module`\
                WHERE `user_id` = ?1 AND `course_id` = ?2",
        )?
        .query_map(params![group_data.user_id, course_id], |row| {
            Ok((row.get(1)?, (row.get(0)?, row.get(2)?)))
        })?
        .collect::<Result<_, _>>()?;
    Ok(modules
        // TODO: check the last update timestamp from response
        .filter(|m| !module_records.contains_key(&m.id))
        .map(|m| Update {
            update_type: if module_records.contains_key(&m.id) {
                UpdateType::Update(m.id)
            } else {
                UpdateType::Insert
            },
            user_id: group_data.user_id,
            course_id,
            module: m,
        })
        .collect())
}

#[tokio::test]
async fn run_check_test() {
    run_check(|u| println!("{:#?}", u)).await.unwrap();
}
