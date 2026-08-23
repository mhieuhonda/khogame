pub mod config;
pub mod db;
pub mod error;
pub mod state;
pub mod auth;
pub mod middleware;
pub mod models;
pub mod repositories;
pub mod handlers;
pub mod templates;
pub mod routes;
pub mod utils;

pub use config::AppConfig;
pub use state::AppState;
pub use error::{AppError, AppResult};

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
    axum::serve(listener, app).await?;
    Ok(())
}

pub fn static_service() -> ServeDir {
    ServeDir::new("static")
}
