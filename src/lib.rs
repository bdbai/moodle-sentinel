mod check;
mod error;
mod migrations;
mod moodle;
mod subscribe;
mod tenant;
mod user;

use crate::check::start_check_loop;
use crate::subscribe::{add_subscribe, remove_group_subscribe, remove_subscribe};
use crate::tenant::Tenant;
use crate::user::get_user_id_from_qq;
use coolq_sdk_rust::api::{
    add_log, get_login_qq, send_group_msg, set_friend_add_request, set_group_add_request_v2,
    CQLogLevel,
};
use coolq_sdk_rust::events::{
    AddFriendRequestEvent, AddGroupRequestEvent, GroupMemberDecreaseEvent, GroupMessageEvent,
    PrivateMessageEvent,
};
use coolq_sdk_rust::prelude::listener;
use coolq_sdk_rust::targets::cqcode::CQCode;
use coolq_sdk_rust::targets::group::Group;
use lazy_static::lazy_static;
use rusqlite::Connection;
use tokio::sync::Mutex;

static DATA_PATH: &'static str = "data/app/com.bdbai.moodle-sentinel";
static DB_PATH: &'static str = "data/app/com.bdbai.moodle-sentinel/data.db";

lazy_static! {
    pub static ref CONN: Mutex<Connection> = {
        std::fs::create_dir_all(DATA_PATH).expect("Cannot create data dir");
        let mut conn = Connection::open(DB_PATH).expect("Cannot open or create db file");
        println!("Migrating...");
        migrations::runner()
            .run(&mut conn)
            .expect("Cannot run migration");
        Mutex::from(conn)
    };
    pub static ref MY_QQ: i64 = get_login_qq().expect("Cannot parse my QQ").into();
}

#[coolq_sdk_rust::main]
fn main() {
    add_log(CQLogLevel::INFOSUCCESS, "info", "Moodle Sentinel 正在加载").expect("日志发送失败");
    let conn = CONN.try_lock().expect("Cannot acquire db conn lock");
    drop(conn);
    add_log(CQLogLevel::INFOSUCCESS, "info", "数据库迁移完成").expect("日志发送失败");
    coolq_sdk_rust::ASYNC_RUNTIME.spawn(start_check_loop());
}

#[listener]
fn on_private_message(_event: PrivateMessageEvent) {
    ()
}

#[listener(priority = "low")]
async fn on_group_message(event: GroupMessageEvent) {
    let atme = event.msg.cqcodes.iter().any(|c| match c {
        CQCode::At(qq) => MY_QQ.eq(qq),
        _ => false,
    });
    if !atme {
        return;
    }
    let group_id = event.group.group_id;
    let user_id = match get_user_id_from_qq(event.user.user_id).await {
        Ok(i) => i,
        Err(error::Error::NotExist) => {
            send_group_msg(group_id, "别急 再等等").expect("无法发送消息");
            return;
        }
        Err(e) => {
            add_log(CQLogLevel::ERROR, "error", format!("无法读取用户 ID {}", e))
                .expect("无法写入日志");
            return;
        }
    };
    let mut params = event.msg.msg.split_ascii_whitespace();
    let command = match params.next() {
        Some(s) => s,
        None => return,
    };
    let param = match params.next().and_then(|p| p.parse().ok()) {
        Some(s) => s,
        None => return,
    };
    let tenant = Tenant::Group(group_id);
    let msg = match command {
        "订阅" => match add_subscribe(user_id, param, tenant).await {
            Ok(()) => Ok("已添加订阅"),
            Err(error::Error::Duplicated) => Ok("请不要重复订阅哦"),
            Err(err) => Err(err),
        },
        "退订" => match remove_subscribe(user_id, param, tenant).await {
            Ok(()) => Ok("已取消订阅"),
            Err(error::Error::NotExist) => Ok("没有订阅过呢"),
            Err(err) => Err(err),
        },
        _ => Ok("说啥呢 听不懂"),
    };
    send_group_msg(
        group_id,
        msg.map(|s| s.to_string())
            .unwrap_or_else(|e| format!("{}", e)),
    )
    .expect("无法回复群消息");
}

#[listener]
async fn group_member_decrease(event: GroupMemberDecreaseEvent) {
    if MY_QQ.eq(&event.being_operate_user.user_id) {
        // 被踢出群
        let Group {
            group_id,
            group_name,
            ..
        } = event.group;
        let removed_subscribe_count = remove_group_subscribe(group_id)
            .await
            .expect(format!("Cannot remove group {}", group_id).as_str());
        add_log(
            CQLogLevel::INFO,
            "subscribe",
            format!(
                "已退订 {}({}) 群内的 {} 个订阅",
                group_name, group_id, removed_subscribe_count
            ),
        )
        .expect("无法写入日志");
    }
}

#[listener]
fn add_friend_request(event: AddFriendRequestEvent) {
    set_friend_add_request(event.flag, true, "").expect("添加好友请求处理失败");
}

#[listener]
fn add_group_request(event: AddGroupRequestEvent) {
    if event.sub_type == 2 {
        set_group_add_request_v2(event.flag, event.sub_type, true, "").expect("无法受邀加入群");
    }
}

#[tokio::test]
async fn init_migrate() -> Result<(), error::Error> {
    if std::fs::metadata(DB_PATH).is_ok() {
        std::fs::remove_file(DB_PATH).expect("Cannot delete old db");
    }
    let conn = CONN.try_lock().expect("Cannot acquire db conn lock");
    let login_result = moodle::login(env!("CQMS_CAMPUS_ID"), env!("CQMS_CAMPUS_PASSWORD")).await?;
    conn.execute(
        "INSERT INTO `user` (`qq`, `nickname`, `moodle_token`)\
        VALUES (?1, ?2, ?3)",
        rusqlite::params![
            env!("CQMS_QQ").parse::<i64>().unwrap(),
            env!("CQMS_QQ_NAME"),
            login_result.token
        ],
    )?;
    drop(conn);
    println!("Migration done");
    Ok(())
}
