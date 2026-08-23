use khogame::{run, AppConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env: ưu tiên file .env ở thư mục hiện tại (tránh nhầm .env của thư mục cha)
    if dotenvy::from_filename(".env").is_err() {
        let _ = dotenvy::dotenv();
    }

    // Init tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "khogame=debug,tower_http=info".into()),
        )
        .with_target(true)
        .init();

    let config = AppConfig::from_env()?;
    tracing::info!("Starting Kho Game server on {}:{}", config.host, config.port);

    run(config).await
}
