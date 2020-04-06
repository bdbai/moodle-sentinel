use crate::moodle;
use std::fmt::{self, Display, Formatter};

#[derive(Debug)]
pub enum Error {
    Duplicated,
    NotExist,
    Moodle(moodle::Error),
    Db(rusqlite::Error),
    Other(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Error::Duplicated => f.write_str("记录重复了，请删除再试"),
            Error::NotExist => f.write_str("记录不存在呢"),
            Error::Other(s) => f.write_str(s.as_str()),
            Error::Db(d) => {
                f.write_str("数据库出错了")?;
                d.fmt(f)
            }
            Error::Moodle(m) => match m {
                moodle::Error::Req(e) => {
                    f.write_str("和 Moodle 通讯时出错了")?;
                    e.fmt(f)
                }
                moodle::Error::Moodle(me) => {
                    if me.error_code == "invalidtoken" {
                        f.write_str("Moodle 登录过期了")
                    } else {
                        f.write_str("Moodle 出错了")?;
                        me.fmt(f)
                    }
                }
            },
        }
    }
}

impl std::error::Error for Error {}

impl From<moodle::Error> for Error {
    fn from(e: moodle::Error) -> Self {
        Error::Moodle(e)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        if matches!(e, rusqlite::Error::QueryReturnedNoRows) {
            Error::NotExist
        } else {
            Error::Db(e)
        }
    }
}
