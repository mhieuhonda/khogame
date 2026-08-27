use crate::repositories::{NotificationRepo, SessionRepo, StatsRepo};
use crate::state::AppState;
use std::time::Duration;

/// Số ngày giữ notification ĐÃ ĐỌC trước khi xoá. Notification chưa đọc
/// không bao giờ bị xoá để không mất thông báo người dùng chưa kịp xem.
const NOTIFICATION_RETENTION_DAYS: i64 = 90;

/// Số ngày giữ `daily_stats` (chart dashboard chỉ dùng 7 ngày gần nhất).
const DAILY_STATS_RETENTION_DAYS: i64 = 90;

/// Chu kỳ chạy dọn dẹp mặc định (6 giờ). Có thể override qua env
/// `JANITOR_INTERVAL_SECS` (tối thiểu 60s để tránh spam DB khi test).
const DEFAULT_INTERVAL_SECS: u64 = 6 * 3600;

/// v2.2.0 — Chu kỳ gửi email queue (2 phút). Ngắn hơn janitor cleanup
/// để email realtime cho user.
const EMAIL_FLUSH_INTERVAL_SECS: u64 = 120;

/// Số email gửi mỗi batch — giới hạn để không quá tải SMTP trong 1 lần.
const EMAIL_BATCH_SIZE: i64 = 25;

/// Task nền dọn dẹp dữ liệu tạm — chạy suốt vòng đời server.
///
/// Trước đây session hết hạn chỉ được dọn opportunistic khi có người
/// đăng nhập (auth.rs), nên nếu traffic thấp thì bảng sessions phình to
/// vô hạn trên prod. Janitor này đảm bảo dọn định kỳ bất kể traffic:
/// - `sessions` hết hạn (`expires_at` < `NOW()`)
/// - `notifications` đã đọc cũ hơn 90 ngày
/// - `daily_stats` cũ hơn 90 ngày (chart chỉ dùng 7 ngày)
pub async fn run_janitor(state: AppState) {
    let interval = Duration::from_secs(
        std::env::var("JANITOR_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v >= 60)
            .unwrap_or(DEFAULT_INTERVAL_SECS),
    );
    tracing::info!(
        "Janitor khởi động: chu kỳ {} giây, giữ notification {} ngày, daily_stats {} ngày",
        interval.as_secs(),
        NOTIFICATION_RETENTION_DAYS,
        DAILY_STATS_RETENTION_DAYS
    );
    loop {
        // Đo thời gian dọn dẹp — nếu quá lâu (VD có 10M session hết hạn),
        // lần sau sẽ chạy ngay lập tức khi sleep hết, gây DB load cao
        // liên tục. Log duration để admin quan sát; vẫn giữ hành vi cũ
        // (sleep interval giữa các lần) vì dọn dẹp là idempotent.
        let start = std::time::Instant::now();
        let (sessions, notifications, daily_stats) = do_cleanup(&state).await;
        let elapsed = start.elapsed();
        if sessions > 0 || notifications > 0 || daily_stats > 0 {
            tracing::info!(
                "Janitor: đã xoá {} session hết hạn, {} notification cũ, {} dòng daily_stats cũ (mất {:?})",
                sessions,
                notifications,
                daily_stats,
                elapsed
            );
        } else if elapsed > Duration::from_secs(30) {
            // Không xoá gì nhưng tốn >30s — có thể DB chậm hoặc query
            // full-scan thiếu index. Cảnh báo để admin check.
            tracing::warn!(
                "Janitor: dọn dẹp mất {:?} nhưng không xoá gì — có thể DB slow",
                elapsed
            );
        }
        tokio::time::sleep(interval).await;
    }
}

/// v2.2.0 — Email queue flusher. Chạy song song với janitor cleanup,
/// chu kỳ ngắn hơn (2 phút) để email đến user nhanh.
/// Đọc `email_queue` WHERE status='pending' AND next_retry_at <= NOW(),
/// gửi SMTP, đánh dấu 'sent' hoặc 'failed' (retry 3 lần).
pub async fn run_email_flusher(state: AppState) {
    let interval = Duration::from_secs(
        std::env::var("EMAIL_FLUSH_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v >= 30)
            .unwrap_or(EMAIL_FLUSH_INTERVAL_SECS),
    );
    tracing::info!("Email flusher khởi động: chu kỳ {} giây", interval.as_secs());
    loop {
        match crate::services::email::flush_pending(&state.db, EMAIL_BATCH_SIZE).await {
            Ok((sent, failed, skipped)) => {
                if sent > 0 || failed > 0 {
                    tracing::info!(
                        "Email flusher: đã gửi {}, thất bại {}, skip {}",
                        sent, failed, skipped
                    );
                }
            }
            Err(e) => {
                tracing::warn!("Email flusher lỗi: {}", e);
            }
        }
        tokio::time::sleep(interval).await;
    }
}

/// Thực hiện một vòng dọn dẹp, trả về (sessions, notifications, `daily_stats`) đã xoá.
async fn do_cleanup(state: &AppState) -> (u64, u64, u64) {
    let sessions = SessionRepo::cleanup_expired(&state.db)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("Janitor: lỗi dọn sessions: {}", e);
            0
        });
    let notifications =
        NotificationRepo::cleanup_read_older_than(&state.db, NOTIFICATION_RETENTION_DAYS)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("Janitor: lỗi dọn notifications: {}", e);
                0
            });
    let daily_stats = StatsRepo::cleanup_old_daily_stats(&state.db, DAILY_STATS_RETENTION_DAYS)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("Janitor: lỗi dọn daily_stats: {}", e);
            0
        });
    (sessions, notifications, daily_stats)
}

/// v2.2.0 — Test compile-time guards
#[cfg(test)]
mod tests {
    use super::{
        DAILY_STATS_RETENTION_DAYS, DEFAULT_INTERVAL_SECS, EMAIL_BATCH_SIZE,
        EMAIL_FLUSH_INTERVAL_SECS, NOTIFICATION_RETENTION_DAYS,
    };

    /// Compile-time guards: nếu ai đổi hằng số janitor thành giá trị vô lý
    /// (retention âm, interval quá ngắn spam DB) thì build fail ngay.
    const _: () = {
        assert!(NOTIFICATION_RETENTION_DAYS > 0);
        assert!(NOTIFICATION_RETENTION_DAYS < 3650);
        assert!(DAILY_STATS_RETENTION_DAYS >= 7); // phải >= cửa sổ chart 7 ngày
        assert!(DEFAULT_INTERVAL_SECS >= 3600);
        assert!(EMAIL_FLUSH_INTERVAL_SECS >= 30);
        assert!(EMAIL_BATCH_SIZE > 0 && EMAIL_BATCH_SIZE <= 100);
    };
}
