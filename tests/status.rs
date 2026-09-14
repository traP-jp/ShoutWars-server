//! `GET /v3/status`。

mod common;

use common::TestServer;
use reqwest::StatusCode;
use serde::Deserialize;
use shoutwars_server::config::Config;

#[derive(Debug, Deserialize)]
struct Status {
    room_count: usize,
    room_limit: usize,
}

#[tokio::test]
async fn returns_room_count_and_limit() {
    let server = TestServer::start().await;

    let reply = server.get("/v3/status").send().await;

    assert_eq!(reply.status, StatusCode::OK);
    let status: Status = reply.msgpack();
    assert!(status.room_limit >= 1, "上限が 1 未満です");
    assert!(
        status.room_count <= status.room_limit,
        "部屋数が上限を超えています"
    );
}

#[tokio::test]
async fn limit_reflects_config() {
    let Some(server) = TestServer::with_config(Config {
        room_limit: 7,
        ..Config::default()
    })
    .await
    else {
        return;
    };

    let status: Status = server.get("/v3/status").send().await.msgpack();

    assert_eq!(status.room_limit, 7);
}
