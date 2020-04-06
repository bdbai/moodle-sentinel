# Moodle Sentinel
XMUM Moodle 内容更新通知 bot，基于酷 Q（通过 [coolq-sdk-rust](https://github.com/juzi5201314/coolq-sdk-rust) by [橘子](https://github.com/juzi5201314)）。

## 功能
- `订阅 [课程 ID]` 添加订阅，有更新时将会发送通知（仅限群消息）
- `退订 [课程 ID]` 取消订阅（仅限群消息）

## 使用
1. 将插件部署至酷 Q 并运行启动一次，初始化数据库后退出；
2. 使用 sqlite 工具打开 `data/app/com.bdbai.moodle-sentinel` 将自己的 QQ 号码、昵称及 Moodle token 写入 `user` 表；
3. 重新启动酷 Q。

## 构建
使用 `i686-pc-windows-msvc` 目标的 Rust 工具链，运行 `cargo build`。

推荐使用 `rustup` [Directory overrides](https://github.com/rust-lang/rustup#directory-overrides)。

## 测试
设定环境变量

|变量名 | 变量值 |
| --- | --- |
| CQMS_CAMPUS_ID | Moodle 登录名 |
| CQMS_CAMPUS_PASSWORD | Moodle 登录密码 |
| CQMS_QQ | 自己的 QQ 号 |
| CQMS_QQ_NAME | QQ 昵称 |
| CQMS_COURSE_ID | 要订阅的课程 ID |
| CQMS_QQ_GROUP | 要订阅的 QQ 群号 |

然后运行所有测试
```sh
cargo test -- --test-threads 1
```

可适当调整测试顺序，使测试通过（x
