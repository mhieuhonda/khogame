//! Repository cho hệ thống góp ý người dùng (v3.4.0).
//!
//! Feedback khác reports: reports là người dùng báo cáo GAME vi phạm,
//! feedback là góp ý / báo cáo lỗi / bảo mật / nâng cấp / chức năng cho
//! CHÍNH nền tảng — admin xem xét tại `/admin/feedback`.

use crate::error::AppResult;
use crate::models::feedback::{Feedback, FeedbackCategory, FeedbackStatus, FeedbackWithUser};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

pub struct FeedbackRepo;

impl FeedbackRepo {
    /// Tạo góp ý mới + thông báo cho toàn bộ staff (admin + moderator).
    ///
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        category: &FeedbackCategory,
        title: &str,
        body: &str,
        page_url: &str,
    ) -> AppResult<Uuid> {
        let mut tx = pool.begin().await?;
        let id: Uuid = sqlx::query_scalar(
            r"INSERT INTO user_feedback (user_id, category, title, body, page_url)
              VALUES ($1, $2, $3, $4, $5) RETURNING id",
        )
        .bind(user_id)
        .bind(category)
        .bind(title)
        .bind(body)
        .bind(page_url)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;

        // Notification best-effort — SAU commit (audit v2: INSERT trong tx
        // mà fail sẽ đưa tx vào trạng thái aborted → commit fail → toàn bộ
        // góp ý 500 vì lỗi phụ trợ. Ngoài tx: fail chỉ mất thông báo).
        let is_security = matches!(category, FeedbackCategory::Security);
        let notif_title = if is_security {
            "🔐 Có báo cáo BẢO MẬT mới cần xem xét"
        } else {
            "💬 Có góp ý mới từ người dùng"
        };
        let notif_body = format!("{}: {}", category.label(), title);
        let _ = sqlx::query(
            r"INSERT INTO notifications (user_id, type, title, content, link)
               SELECT id, 'feedback_status'::notification_type, $1, $2, $3
               FROM users
               WHERE (CASE WHEN $4 THEN role = 'admin'
                           ELSE role IN ('admin', 'moderator') END)
                 AND NOT is_banned",
        )
        .bind(notif_title)
        .bind(notif_body)
        .bind("/admin/feedback")
        .bind(is_security)
        .execute(pool)
        .await;

        Ok(id)
    }

    /// Số góp ý user gửi trong 24 giờ qua — dùng cho rate-limit
    /// (chặn spam: mỗi user tối đa 10 góp ý / ngày).
    ///
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count_recent_by_user(pool: &PgPool, user_id: Uuid) -> AppResult<i64> {
        let n: i64 = sqlx::query_scalar(
            r"SELECT COUNT(*) FROM user_feedback
               WHERE user_id = $1 AND created_at > NOW() - INTERVAL '24 hours'",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(n)
    }

    /// Lấy 1 feedback theo id (kiểm tra quyền security ở handler).
    ///
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<Feedback>> {
        let f = sqlx::query_as::<_, Feedback>(
            r"SELECT id, user_id, category, title, body, page_url, status,
                      admin_response, handled_by, handled_at, created_at, updated_at
              FROM user_feedback WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(f)
    }

    /// Danh sách feedback cho admin, lọc theo status (None = tất cả).
    /// `include_security = false` (moderator) → loại bỏ góp ý BẢO MẬT.
    ///
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn list_for_admin(
        pool: &PgPool,
        status: Option<FeedbackStatus>,
        limit: i64,
        include_security: bool,
    ) -> AppResult<Vec<FeedbackWithUser>> {
        // 2 query tĩnh (KHÔNG format!) — sqlx lint E0277 chặn dynamic SQL.
        let items = match status {
            Some(s) => {
                sqlx::query_as::<_, FeedbackWithUser>(
                    r"SELECT f.id, f.user_id, f.category, f.title, f.body, f.page_url,
                            f.status, f.admin_response, f.handled_at, f.created_at,
                            u.display_name AS user_display_name, u.username AS user_username,
                            u.avatar_url  AS user_avatar_url
                      FROM user_feedback f
                      JOIN users u ON u.id = f.user_id
                      WHERE f.status = $1 AND ($3 OR f.category != 'security')
                      ORDER BY f.created_at DESC LIMIT $2",
                )
                .bind(s)
                .bind(limit)
                .bind(include_security)
                .fetch_all(pool)
                .await?
            }
            None => {
                sqlx::query_as::<_, FeedbackWithUser>(
                    r"SELECT f.id, f.user_id, f.category, f.title, f.body, f.page_url,
                            f.status, f.admin_response, f.handled_at, f.created_at,
                            u.display_name AS user_display_name, u.username AS user_username,
                            u.avatar_url  AS user_avatar_url
                      FROM user_feedback f
                      JOIN users u ON u.id = f.user_id
                      WHERE $2 OR f.category != 'security'
                      ORDER BY f.created_at DESC LIMIT $1",
                )
                .bind(limit)
                .bind(include_security)
                .fetch_all(pool)
                .await?
            }
        };
        Ok(items)
    }

    /// Góp ý của 1 user ("Góp ý của tôi" trên trang feedback).
    ///
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn list_by_user(
        pool: &PgPool,
        user_id: Uuid,
        limit: i64,
    ) -> AppResult<Vec<FeedbackWithUser>> {
        let items = sqlx::query_as::<_, FeedbackWithUser>(
            r"SELECT f.id, f.user_id, f.category, f.title, f.body, f.page_url,
                    f.status, f.admin_response, f.handled_at, f.created_at,
                    u.display_name AS user_display_name, u.username AS user_username,
                    u.avatar_url  AS user_avatar_url
              FROM user_feedback f
              JOIN users u ON u.id = f.user_id
              WHERE f.user_id = $1
              ORDER BY f.created_at DESC LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(items)
    }

    /// Đếm feedback theo trạng thái cho badge + filter tabs.
    ///
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn counts_by_status(
        pool: &PgPool,
        include_security: bool,
    ) -> AppResult<FeedbackCounts> {
        let rows: Vec<(String, i64)> = sqlx::query(
            r"SELECT status::text, COUNT(*) FROM user_feedback
              WHERE $1 OR category != 'security'
              GROUP BY status",
        )
        .bind(include_security)
        .map(|row: sqlx::postgres::PgRow| {
            use sqlx::Row;
            let s: String = row.get(0);
            let c: i64 = row.get(1);
            (s, c)
        })
        .fetch_all(pool)
        .await?;
        let mut counts = FeedbackCounts::default();
        for (status, count) in rows {
            match status.as_str() {
                "pending" => counts.pending = count,
                "reviewing" => counts.reviewing = count,
                "resolved" => counts.resolved = count,
                "dismissed" => counts.dismissed = count,
                _ => {}
            }
        }
        counts.total = counts.pending + counts.reviewing + counts.resolved + counts.dismissed;
        Ok(counts)
    }

    /// Admin cập nhật trạng thái + phản hồi. Trả về user_id của feedback
    /// (để gửi notification cho người gửi) hoặc None nếu feedback không tồn tại.
    ///
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn update_status(
        pool: &PgPool,
        feedback_id: Uuid,
        admin_id: Uuid,
        status: FeedbackStatus,
        admin_response: &str,
    ) -> AppResult<Option<Uuid>> {
        let user_id: Option<Uuid> = sqlx::query_scalar(
            r"UPDATE user_feedback
               SET status = $1, admin_response = $2, handled_by = $3, handled_at = NOW()
               WHERE id = $4
               RETURNING user_id",
        )
        .bind(status)
        .bind(admin_response)
        .bind(admin_id)
        .bind(feedback_id)
        .fetch_optional(pool)
        .await?;
        Ok(user_id)
    }

    /// Timestamp hiện tại (helper cho test/template).
    #[must_use]
    pub fn now() -> chrono::DateTime<Utc> {
        Utc::now()
    }
}

/// Số lượng feedback theo trạng thái.
#[derive(Debug, Clone, Copy, Default)]
pub struct FeedbackCounts {
    pub pending: i64,
    pub reviewing: i64,
    pub resolved: i64,
    pub dismissed: i64,
    pub total: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counts_default() {
        let c = FeedbackCounts::default();
        assert_eq!(
            (c.pending, c.reviewing, c.resolved, c.dismissed, c.total),
            (0, 0, 0, 0, 0)
        );
    }

    #[test]
    fn test_now_returns_utc() {
        let before = Utc::now();
        let t = FeedbackRepo::now();
        assert!(t >= before, "now() không được trả thời gian quá khứ");
    }
}
