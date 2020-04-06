use crate::moodle::response::MoodleError;
use std::fmt::{self, Display, Formatter};

#[derive(Debug)]
pub enum Error {
    Moodle(MoodleError),
    Req(reqwest::Error),
}

impl From<MoodleError> for Error {
    fn from(e: MoodleError) -> Self {
        Error::Moodle(e)
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Req(e)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Error::Moodle(e) => e.fmt(f),
            Error::Req(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {}
