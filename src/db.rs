use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::time::Duration;

/// Cấu hình pool đọc từ biến môi trường với giá trị mặc định an toàn cho prod.
///
/// - `DB_MAX_CONNECTIONS` (mặc định 25): số connection tối đa. PostgreSQL 17
///   mặc định cho phép 100 connection; nếu chạy nhiều service chung một
///   cluster thì nên giảm để tránh cạn slot. v2.3.0 tăng từ 15 → 25 để
///   giảm acquire contention khi concurrent request tăng (homepage chạy
///   10 queries song song, đa section cần pool rộng).
/// - `DB_MIN_CONNECTIONS` (mặc định 2): số connection giữ ấm, giảm latency
///   của request đầu tiên sau khi idle.
/// - `DB_ACQUIRE_TIMEOUT_SECS` (mặc định 10): thời gian tối đa chờ một
///   connection rảnh từ pool trước khi trả 500 — tăng nếu có query nặng.
/// - `DB_STATEMENT_TIMEOUT_SECS` (mặc định 15): statement_timeout của mỗi
///   connection trong pool (v2.6.0). Bất kỳ query nào chạy lâu hơn sẽ bị
///   PostgreSQL ngắt → trả lỗi thay vì treo connection vô thời hạn.
///   Giúp pool không bị exhausted khi 1 query nặng (vd: sitemap với 10K
///   rows) chiếm connection mãi. < request_timeout (30s) để handler vẫn
///   kịp trả response lỗi cho user.
#[derive(Debug, Clone)]
struct PoolTuning {
    max_connections: u32,
    min_connections: u32,
    acquire_timeout: Duration,
    statement_timeout: Duration,
}

