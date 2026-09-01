use std::net::{Ipv4Addr, SocketAddr};

use shoutwars_server::{app, config::Config, serve};
use tokio::net::TcpListener;

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
    let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, config.port));
    let listener = TcpListener::bind(address).await?;
    // 基盤が渡した設定を確かめられるようにする。
    // パスワードは有無だけを出す。設定漏れと値の誤りを切り分けられれば足りる。
    tracing::info!(
        %address,
        password = config.password.is_some(),
        room_limit = config.room_limit,
        lobby_lifetime = ?config.lobby_lifetime,
        game_lifetime = ?config.game_lifetime,
        tick_ms = config.tick_ms(),
        record_retention = config.record_retention,
        "起動しました"
    );

    serve(listener, app(&config), async {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("停止します");
    })
    .await?;
    Ok(())
}
