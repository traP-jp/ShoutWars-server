//! `PASSWORD` による認証。
//!
//! いずれもサーバー側の設定を前提とするため、外部サーバーに対しては実行されない。

mod common;

use common::TestServer;
use reqwest::StatusCode;
use shoutwars_server::config::Config;

const PASSWORD: &str = "correct horse battery staple";

/// ヘッダ値は可視 ASCII しか文字列として読めない。
/// バイト列のまま比較していないとこの形のパスワードが常に拒否される。
const NON_ASCII_PASSWORD: &str = "テスト用のパスワード";

fn config_with_auth() -> Config {
    config_with_password(PASSWORD)
}

fn config_with_password(password: &str) -> Config {
    Config {
        password: Some(password.to_owned()),
        ..Config::default()
    }
}

#[tokio::test]
async fn accepts_the_correct_password() {
    let Some(server) = TestServer::with_config(config_with_auth()).await else {
        return;
    };

    let reply = server.get("/v3/status").bearer(PASSWORD).send().await;

    assert_eq!(reply.status, StatusCode::OK);
}

#[tokio::test]
async fn rejects_a_missing_password() {
    let Some(server) = TestServer::with_config(config_with_auth()).await else {
        return;
    };

    let reply = server.get("/v3/status").send().await;

    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.error_code(), "unauthorized");
}

#[tokio::test]
async fn rejects_a_wrong_password() {
    let Some(server) = TestServer::with_config(config_with_auth()).await else {
        return;
    };

    let reply = server.get("/v3/status").bearer("ちがう").send().await;

    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.error_code(), "unauthorized");
}

#[tokio::test]
async fn requires_nothing_when_no_password_is_set() {
    let Some(server) = TestServer::with_config(Config::default()).await else {
        return;
    };

    let reply = server.get("/v3/status").send().await;

    assert_eq!(reply.status, StatusCode::OK);
}

#[tokio::test]
async fn accepts_a_password_outside_visible_ascii() {
    let Some(server) = TestServer::with_config(config_with_password(NON_ASCII_PASSWORD)).await
    else {
        return;
    };

    let reply = server
        .get("/v3/status")
        .bearer(NON_ASCII_PASSWORD)
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::OK);
}

#[tokio::test]
async fn scheme_name_is_case_insensitive() {
    let Some(server) = TestServer::with_config(config_with_auth()).await else {
        return;
    };

    let reply = server
        .get("/v3/status")
        .header("Authorization", &format!("bearer {PASSWORD}"))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::OK);
}

#[tokio::test]
async fn rejects_other_schemes() {
    let Some(server) = TestServer::with_config(config_with_auth()).await else {
        return;
    };

    let reply = server
        .get("/v3/status")
        .header("Authorization", &format!("Basic {PASSWORD}"))
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.error_code(), "unauthorized");
}
