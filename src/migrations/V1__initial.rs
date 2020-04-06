use barrel::{backend::Sqlite, types, Migration};

pub fn migration() -> String {
    let mut m = Migration::new();

    m.create_table("user", |t| {
        t.add_column("id", types::integer().primary(true));
        t.add_column("qq", types::integer().unique(true));
        t.add_column("nickname", types::text());
        t.add_column("moodle_token", types::varchar(63));
    });

    m.create_table("user_course_module", |t| {
        t.add_column("id", types::integer().primary(true));
        t.add_column("user_id", types::integer().indexed(true));
        t.add_column("course_id", types::integer().indexed(true));
        t.add_column("module_id", types::integer());
        t.add_column("updated_at", types::date());
    });

    m.create_table("user_course_group", |t| {
        t.add_column("id", types::integer().primary(true));
        t.add_column("user_id", types::integer().indexed(true));
        t.add_column("course_id", types::integer().indexed(true));
        t.add_column("group_qq", types::integer().indexed(true));
        t.add_column("failure_count", types::integer().default(0));
        t.add_column("created_at", types::date());
    });

    m.create_table("user_course_self", |t| {
        t.add_column("id", types::integer().primary(true));
        t.add_column("user_id", types::integer().indexed(true));
        t.add_column("course_id", types::integer());
        t.add_column("created_at", types::date());
    });

    m.make::<Sqlite>()
}
