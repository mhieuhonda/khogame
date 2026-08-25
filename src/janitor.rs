use crate::repositories::{NotificationRepo, SessionRepo};
use crate::state::AppState;
use std::time::Duration;

/// Số ngày giữ notification ĐÃ ĐỌC trước khi xoá. Notification chưa đọc
/// không bao giờ bị xoá để không mất thông báo người dùng chưa kịp xem.
const NOTIFICATION_RETENTION_DAYS: i64 = 90;

/// Chu kỳ chạy dọn dẹp mặc định (6 giờ). Có thể override qua env
/// `JANITOR_INTERVAL_SECS` (tối thiểu 60s để tránh spam DB khi test).
const DEFAULT_INTERVAL_SECS: u64 = 6 * 3600;

/// Task nền dọn dẹp dữ liệu tạm — chạy suốt vòng đời server.
///
/// Trước đây session hết hạn chỉ được dọn opportunistic khi có người
/// đăng nhập (auth.rs), nên nếu traffic thấp thì bảng sessions phình to
/// vô hạn trên prod. Janitor này đảm bảo dọn định kỳ bất kể traffic:
/// - `sessions` hết hạn (expires_at < NOW())
/// - `notifications` đã đọc cũ hơn 90 ngày
pub async fn run_janitor(state: AppState) {
    let interval = Duration::from_secs(
        std::env::var("JANITOR_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v >= 60)
            .unwrap_or(DEFAULT_INTERVAL_SECS),
    );
    tracing::info!(
        "Janitor khởi động: chu kỳ {} giây, giữ notification đã đọc {} ngày",
        interval.as_secs(),
        NOTIFICATION_RETENTION_DAYS
    );
    loop {
        // Chạy dọn ngay lần đầu (không đợi trọn chu kỳ) để deploy mới
        // dọn được rác tồn đọng từ các bản trước ngay lập tức.
        let (sessions, notifications) = do_cleanup(&state).await;
        if sessions > 0 || notifications > 0 {
            tracing::info!(
                "Janitor: đã xoá {} session hết hạn, {} notification cũ",
                sessions,
                notifications
            );
        }
        tokio::time::sleep(interval).await;
    }
}

/// Thực hiện một vòng dọn dẹp, trả về (sessions_deleted, notifications_deleted).
async fn do_cleanup(state: &AppState) -> (u64, u64) {
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
    (sessions, notifications)
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_INTERVAL_SECS, NOTIFICATION_RETENTION_DAYS};

    /// Compile-time guards: nếu ai đổi hằng số janitor thành giá trị vô lý
    /// (retention âm, interval quá ngắn spam DB) thì build fail ngay.
    const _: () = {
        assert!(NOTIFICATION_RETENTION_DAYS > 0);
        assert!(NOTIFICATION_RETENTION_DAYS < 3650);
        assert!(DEFAULT_INTERVAL_SECS >= 3600);
    };
}
