use crate::error::Error;
use crate::CONN;
use rusqlite::Connection;

pub async fn get_user_id_from_qq(qq: i64) -> Result<u32, Error> {
    let conn = CONN.lock().await;
    let mut stmt = conn
        .prepare_cached("SELECT `id` FROM `user` WHERE `qq` = ?1")
        .unwrap();
    Ok(stmt.query_row(&[qq], |row| Ok(row.get(0)?))?)
}

pub fn get_user_moodle_token(conn: &Connection, user_id: u32) -> Result<String, Error> {
    let mut stmt = conn
        .prepare_cached("SELECT `moodle_token` FROM `user` WHERE `id` = ?1")
        .unwrap();
    Ok(stmt.query_row(&[user_id], |row| Ok(row.get(0)?))?)
    // TODO: refresh token
}
