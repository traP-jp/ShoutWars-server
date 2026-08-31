//! `PASSWORD` による認証 (仕様「API」、仕様「エラーコード一覧」)。
//!
//! いずれもサーバー側の設定を前提とするため、外部サーバーに対しては実行されない。

mod common;

use common::TestServer;
use reqwest::StatusCode;
use shoutwars_server::config::Config;

const PASSWORD: &str = "correct horse battery staple";

/// ヘッダ値は可視 ASCII しか文字列として読めない。バイト列のまま比較していないと
/// この形のパスワードが常に拒否される。
const 非ASCIIのパスワード: &str = "テスト用のパスワード";

fn 認証あり() -> Config {
    パスワードを設定(PASSWORD)
}

fn パスワードを設定(password: &str) -> Config {
    Config {
        password: Some(password.to_owned()),
        ..Config::default()
    }
}

#[tokio::test]
async fn 正しいパスワードなら通す() {
    let Some(server) = TestServer::with_config(認証あり()).await else {
        return;
    };

    let reply = server.get("/v3/status").bearer(PASSWORD).send().await;

    assert_eq!(reply.status, StatusCode::OK);
}

#[tokio::test]
async fn パスワード無しは拒む() {
    let Some(server) = TestServer::with_config(認証あり()).await else {
        return;
    };

    let reply = server.get("/v3/status").send().await;

    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.error_code(), "unauthorized");
}

#[tokio::test]
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

#[tokio::test]
async fn 可視文字以外を含むパスワードでも通る() {
    let Some(server) = TestServer::with_config(パスワードを設定(非ASCIIのパスワード)).await
    else {
        return;
    };

    let reply = server
        .get("/v3/status")
        .bearer(非ASCIIのパスワード)
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::OK);
}

#[tokio::test]
async fn スキーム名は大文字小文字を区別しない() {
    let Some(server) = TestServer::with_config(認証あり()).await else {
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
async fn 他のスキームは拒む() {
    let Some(server) = TestServer::with_config(認証あり()).await else {
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
