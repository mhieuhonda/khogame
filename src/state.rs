use crate::config::AppConfig;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<AppConfig>,
    pub http_client: reqwest::Client,
}

impl AppState {
    pub async fn new(config: AppConfig) -> anyhow::Result<Self> {
        let db = crate::db::connect(&config.database_url).await?;
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("KhoGame/0.1 (Rust)")
            .build()?;
        Ok(Self {
            db,
            config: Arc::new(config),
            http_client,
        })
    }
}
