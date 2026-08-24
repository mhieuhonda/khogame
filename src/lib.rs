pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod repositories;
pub mod routes;
pub mod state;
pub mod templates;
pub mod utils;

pub use config::AppConfig;
pub use error::{AppError, AppResult};
pub use state::AppState;

use std::sync::Arc;
use tower_http::services::ServeDir;

pub async fn run(config: AppConfig) -> anyhow::Result<()> {
    let state = AppState::new(config.clone()).await?;

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&state.db)
        .await
        .map_err(|e| anyhow::anyhow!("Migration failed: {}", e))?;

    let app = routes::build_router(Arc::new(state));

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Listening on http://{}", addr);
    // Dùng into_make_service_with_connect_info để middleware có thể lấy IP thật
    // của client qua ConnectInfo<SocketAddr> extractor (chống giả mạo X-Forwarded-For).
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}

pub fn static_service() -> ServeDir {
    ServeDir::new("static")
}
