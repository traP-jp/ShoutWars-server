//! `POST /v3/room/start` (仕様 §4.5)。

mod common;

use common::TestServer;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Serialize)]
struct Start {
    session_id: String,
}

#[derive(Debug, Deserialize)]
struct Created {
    name: String,
    session_id: String,
}

#[derive(Debug, Deserialize)]
struct Joined {
    session_id: String,
}

async fn 部屋を用意(server: &TestServer) -> Created {
    server
        .post(
            "/v3/room/create",
            &Create {
                version: "1.0".to_owned(),
                user: UserName {
                    name: "Alice".to_owned(),
                },
                size: 2,
            },
        )
        .send()
        .await
        .msgpack()
}

async fn 参加(server: &TestServer, number: &str) -> Joined {
    server
        .post(
            "/v3/room/join",
            &Join {
                version: "1.0".to_owned(),
                name: number.to_owned(),
                user: UserName {
                    name: "Bob".to_owned(),
                },
            },
        )
        .send()
        .await
        .msgpack()
}

fn 開始(session_id: &str) -> Start {
    Start {
        session_id: session_id.to_owned(),
    }
}

#[tokio::test]
async fn 部屋主は開始できる() {
    let server = TestServer::start().await;
    let created = 部屋を用意(&server).await;

    let reply = server
        .post("/v3/room/start", &開始(&created.session_id))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::OK);
}

#[tokio::test]
async fn 部屋主以外は開始できない() {
    let server = TestServer::start().await;
    let created = 部屋を用意(&server).await;
    let joined = 参加(&server, &created.name).await;

    let reply = server
        .post("/v3/room/start", &開始(&joined.session_id))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert_eq!(reply.error_code(), "not_owner");
}

#[tokio::test]
async fn 二度目の開始は拒む() {
    let server = TestServer::start().await;
    let created = 部屋を用意(&server).await;

    server
        .post("/v3/room/start", &開始(&created.session_id))
        .send()
        .await;
    let reply = server
        .post("/v3/room/start", &開始(&created.session_id))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::CONFLICT);
    assert_eq!(reply.error_code(), "game_started");
}

#[tokio::test]
async fn 無効なセッションは拒む() {
    let server = TestServer::start().await;
    部屋を用意(&server).await;

    let reply = server
        .post("/v3/room/start", &開始(&Uuid::new_v4().to_string()))
        .send()
        .await;

    // 部屋の存在に言及しない (§5.3)。
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.error_code(), "invalid_session");
}

#[tokio::test]
async fn 開始した部屋には参加できない() {
    let server = TestServer::start().await;
    let created = 部屋を用意(&server).await;

    server
        .post("/v3/room/start", &開始(&created.session_id))
        .send()
        .await;
    let reply = server
        .post(
            "/v3/room/join",
            &Join {
                version: "1.0".to_owned(),
                name: created.name.clone(),
                user: UserName {
                    name: "Bob".to_owned(),
                },
            },
        )
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::CONFLICT);
    assert_eq!(reply.error_code(), "game_started");
}
