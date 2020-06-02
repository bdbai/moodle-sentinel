use serde::export::Formatter;
use serde::Deserialize;
use std::error::Error;
use std::fmt::{self, Display};

#[derive(Debug, Clone, Deserialize)]
pub struct MoodleError {
    pub exception: Option<String>,
    #[serde(rename = "errorcode")]
    pub error_code: String,
    #[serde(rename = "message")]
    pub message: Option<String>,
    #[serde(rename = "error")]
    pub error: Option<String>,
}
impl Display for MoodleError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(
            self.message
                .as_ref()
                .or(self.error.as_ref())
                .map(|s| s.as_str())
                .unwrap_or(self.error_code.as_str()),
        )
    }
}
impl Error for MoodleError {}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Response<T> {
    MoodleError(MoodleError),
    Data(T),
}

impl<T: std::fmt::Debug> Into<Result<T, MoodleError>> for Response<T> {
    fn into(self) -> Result<T, MoodleError> {
        match self {
            Response::MoodleError(e) => Err(e),
            Response::Data(data) => Ok(data),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginResult {
    pub token: String,
    #[serde(rename = "privatetoken")]
    pub private_token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResourceInfo {
    #[serde(rename = "lastmodified")]
    pub last_modified: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Content {
    #[serde(rename = "fileurl")]
    pub url: String,
    #[serde(rename = "filename")]
    pub name: String,
    #[serde(rename = "timemodified")]
    pub last_modified: i32,
}

// All fields must be Option<T> because of user-invisible contents
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "modname")]
pub enum ModuleType {
    #[serde(rename = "resource")]
    Resource {
        #[serde(rename = "contentsinfo")]
        info: Option<ResourceInfo>,
    },
    #[serde(rename = "mediasite")]
    Mediasite,
    #[serde(rename = "url")]
    Url { contents: Option<Vec<Content>> },
    #[serde(rename = "folder")]
    Folder { contents: Option<Vec<Content>> },
    #[serde(rename = "page")]
    Page,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CourseModule {
    #[serde(default)]
    pub id: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    #[serde(rename = "uservisible")]
    pub user_visible: bool,
    #[serde(flatten)]
    pub content: ModuleType,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CourseSection {
    pub id: u32,
    pub modules: Vec<CourseModule>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoursesPublicInformation {
    pub courses: Vec<CoursePublicInformation>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoursePublicInformation {
    pub id: u32,
    #[serde(rename = "fullname")]
    pub full_name: String,
    #[serde(rename = "displayname")]
    pub display_name: String,
}
