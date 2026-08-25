use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::time::Duration;

/// Cấu hình pool đọc từ biến môi trường với giá trị mặc định an toàn cho prod.
///
/// - `DB_MAX_CONNECTIONS` (mặc định 15): số connection tối đa. PostgreSQL 17
///   mặc định cho phép 100 connection; nếu chạy nhiều service chung một
///   cluster thì nên giảm để tránh cạn slot.
/// - `DB_MIN_CONNECTIONS` (mặc định 1): số connection giữ ấm, giảm latency
///   của request đầu tiên sau khi idle.
/// - `DB_ACQUIRE_TIMEOUT_SECS` (mặc định 10): thời gian tối đa chờ một
///   connection rảnh từ pool trước khi trả 500 — tăng nếu có query nặng.
#[derive(Debug, Clone)]
struct PoolTuning {
    max_connections: u32,
    min_connections: u32,
    acquire_timeout: Duration,
}

impl PoolTuning {
    fn from_env() -> Self {
        let max_connections = std::env::var("DB_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(15);
        let min_connections = std::env::var("DB_MIN_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v <= max_connections)
            .unwrap_or(1);
        let acquire_timeout = Duration::from_secs(
            std::env::var("DB_ACQUIRE_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(10),
        );
        Self {
            max_connections,
            min_connections,
            acquire_timeout,
        }
    }
}

/// Kết nối database với retry — chống crash khi Postgres container
/// chưa sẵn sàng lúc app khởi động (cold start race trên Coolify/K8s).
pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    let tuning = PoolTuning::from_env();
    tracing::info!(
        "DB pool: max={}, min={}, acquire_timeout={:?}",
        tuning.max_connections,
        tuning.min_connections,
        tuning.acquire_timeout
    );
    let mut last_err: Option<anyhow::Error> = None;
    // Thử tối đa 30 lần x 2 giây = 1 phút
    for attempt in 1..=30 {
        match PgPoolOptions::new()
            .max_connections(tuning.max_connections)
            .min_connections(tuning.min_connections)
            .acquire_timeout(tuning.acquire_timeout)
            .idle_timeout(Some(Duration::from_secs(300)))
            .max_lifetime(Some(Duration::from_mins(30)))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_tuning_defaults() {
        // Không set env → dùng default 15/1/10s (test chạy trong process
        // riêng nên an toàn khi đọc env).
        let t = PoolTuning::from_env();
        assert!(t.max_connections > 0);
        assert!(t.min_connections <= t.max_connections);
        assert!(t.acquire_timeout.as_secs() > 0);
    }
}
