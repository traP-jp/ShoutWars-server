//! ShoutWars のバックエンド。
//!
//! 通信仕様は `docs/protocol.md` を参照。実装ではなく仕様が正である。
//!
//! ランタイムは単一スレッドで動かす。完全に I/O バウンドでサーバー側にゲームロジックが
//! 無いため、マルチコアの恩恵が無い。並列実行を無くすことでデータ競合が原理的に起こらなくなる。

mod config;
mod msgpack;

use std::net::{Ipv4Addr, SocketAddr};

use axum::{Router, extract::State, routing::get};
use serde::Serialize;
use tokio::net::TcpListener;

use crate::{config::Config, msgpack::MsgPack};

#[derive(Clone)]
struct AppState {
    room_limit: usize,
}

#[derive(Serialize)]
struct Status {
    room_count: usize,
    room_limit: usize,
}

async fn status(State(state): State<AppState>) -> MsgPack<Status> {
    MsgPack(Status {
        room_count: 0, // TODO: 部屋の管理を実装したら差し替える
        room_limit: state.room_limit,
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run().await {
        // Result を返す main は Debug 表示になり読みにくいため、自分で出す。
        eprintln!("エラー: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "shoutwars_server=info,tower_http=info".into()),
        )
        .init();

    let config = Config::from_env()?;
    let state = AppState {
        room_limit: config.room_limit,
    };

    let app = Router::new()
        .route("/v3/status", get(status))
        .with_state(state);

    let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, config.port));
    let listener = TcpListener::bind(address).await?;
    tracing::info!(%address, "起動しました");

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("停止します");
        })
        .await?;
    Ok(())
}
