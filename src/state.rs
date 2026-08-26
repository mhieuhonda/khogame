use crate::config::AppConfig;
use crate::middleware::RateLimiter;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<AppConfig>,
    pub http_client: reqwest::Client,
    pub rate_limiter: Arc<RateLimiter>,
    /// Cache maintenance mode (làm mới mỗi 30s)
    maintenance_cache: Arc<tokio::sync::RwLock<(bool, std::time::Instant)>>,
}

impl AppState {
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn new(config: AppConfig) -> anyhow::Result<Self> {
        let db = crate::db::connect(&config.database_url).await?;
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            // Connect timeout riêng ngắn hơn — nếu DNS/TCP handshake chậm
            // (5s+), khả năng cao là mạng/registrar bị lỗi, không phải do
            // server target phản hồi chậm. Fail nhanh để error.rs có thể
            // log với delay ngắn, thay vì treo 15s tổng.
            .connect_timeout(Duration::from_secs(5))
            .user_agent(format!("KhoGame/{} (Rust)", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            db,
            config: Arc::new(config),
            http_client,
            rate_limiter: Arc::new(RateLimiter::new()),
            maintenance_cache: Arc::new(tokio::sync::RwLock::new((
                false,
                std::time::Instant::now(),
            ))),
        })
    }

    /// Kiểm tra maintenance mode với cache 30 giây
    pub async fn maintenance_enabled(&self) -> bool {
        {
            let cache = self.maintenance_cache.read().await;
            if cache.1.elapsed() < Duration::from_secs(30) {
                return cache.0;
            }
        }
        let on = crate::repositories::SettingsRepo::get(&self.db, "maintenance_mode")
            .await
            .ok()
            .flatten()
            .is_some_and(|v| v == "on");
        let mut cache = self.maintenance_cache.write().await;
        *cache = (on, std::time::Instant::now());
        on
    }
}
