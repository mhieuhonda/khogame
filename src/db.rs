use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::time::Duration;

/// Kết nối database với retry — chống crash khi Postgres container
/// chưa sẵn sàng lúc app khởi động (cold start race trên Coolify/K8s).
pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    let mut last_err: Option<anyhow::Error> = None;
    // Thử tối đa 30 lần x 2 giây = 1 phút
    for attempt in 1..=30 {
        match PgPoolOptions::new()
            .max_connections(15)
            .min_connections(1)
            .acquire_timeout(Duration::from_secs(10))
            .idle_timeout(Some(Duration::from_secs(300)))
            .max_lifetime(Some(Duration::from_secs(1800)))
            .connect(database_url)
            .await
        {
            Ok(pool) => {
                if attempt > 1 {
                    tracing::info!("Database kết nối sau {} lần thử", attempt);
                }
                return Ok(pool);
            }
            Err(e) => {
                tracing::warn!(
                    "Kết nối DB lần {} thất bại: {} — thử lại sau 2s",
                    attempt,
                    e
                );
                last_err = Some(e.into());
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Không kết nối được database")))
}
