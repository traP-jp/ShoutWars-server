//! ShoutWars のバックエンド。
//!
//! 通信仕様は `docs/protocol.md` を参照。実装ではなく仕様が正である。
//!
//! ランタイムは単一スレッドで動かす。完全に I/O バウンドでサーバー側にゲームロジックが
//! 無いため、マルチコアの恩恵が無い。並列実行を無くすことでデータ競合が原理的に起こらなくなる。

pub mod config;
mod msgpack;
mod status;

use std::future::Future;

use axum::{Router, routing::get};
use tokio::net::TcpListener;

use crate::config::Config;

/// ルーターが共有する状態。
#[derive(Debug, Clone)]
pub(crate) struct AppState {
    room_limit: usize,
}

/// 設定からルーターを組み立てる。
///
/// 待ち受けと分離してあるのは、テストが任意の設定と空きポートで起動できるようにするため。
pub fn app(config: &Config) -> Router {
    let state = AppState {
        room_limit: config.room_limit,
    };
    Router::new()
        .route("/v3/status", get(status::status))
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
