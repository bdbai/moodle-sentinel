mod error;
mod response;

pub use crate::moodle::error::Error;
pub use crate::moodle::response::{CourseModule, CourseSection, ModuleType};

use crate::moodle::response::{CoursesPublicInformation, LoginResult, MoodleError, Response};
use lazy_static::lazy_static;
use reqwest::{self, Client};
use serde::Serialize;

lazy_static! {
    static ref CLIENT: Client = Client::new();
}

const LOGIN_URL: &'static str = "https://l.xmu.edu.my/login/token.php?service=moodle_mobile_app";
const API_URL: &'static str = "https://l.xmu.edu.my/webservice/rest/server.php";

#[allow(unused)]
pub async fn login<T: Serialize>(username: T, password: T) -> Result<LoginResult, Error> {
    Ok(Into::<Result<LoginResult, MoodleError>>::into(
        CLIENT
            .post(LOGIN_URL)
            .form(&[("username", username), ("password", password)])
            .send()
            .await?
            .json::<Response<LoginResult>>()
            .await?,
    )?)
}

// Not `core_course_get_updates_since` because
// "This module does not implement the check_updates_since callback: module"
pub async fn get_course_content(
    token: impl AsRef<str>,
    course_id: impl Serialize,
) -> Result<Vec<CourseSection>, Error> {
    Ok(Into::<Result<Vec<CourseSection>, MoodleError>>::into(
        CLIENT
            .post(API_URL)
            .query(&[
                ("wsfunction", "core_course_get_contents"),
                ("wstoken", token.as_ref()),
                ("moodlewsrestformat", "json"),
            ])
            .form(&[("courseid", course_id)])
            .send()
            .await?
            .json::<Response<Vec<CourseSection>>>()
            .await?,
    )?)
}

pub async fn get_course_public_information(
    token: impl AsRef<str>,
    course_id: u32,
) -> Result<CoursesPublicInformation, Error> {
    Ok(Into::<Result<CoursesPublicInformation, MoodleError>>::into(
        CLIENT
            .post(API_URL)
            .query(&[
                ("wsfunction", "core_course_get_courses_by_field"),
                ("wstoken", token.as_ref()),
                ("moodlewsrestformat", "json"),
            ])
            .form(&[("field", "id"), ("value", course_id.to_string().as_str())])
            .send()
            .await?
            .json::<Response<CoursesPublicInformation>>()
            .await?,
    )?)
}

#[tokio::test]
async fn get_course_content_test() {
    let token = login(env!("CQMS_CAMPUS_ID"), env!("CQMS_CAMPUS_PASSWORD"))
        .await
        .unwrap()
        .token;
    let sections = get_course_content(token, env!("CQMS_COURSE_ID").parse::<u32>().unwrap())
        .await
        .unwrap();
    println!("{:#?}", sections);
}

#[tokio::test]
async fn get_course_public_information_test() {
    let token = login(env!("CQMS_CAMPUS_ID"), env!("CQMS_CAMPUS_PASSWORD"))
        .await
        .unwrap()
        .token;
    println!(
        "{:#?}",
        get_course_public_information(token, env!("CQMS_COURSE_ID").parse().unwrap())
            .await
            .unwrap()
    );
}
