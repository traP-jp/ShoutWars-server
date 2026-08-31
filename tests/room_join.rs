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
    name: String,
    user: UserName,
}

#[derive(Debug, Serialize)]
struct UserName {
    name: String,
}

#[derive(Debug, Deserialize)]
struct Created {
    name: String,
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

fn 作成(size: usize) -> Create {
    Create {
        version: "1.0".to_owned(),
        user: UserName {
            name: "Alice".to_owned(),
        },
        size,
    }
}

fn 参加(number: &str, version: &str) -> Join {
    Join {
        version: version.to_owned(),
        name: number.to_owned(),
        user: UserName {
            name: "Bob".to_owned(),
        },
    }
}

async fn 部屋を用意(server: &TestServer, size: usize) -> Created {
    server
        .post("/v3/room/create", &作成(size))
        .send()
        .await
        .msgpack()
}

#[tokio::test]
async fn 部屋に参加できる() {
    let server = TestServer::start().await;
    let created = 部屋を用意(&server, 2).await;

    let reply = server
        .post("/v3/room/join", &参加(&created.name, "1.0"))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::OK);
    let joined: Joined = reply.msgpack();
    assert!(Uuid::parse_str(&joined.session_id).is_ok());
    assert!(Uuid::parse_str(&joined.id).is_ok());
    assert_eq!(joined.tick_ms, Config::default().tick_ms());
}

#[tokio::test]
async fn 参加者の識別子は部屋主より大きい() {
    let server = TestServer::start().await;
    let created = 部屋を用意(&server, 2).await;

    let joined: Joined = server
        .post("/v3/room/join", &参加(&created.name, "1.0"))
        .send()
        .await
        .msgpack();

    // UUIDv7 は参加順に増える。昇順に並べれば先頭が部屋主になる (仕様「部屋主」)。
    assert!(
        created.user_id < joined.user_id,
        "部屋主 {} より若い ID が振られました: {}",
        created.user_id,
        joined.user_id
    );
}

#[tokio::test]
async fn 存在しない部屋には参加できない() {
    let server = TestServer::start().await;

    let reply = server
        .post("/v3/room/join", &参加("000000", "1.0"))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.error_code(), "room_not_found");
}

#[tokio::test]
async fn 部屋番号の形式が違えば拒む() {
    let server = TestServer::start().await;

    for number in ["12345", "1234567", "12345a", "あいうえお"] {
        let reply = server
            .post("/v3/room/join", &参加(number, "1.0"))
            .send()
            .await;

        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "number={number}");
        assert_eq!(reply.error_code(), "bad_request", "number={number}");
    }
}

#[tokio::test]
async fn バージョンが違えば拒む() {
    let server = TestServer::start().await;
    let created = 部屋を用意(&server, 2).await;

    let reply = server
        .post("/v3/room/join", &参加(&created.name, "1.1"))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::CONFLICT);
    assert_eq!(reply.error_code(), "version_mismatch");
}

#[tokio::test]
async fn 満員なら拒む() {
    let server = TestServer::start().await;
    let created = 部屋を用意(&server, 2).await;

    let reply = server
        .post("/v3/room/join", &参加(&created.name, "1.0"))
        .send()
        .await;
    assert_eq!(reply.status, StatusCode::OK, "2 人目は入れる");

    let reply = server
        .post("/v3/room/join", &参加(&created.name, "1.0"))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::CONFLICT);
    assert_eq!(reply.error_code(), "room_full");
}

#[tokio::test]
async fn 期限切れの部屋には参加できない() {
    let Some(server) = TestServer::with_config(Config {
        lobby_lifetime: std::time::Duration::from_millis(50),
        ..Config::default()
    })
    .await
    else {
        return;
    };
    let created = 部屋を用意(&server, 2).await;
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let reply = server
        .post("/v3/room/join", &参加(&created.name, "1.0"))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.error_code(), "room_not_found");
}

#[tokio::test]
async fn 参加時のtickは経過した窓の数() {
    let Some(server) = TestServer::with_config(Config {
        tick: std::time::Duration::from_millis(20),
        ..Config::default()
    })
    .await
    else {
        return;
    };
    let created = 部屋を用意(&server, 4).await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let joined: Joined = server
        .post("/v3/room/join", &参加(&created.name, "1.0"))
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
