//! 定義されていないパスとメソッドへの応答。

mod common;

use common::TestServer;
use reqwest::StatusCode;
use shoutwars_server::config::Config;

#[tokio::test]
async fn an_unknown_path_returns_the_error_format() {
    let server = TestServer::start().await;

    let reply = server.post("/v3/room/nope", &()).send().await;

    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.error_code(), "not_found");
}

/// 現行のクライアントは `/v2` を喋る。繋いだときに理由が読めなければならない。
#[tokio::test]
async fn an_old_api_version_returns_the_error_format() {
    let server = TestServer::start().await;

    let reply = server.post("/v2/room/create", &()).send().await;

    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.error_code(), "not_found");
}

#[tokio::test]
async fn a_wrong_method_returns_the_error_format() {
    let server = TestServer::start().await;

    let reply = server.get("/v3/room/sync").send().await;

    assert_eq!(reply.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(reply.error_code(), "not_found");
    // 本文を差し替えても、どのメソッドが使えるかは標準どおり伝わること。
    assert_eq!(
        reply.headers.get("allow").map(|v| v.to_str().unwrap_or("")),
        Some("POST")
    );
}

/// パスワードは API 全体を守る。存在しないパスも例外ではない。
#[tokio::test]
async fn an_unknown_path_still_requires_the_password() {
    let Some(server) = TestServer::with_config(Config {
        password: Some("hunter2".to_owned()),
        ..Config::default()
    })
    .await
    else {
        return;
    };

    let reply = server.post("/v3/room/nope", &()).send().await;

    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.error_code(), "unauthorized");
}
