//! `POST /v3/room/start`。

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

async fn prepare_room(server: &TestServer) -> Created {
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

async fn join_room(server: &TestServer, number: &str) -> Joined {
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

fn start_request(session_id: &str) -> Start {
    Start {
        session_id: session_id.to_owned(),
    }
}

#[tokio::test]
async fn the_owner_can_start() {
    let server = TestServer::start().await;
    let created = prepare_room(&server).await;

    let reply = server
        .post("/v3/room/start", &start_request(&created.session_id))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::OK);
}

#[tokio::test]
async fn others_cannot_start() {
    let server = TestServer::start().await;
    let created = prepare_room(&server).await;
    let joined = join_room(&server, &created.name).await;

    let reply = server
        .post("/v3/room/start", &start_request(&joined.session_id))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert_eq!(reply.error_code(), "not_owner");
}

#[tokio::test]
async fn rejects_starting_twice() {
    let server = TestServer::start().await;
    let created = prepare_room(&server).await;

    server
        .post("/v3/room/start", &start_request(&created.session_id))
        .send()
        .await;
    let reply = server
        .post("/v3/room/start", &start_request(&created.session_id))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::CONFLICT);
    assert_eq!(reply.error_code(), "game_started");
}

#[tokio::test]
async fn rejects_an_invalid_session() {
    let server = TestServer::start().await;
    prepare_room(&server).await;

    let reply = server
        .post(
            "/v3/room/start",
            &start_request(&Uuid::new_v4().to_string()),
        )
        .send()
        .await;

    // 部屋の存在に言及しない。
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.error_code(), "invalid_session");
}

#[tokio::test]
async fn cannot_join_after_the_start() {
    let server = TestServer::start().await;
    let created = prepare_room(&server).await;

    server
        .post("/v3/room/start", &start_request(&created.session_id))
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
