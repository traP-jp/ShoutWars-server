//! `POST /v3/room/create`。

mod common;

use common::TestServer;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use shoutwars_server::config::Config;
use uuid::Uuid;

#[derive(Debug, Serialize)]
struct Request {
    version: String,
    user: UserName,
    size: usize,
}

#[derive(Debug, Serialize)]
struct UserName {
    name: String,
}

/// UUID は文字列で流れる。バイト列で符号化していれば、ここで落ちる。
#[derive(Debug, Deserialize)]
struct Created {
    session_id: String,
    user_id: String,
    id: String,
    code: String,
    next_tick: u64,
    tick_ms: u64,
}

fn create_request(name: &str, size: usize) -> Request {
    Request {
        version: "1.0".to_owned(),
        user: UserName {
            name: name.to_owned(),
        },
        size,
    }
}

#[tokio::test]
async fn creates_a_room() {
    let server = TestServer::start().await;

    let reply = server
        .post("/v3/room/create", &create_request("Alice", 2))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::OK);
    let created: Created = reply.msgpack();
    for (label, text) in [
        ("session_id", &created.session_id),
        ("user_id", &created.user_id),
        ("id", &created.id),
    ] {
        assert!(
            Uuid::parse_str(text).is_ok(),
            "{label} が UUID の文字列ではありません: {text}"
        );
    }
    assert_eq!(created.next_tick, 0, "作成直後は 0 から受け取る");
}

#[tokio::test]
async fn room_number_is_six_digits() {
    let server = TestServer::start().await;

    let created: Created = server
        .post("/v3/room/create", &create_request("Alice", 2))
        .send()
        .await
        .msgpack();

    assert_eq!(
        created.code.len(),
        6,
        "6 桁ではありません: {}",
        created.code
    );
    assert!(
        created.code.bytes().all(|b| b.is_ascii_digit()),
        "数字以外を含みます: {}",
        created.code
    );
}

#[tokio::test]
async fn room_numbers_do_not_collide() {
    let server = TestServer::start().await;

    let mut numbers = Vec::new();
    for _ in 0..20 {
        let created: Created = server
            .post("/v3/room/create", &create_request("Alice", 2))
            .send()
            .await
            .msgpack();
        numbers.push(created.code);
    }

    numbers.sort_unstable();
    let before = numbers.len();
    numbers.dedup();
    assert_eq!(numbers.len(), before, "参加コードが重複しました");
}

#[tokio::test]
async fn tick_ms_reflects_config() {
    let Some(server) = TestServer::with_config(Config {
        tick: std::time::Duration::from_millis(5),
        ..Config::default()
    })
    .await
    else {
        return;
    };

    let created: Created = server
        .post("/v3/room/create", &create_request("Alice", 2))
        .send()
        .await
        .msgpack();

    assert_eq!(created.tick_ms, 5);
}

#[tokio::test]
async fn room_count_increases() {
    let server = TestServer::start().await;

    let before: Status = server.get("/v3/status").send().await.msgpack();
    server
        .post("/v3/room/create", &create_request("Alice", 2))
        .send()
        .await;
    let after: Status = server.get("/v3/status").send().await.msgpack();

    // 外部サーバーは他のテストと共有するため、増分がちょうど 1 とは限らない。
    assert!(
        after.room_count > before.room_count,
        "部屋を作っても room_count が増えていません"
    );
}

#[derive(Debug, Deserialize)]
struct Status {
    room_count: usize,
}

#[tokio::test]
async fn rejects_a_size_out_of_range() {
    let server = TestServer::start().await;

    for size in [0, 1, 5, 100] {
        let reply = server
            .post("/v3/room/create", &create_request("Alice", size))
            .send()
            .await;

        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "size={size}");
        assert_eq!(reply.error_code(), "bad_request", "size={size}");
    }
}

#[tokio::test]
async fn rejects_a_too_long_user_name() {
    let server = TestServer::start().await;

    let reply = server
        .post("/v3/room/create", &create_request(&"あ".repeat(33), 2))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.error_code(), "bad_request");
}

#[tokio::test]
async fn accepts_a_user_name_at_the_limit() {
    let server = TestServer::start().await;

    let reply = server
        .post("/v3/room/create", &create_request(&"あ".repeat(32), 2))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::OK, "32 文字は上限以内");
}

#[tokio::test]
async fn rejects_when_the_room_limit_is_reached() {
    let Some(server) = TestServer::with_config(Config {
        room_limit: 2,
        ..Config::default()
    })
    .await
    else {
        return;
    };

    for _ in 0..2 {
        let reply = server
            .post("/v3/room/create", &create_request("Alice", 2))
            .send()
            .await;
        assert_eq!(reply.status, StatusCode::OK);
    }

    let reply = server
        .post("/v3/room/create", &create_request("Alice", 2))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(reply.error_code(), "room_limit_reached");
}

#[tokio::test]
async fn rejects_a_malformed_body() {
    let server = TestServer::start().await;

    let reply = server
        .post("/v3/room/create", &"これは部屋ではない")
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.error_code(), "bad_request");
}

#[tokio::test]
async fn does_not_count_expired_rooms() {
    let Some(server) = TestServer::with_config(Config {
        lobby_lifetime: std::time::Duration::from_millis(50),
        ..Config::default()
    })
    .await
    else {
        return;
    };

    server
        .post("/v3/room/create", &create_request("Alice", 2))
        .send()
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let status: Status = server.get("/v3/status").send().await.msgpack();

    assert_eq!(status.room_count, 0, "期限切れの部屋が残っています");
}

#[tokio::test]
async fn the_session_id_is_not_a_uuidv7() {
    let server = TestServer::start().await;

    let created: Created = server
        .post("/v3/room/create", &create_request("Alice", 2))
        .send()
        .await
        .expect_ok();

    // セッション ID は予測できてはならない。UUIDv7 は先頭 48 bit が時刻で秘密にならない。
    let session = Uuid::parse_str(&created.session_id).expect("UUID として読める");
    assert_eq!(
        session.get_version_num(),
        4,
        "セッション ID が UUIDv4 ではありません"
    );
    // ユーザー ID は逆に、参加順に増えなければならない。
    let user = Uuid::parse_str(&created.user_id).expect("UUID として読める");
    assert_eq!(
        user.get_version_num(),
        7,
        "ユーザー ID が UUIDv7 ではありません"
    );
}
