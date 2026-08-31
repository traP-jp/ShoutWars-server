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
    tracing::info!(%address, "起動しました");

    serve(listener, app(&config), async {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("停止します");
    })
    .await?;
    Ok(())
}
