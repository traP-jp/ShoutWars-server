//! `PASSWORD` による認証 (仕様 §4、§5.2)。
//!
//! いずれもサーバー側の設定を前提とするため、外部サーバーに対しては実行されない。

mod common;

use common::TestServer;
use reqwest::StatusCode;
use shoutwars_server::config::Config;

const PASSWORD: &str = "テスト用のパスワード";

fn 認証あり() -> Config {
    Config {
        password: Some(PASSWORD.to_owned()),
        ..Config::default()
    }
}

#[tokio::test]
#[ignore = "認証が未実装"]
async fn 正しいパスワードなら通す() {
    let Some(server) = TestServer::with_config(認証あり()).await else {
        return;
    };

    let reply = server.get("/v3/status").bearer(PASSWORD).send().await;

    assert_eq!(reply.status, StatusCode::OK);
}

#[tokio::test]
#[ignore = "認証が未実装"]
async fn パスワード無しは拒む() {
    let Some(server) = TestServer::with_config(認証あり()).await else {
        return;
    };

    let reply = server.get("/v3/status").send().await;

    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.error_code(), "unauthorized");
}

#[tokio::test]
#[ignore = "認証が未実装"]
async fn 誤ったパスワードは拒む() {
    let Some(server) = TestServer::with_config(認証あり()).await else {
        return;
    };

    let reply = server.get("/v3/status").bearer("ちがう").send().await;

    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.error_code(), "unauthorized");
}

#[tokio::test]
async fn パスワード未設定なら認証を求めない() {
    let Some(server) = TestServer::with_config(Config::default()).await else {
        return;
    };

    let reply = server.get("/v3/status").send().await;

    assert_eq!(reply.status, StatusCode::OK);
}
