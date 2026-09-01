//! `POST /v3/room/join`。

mod common;

use common::TestServer;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use shoutwars_server::config::Config;
use uuid::Uuid;

#[derive(Debug, Serialize)]
struct Create {
    version: String,
    user: UserName,
    size: usize,
}

#[derive(Debug, Serialize)]
struct Join {
    version: String,
    code: String,
    user: UserName,
}

#[derive(Debug, Serialize)]
struct UserName {
    name: String,
}

#[derive(Debug, Deserialize)]
struct Created {
    code: String,
    user_id: String,
}

#[derive(Debug, Deserialize)]
struct Joined {
    session_id: String,
    user_id: String,
    id: String,
    next_tick: u64,
    tick_ms: u64,
}

fn create_request(size: usize) -> Create {
    Create {
        version: "1.0".to_owned(),
        user: UserName {
            name: "Alice".to_owned(),
        },
        size,
    }
}

fn join_request(code: &str, version: &str) -> Join {
    Join {
        version: version.to_owned(),
        code: code.to_owned(),
        user: UserName {
            name: "Bob".to_owned(),
        },
    }
}

async fn prepare_room(server: &TestServer, size: usize) -> Created {
    server
        .post("/v3/room/create", &create_request(size))
        .send()
        .await
        .msgpack()
}

#[tokio::test]
async fn joins_a_room() {
    let server = TestServer::start().await;
    let created = prepare_room(&server, 2).await;

    let reply = server
        .post("/v3/room/join", &join_request(&created.code, "1.0"))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::OK);
    let joined: Joined = reply.msgpack();
    assert!(Uuid::parse_str(&joined.session_id).is_ok());
    assert!(Uuid::parse_str(&joined.id).is_ok());
    assert_eq!(joined.tick_ms, Config::default().tick_ms());
}

#[tokio::test]
async fn joiner_id_is_greater_than_the_owner() {
    let server = TestServer::start().await;
    let created = prepare_room(&server, 2).await;

    let joined: Joined = server
        .post("/v3/room/join", &join_request(&created.code, "1.0"))
        .send()
        .await
        .msgpack();

    // UUIDv7 はjoin_request順に増える。昇順に並べれば先頭が部屋主になる。
    assert!(
        created.user_id < joined.user_id,
        "部屋主 {} より若い ID が振られました: {}",
        created.user_id,
        joined.user_id
    );
}

#[tokio::test]
async fn cannot_join_a_missing_room() {
    let server = TestServer::start().await;

    let reply = server
        .post("/v3/room/join", &join_request("000000", "1.0"))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.error_code(), "room_not_found");
}

#[tokio::test]
async fn rejects_a_malformed_room_number() {
    let server = TestServer::start().await;

    for code in ["12345", "1234567", "12345a", "あいうえお"] {
        let reply = server
            .post("/v3/room/join", &join_request(code, "1.0"))
            .send()
            .await;

        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "code={code}");
        assert_eq!(reply.error_code(), "bad_request", "code={code}");
    }
}

#[tokio::test]
async fn rejects_a_version_mismatch() {
    let server = TestServer::start().await;
    let created = prepare_room(&server, 2).await;

    let reply = server
        .post("/v3/room/join", &join_request(&created.code, "1.1"))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::CONFLICT);
    assert_eq!(reply.error_code(), "version_mismatch");
}

#[tokio::test]
async fn rejects_a_full_room() {
    let server = TestServer::start().await;
    let created = prepare_room(&server, 2).await;

    let reply = server
        .post("/v3/room/join", &join_request(&created.code, "1.0"))
        .send()
        .await;
    assert_eq!(reply.status, StatusCode::OK, "2 人目は入れる");

    let reply = server
        .post("/v3/room/join", &join_request(&created.code, "1.0"))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::CONFLICT);
    assert_eq!(reply.error_code(), "room_full");
}

#[tokio::test]
async fn cannot_join_an_expired_room() {
    let Some(server) = TestServer::with_config(Config {
        lobby_lifetime: std::time::Duration::from_millis(50),
        ..Config::default()
    })
    .await
    else {
        return;
    };
    let created = prepare_room(&server, 2).await;
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let reply = server
        .post("/v3/room/join", &join_request(&created.code, "1.0"))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.error_code(), "room_not_found");
}

#[tokio::test]
async fn cursor_starts_at_the_current_window() {
    let Some(server) = TestServer::with_config(Config {
        tick: std::time::Duration::from_millis(20),
        ..Config::default()
    })
    .await
    else {
        return;
    };
    let created = prepare_room(&server, 4).await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let joined: Joined = server
        .post("/v3/room/join", &join_request(&created.code, "1.0"))
        .send()
        .await
        .msgpack();

    // 100 ms 経過し 1 窓 20 ms なので 5 窓目以降。処理時間ぶん余裕を見る。
    assert!(
        (5..10).contains(&joined.next_tick),
        "経過した窓の数と合いません: {}",
        joined.next_tick
    );
}
