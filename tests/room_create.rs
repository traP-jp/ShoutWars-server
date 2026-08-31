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
    name: String,
    next_tick: u64,
    tick_ms: u64,
}

fn 部屋を作る(name: &str, size: usize) -> Request {
    Request {
        version: "1.0".to_owned(),
        user: UserName {
            name: name.to_owned(),
        },
        size,
    }
}

#[tokio::test]
async fn 部屋を作れる() {
    let server = TestServer::start().await;

    let reply = server
        .post("/v3/room/create", &部屋を作る("Alice", 2))
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
async fn 部屋番号は六桁の数字() {
    let server = TestServer::start().await;

    let created: Created = server
        .post("/v3/room/create", &部屋を作る("Alice", 2))
        .send()
        .await
        .msgpack();

    assert_eq!(
        created.name.len(),
        6,
        "6 桁ではありません: {}",
        created.name
    );
    assert!(
        created.name.bytes().all(|b| b.is_ascii_digit()),
        "数字以外を含みます: {}",
        created.name
    );
}

#[tokio::test]
async fn 部屋番号は重複しない() {
    let server = TestServer::start().await;

    let mut numbers = Vec::new();
    for _ in 0..20 {
        let created: Created = server
            .post("/v3/room/create", &部屋を作る("Alice", 2))
            .send()
            .await
            .msgpack();
        numbers.push(created.name);
    }

    numbers.sort_unstable();
    let before = numbers.len();
    numbers.dedup();
    assert_eq!(numbers.len(), before, "部屋番号が重複しました");
}

#[tokio::test]
async fn tick_msは設定を伝える() {
    let Some(server) = TestServer::with_config(Config {
        tick: std::time::Duration::from_millis(5),
        ..Config::default()
    })
    .await
    else {
        return;
    };

    let created: Created = server
        .post("/v3/room/create", &部屋を作る("Alice", 2))
        .send()
        .await
        .msgpack();

    assert_eq!(created.tick_ms, 5);
}

#[tokio::test]
async fn 部屋数に反映される() {
    let server = TestServer::start().await;

    let before: Status = server.get("/v3/status").send().await.msgpack();
    server
        .post("/v3/room/create", &部屋を作る("Alice", 2))
        .send()
        .await;
    let after: Status = server.get("/v3/status").send().await.msgpack();

    assert_eq!(after.room_count, before.room_count + 1);
}

#[derive(Debug, Deserialize)]
struct Status {
    room_count: usize,
}

#[tokio::test]
async fn 人数が範囲外なら拒む() {
    let server = TestServer::start().await;

    for size in [0, 1, 5, 100] {
        let reply = server
            .post("/v3/room/create", &部屋を作る("Alice", size))
            .send()
            .await;

        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "size={size}");
        assert_eq!(reply.error_code(), "bad_request", "size={size}");
    }
}

#[tokio::test]
async fn 長すぎるユーザー名は拒む() {
    let server = TestServer::start().await;

    let reply = server
        .post("/v3/room/create", &部屋を作る(&"あ".repeat(33), 2))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.error_code(), "bad_request");
}

#[tokio::test]
async fn 上限ちょうどのユーザー名は通る() {
    let server = TestServer::start().await;

    let reply = server
        .post("/v3/room/create", &部屋を作る(&"あ".repeat(32), 2))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::OK, "32 文字は上限以内");
}

#[tokio::test]
async fn 部屋数の上限に達したら拒む() {
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
            .post("/v3/room/create", &部屋を作る("Alice", 2))
            .send()
            .await;
        assert_eq!(reply.status, StatusCode::OK);
    }

    let reply = server
        .post("/v3/room/create", &部屋を作る("Alice", 2))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(reply.error_code(), "room_limit_reached");
}

#[tokio::test]
async fn 形式が違う本文は拒む() {
    let server = TestServer::start().await;

    let reply = server
        .post("/v3/room/create", &"これは部屋ではない")
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.error_code(), "bad_request");
}

#[tokio::test]
async fn 期限切れの部屋は数えない() {
    let Some(server) = TestServer::with_config(Config {
        lobby_lifetime: std::time::Duration::from_millis(50),
        ..Config::default()
    })
    .await
    else {
        return;
    };

    server
        .post("/v3/room/create", &部屋を作る("Alice", 2))
        .send()
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let status: Status = server.get("/v3/status").send().await.msgpack();

    assert_eq!(status.room_count, 0, "期限切れの部屋が残っています");
}
