use cqrs_builder::AppJson;

fn main() {
    AppJson::new("com.bdbai.moodle-sentinel")
        .name("Moodle Sentinel".to_owned())
        .author("bdbai <bdbaiapp@163.com>".to_owned())
        .version("0.1.0".to_owned())
        .version_id(10)
        .description("Check new content on XMUM Moodle".to_owned())
        .finish();
}