impl PoolTuning {
    fn from_env() -> Self {
        let max_connections = std::env::var("DB_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(25);
        let min_connections = std::env::var("DB_MIN_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v <= max_connections)
            .unwrap_or(2);
        let acquire_timeout = Duration::from_secs(
            std::env::var("DB_ACQUIRE_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(10),
        );
        // v2.6.0 — statement_timeout ngắn hơn request_timeout (30s) để
        // handler kịp trả lỗi có ý nghĩa cho user thay vì bị outer
        // timeout cắt đứt. Mặc định 15s — đủ cho 99% query thông thường.
        let statement_timeout = Duration::from_secs(
            std::env::var("DB_STATEMENT_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .filter(|v| *v > 0 && *v <= 600)
                .unwrap_or(15),
        );
        Self {
            max_connections,
            min_connections,
            acquire_timeout,
            statement_timeout,
        }
    }
}

/// Kết nối database với retry — chống crash khi Postgres container
/// chưa sẵn sàng lúc app khởi động (cold start race trên Coolify/K8s).
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    let tuning = PoolTuning::from_env();
    tracing::info!(
        "DB pool: max={}, min={}, acquire_timeout={:?}, statement_timeout={:?}",
        tuning.max_connections,
        tuning.min_connections,
        tuning.acquire_timeout,
        tuning.statement_timeout,
    );
    // v2.6.0 — Set statement_timeout trên mỗi connection trong pool thông
    // qua PgConnectOptions. Mọi query vượt quá thời gian sẽ bị server ngắt
    // thay vì treo connection mãi → tránh pool exhaustion dưới load nặng.
    let pg_options: sqlx::postgres::PgConnectOptions = database_url
        .parse::<sqlx::postgres::PgConnectOptions>()
        .map_err(|e| anyhow::anyhow!("DATABASE_URL parse fail: {e}"))?
        .options([(
            "statement_timeout",
            &format!("{}", tuning.statement_timeout.as_millis()),
        )]);
    let mut last_err: Option<anyhow::Error> = None;
    // Thử tối đa 30 lần x 2 giây = 1 phút
    for attempt in 1..=30 {
        match PgPoolOptions::new()
            .max_connections(tuning.max_connections)
            .min_connections(tuning.min_connections)
            .acquire_timeout(tuning.acquire_timeout)
            .idle_timeout(Some(Duration::from_secs(300)))
            .max_lifetime(Some(Duration::from_mins(30)))
            .connect_with(pg_options.clone())
            .await
        {
            Ok(pool) => {
                if attempt > 1 {
                    tracing::info!("Database kết nối sau {} lần thử", attempt);
                }
                return Ok(pool);
            }
            Err(e) => {
                // Cẩn thận: sqlx error Display có thể chứa DATABASE_URL
                // với user:pass@host — log raw sẽ rò rỉ credential vào
                // stdout/log aggregator. FIX v2.8.1: trước đây dùng
                // `split('@').next()` — giữ lại ĐÚNG nửa chứa password
                // (postgres://user:PASS) và vứt mất host. Giờ redact đúng
                // đoạn userinfo giữa "://" và "@".
                let safe_msg = e.to_string();
                let stripped = redact_db_credentials(&safe_msg);
                // Phân biệt retryable vs fatal: connection refused/timeout
                // → retry; auth fail/db doesn't exist → fail fast không
                // có point retry 30 lần chờ 60s.
                if let sqlx::Error::Configuration(_) | sqlx::Error::Database(_) = &e {
                    tracing::error!(
                        "Kết nối DB fail fatal (lần {}): {} — không retry",
                        attempt,
                        stripped
                    );
                    return Err(e.into());
                }
                tracing::warn!(
                    "Kết nối DB lần {} thất bại: {} — thử lại sau 2s",
                    attempt,
                    stripped
                );
                last_err = Some(e.into());
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Không kết nối được database")))
}

/// FIX v2.8.1 — Redact userinfo (user:password) trong chuỗi lỗi có thể
/// chứa DSN kiểu `postgres://user:pass@host:port/db`, giữ lại host để
/// operator vẫn đọc được context. Xử lý NHIỀU lần xuất hiện (error message
/// có thể lặp URL). Không có "://" hoặc "@" → trả nguyên văn.
/// Thao tác trên char boundary (ASCII "://" và "@") → an toàn UTF-8
/// cho message tiếng Việt.
fn redact_db_credentials(msg: &str) -> String {
    let mut out = String::with_capacity(msg.len());
    let mut rest = msg;
    while let Some(pos) = rest.find("://") {
        out.push_str(&rest[..pos]);
        out.push_str("://");
        let after = &rest[pos + 3..];
        // Tìm '@' trong 256 ký tự đầu sau "://" (userinfo thực tế ngắn;
        // tránh nuốt '@' của phần message dài phía sau — VD email)
        let window: String = after.chars().take(256).collect();
        match window.find('@') {
            // Userinfo không rỗng → redact, nhảy qua '@'
            Some(at) if at > 0 => {
                out.push_str("***@");
                rest = &after[at + 1..];
            }
            // Không có '@' hoặc userinfo rỗng ("scheme://@host") →
            // tiếp tục quét từ sau "://"
            _ => rest = after,
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_tuning_defaults() {
        // Không set env → dùng default 25/2/10s/15s statement (test chạy
        // trong process riêng nên an toàn khi đọc env).
        let t = PoolTuning::from_env();
        assert!(t.max_connections > 0);
        assert!(t.min_connections <= t.max_connections);
        assert!(t.acquire_timeout.as_secs() > 0);
        // v2.6.0 — statement_timeout phải > 0 và <= 600s để không vô hiệu
        // hóa bảo vệ pool exhaustion cũng như không ngắt query dài hợp lệ.
        assert!(t.statement_timeout.as_secs() > 0);
        assert!(t.statement_timeout.as_secs() <= 600);
    }

    /// FIX v2.8.1 — redact_db_credentials phải XÓA password và GIỮ host.
    /// Trước đây split('@').next() làm ngược lại mọi thứ (giữ password,
    /// vứt host).
    #[test]
    fn test_redact_db_credentials() {
        assert_eq!(
            redact_db_credentials("postgres://khogame:S3cret@db.local:5432/khogame"),
            "postgres://***@db.local:5432/khogame"
        );
        // Không có userinfo → nguyên văn
        assert_eq!(
            redact_db_credentials("postgres://localhost:5432/khogame"),
            "postgres://localhost:5432/khogame"
        );
        // URL xuất hiện 2 lần trong message — cả hai đều được redact
        assert_eq!(
            redact_db_credentials(
                "fail: postgres://u:p@a/db rồi lại postgres://u2:p2@b/db"
            ),
            "fail: postgres://***@a/db rồi lại postgres://***@b/db"
        );
        // Chuỗi không liên quan đến URL → nguyên văn (kể cả email)
        assert_eq!(
            redact_db_credentials("admin@example.com gửi lỗi thường"),
            "admin@example.com gửi lỗi thường"
        );
    }
}
