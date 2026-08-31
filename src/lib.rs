//! ShoutWars のバックエンド。
//!
//! 通信仕様は `docs/protocol.md` を参照。実装ではなく仕様が正である。
//!
//! ランタイムは単一スレッドで動かす。完全に I/O バウンドでサーバー側にゲームロジックが
//! 無いため、マルチコアの恩恵が無い。並列実行を無くすことでデータ競合が原理的に起こらなくなる。

mod api;
mod auth;
pub mod config;
mod error;
mod msgpack;
mod room;
mod rooms;

use std::{future::Future, sync::Arc};

use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post},
};
use tokio::net::TcpListener;

use crate::config::Config;

/// 本文の上限 (仕様 §6.1)。これを超えるリクエストは読まずに拒む。
const BODY_LIMIT: usize = 1024 * 1024;

/// ルーターが共有する状態。
#[derive(Debug, Clone)]
pub(crate) struct AppState {
    config: Arc<Config>,
    rooms: rooms::Shared,
}

/// 設定からルーターを組み立てる。
///
/// 待ち受けと分離してあるのは、テストが任意の設定と空きポートで起動できるようにするため。
pub fn app(config: &Config) -> Router {
    let config = Arc::new(config.clone());
    let state = AppState {
        rooms: rooms::Shared::new(Arc::clone(&config)),
        config,
    };
    Router::new()
        .route("/v3/status", get(api::status::status))
        .route("/v3/room/create", post(api::create::create))
        .route("/v3/room/join", post(api::join::join))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_password,
        ))
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .with_state(state)
}

/// `shutdown` が完了するまで待ち受ける。処理中のリクエストは終わるまで待つ。
///
/// # Errors
/// 接続の待ち受けに失敗した場合。
pub async fn serve(
    listener: TcpListener,
    app: Router,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
}
