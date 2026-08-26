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
        // Fail-fast: PgPoolOptions::connect có thể trả Ok dù DB thực sự
        // misconfigured (vd: database sai, postmaster đang restart). Phát
        // `SELECT 1` ngay — nếu fail, crash với message rõ ràng thay vì
        // để mỗi request đầu tiên đều 500.
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&db)
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "DB health check (SELECT 1) failed sau khi connect — \
                     pool mở được nhưng query fail: {e}. \
                     Có thể DATABASE_URL trỏ DB sai, postmaster đang restart, \
                     hoặc migration chưa chạy. Check config + `psql $DATABASE_URL -c '\\dt'`."
                )
            })?;
        tracing::info!("DB health check (SELECT 1) OK");
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
        // Đọc cache trước — nếu còn fresh thì trả ngay không cần lock write.
        {
            let cache = self.maintenance_cache.read().await;
            if cache.1.elapsed() < Duration::from_secs(30) {
                return cache.0;
            }
        }
        // Cache stale — query DB. Nếu nhiều task cùng到这里, mỗi task sẽ
        // query DB (TOCTOU giữa read và write lock). Đây là perf hit nhỏ,
        // không phải correctness bug (mỗi query cho cùng kết quả trong
        // cửa sổ này). Tránh double-check lock để giữ code đơn giản.
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
