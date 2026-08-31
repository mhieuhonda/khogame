use crate::error::AppResult;
use crate::models::chat::ChatMessageWithUser;
use sqlx::PgPool;
use uuid::Uuid;

/// Repository cho bảng `chat_messages`.
///
/// Realtime delivery dùng broadcast channel (xem `AppState::chat_tx`) —
/// DB chỉ là backing store cho:
///   - Lịch sử chat khi user mới vào trang chủ (HTTP GET /chat/history)
///   - Admin kiểm duyệt / truy vết
pub struct ChatRepo;

impl ChatRepo {
    /// Thêm 1 tin nhắn mới vào DB. Trả về message đã JOIN với `users`
    /// (sẵn sàng broadcast cho các client khác mà không cần query thêm).
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail hoặc FOREIGN KEY không thoả (user_id không tồn tại).
    pub async fn create(
        db: &PgPool,
        user_id: Uuid,
        content: &str,
        author_ip: Option<&str>,
        author_ua: Option<&str>,
    ) -> AppResult<ChatMessageWithUser> {
        // INSERT ... RETURNING kèm JOIN users — 1 round-trip thay vì 2.
        // Trả sẵn row với username/display_name/avatar_url để caller broadcast
        // ngay mà không cần query lại.
        let row = sqlx::query_as::<_, ChatMessageWithUser>(
            r#"
            WITH inserted AS (
                INSERT INTO chat_messages (user_id, content, author_ip, author_ua)
                VALUES ($1, $2, $3, $4)
                RETURNING id, user_id, content, is_deleted, created_at
            )
            SELECT
                i.id, i.user_id, i.content, i.is_deleted, i.created_at,
                u.username, u.display_name, u.avatar_url,
                u.role::text AS role,
                COALESCE(b.name_glow_until > NOW(), FALSE) AS name_glow,
                CASE WHEN b.avatar_frame_until > NOW() THEN b.avatar_frame END AS avatar_frame
            FROM inserted i
            JOIN users u ON u.id = i.user_id
            LEFT JOIN user_boosts b ON b.user_id = u.id
            "#,
        )
        .bind(user_id)
        .bind(content)
        .bind(author_ip)
        .bind(author_ua)
        .fetch_one(db)
        .await?;
        Ok(row)
    }

    /// Lấy N tin nhắn gần nhất (không bị soft-delete) kèm thông tin author.
    /// Trả về theo thứ tự mới-nhất-cuối (đúng cho `prepend` ở client,
    /// hoặc reverse để render cũ→mới từ trên xuống).
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn recent(db: &PgPool, limit: i64) -> AppResult<Vec<ChatMessageWithUser>> {
        let rows = sqlx::query_as::<_, ChatMessageWithUser>(
            r#"
            SELECT
                m.id, m.user_id, m.content, m.is_deleted, m.created_at,
                u.username, u.display_name, u.avatar_url,
                u.role::text AS role,
                COALESCE(b.name_glow_until > NOW(), FALSE) AS name_glow,
                CASE WHEN b.avatar_frame_until > NOW() THEN b.avatar_frame END AS avatar_frame
            FROM chat_messages m
            JOIN users u ON u.id = m.user_id
            LEFT JOIN user_boosts b ON b.user_id = m.user_id
            WHERE m.is_deleted = FALSE
            ORDER BY m.created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(db)
        .await?;
        Ok(rows)
    }

    /// Đếm số tin nhắn trong ngày (giờ VN) — hiển thị ở header của chat card
    /// ("Đã chat X tin hôm nay") cho cảm giác sống động.
    ///
    /// v2.9.2 FIX: trước đây dùng mốc UTC (`date_trunc('day', ... AT TIME
    /// ZONE 'UTC')`) → "hôm nay" đếm từ 07:00 giờ VN, lệch với phần còn lại
    /// của app (điểm danh/streak theo ngày VN). Giờ dùng chung chuẩn ngày VN
    /// (`utils::SQL_TODAY_START_VN`) không phụ thuộc timezone server.
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn count_today(db: &PgPool) -> AppResult<i64> {
        // SQL động: chỉ nhét hằng SQL_TODAY_START_VN (không input user).
        let sql = format!(
            r#"
            SELECT COUNT(*) FROM chat_messages
            WHERE is_deleted = FALSE
              AND created_at >= {}
            "#,
            crate::utils::SQL_TODAY_START_VN
        );
        let count = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(sql.as_str()))
            .fetch_one(db)
            .await?;
        Ok(count)
    }

    /// Soft-delete 1 tin nhắn (admin/staff). Trả về `true` nếu row thực sự
    /// bị update, `false` nếu không tìm thấy. Client nhận broadcast sẽ
    /// ẩn tin nhắn khỏi UI.
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn soft_delete(db: &PgPool, id: Uuid) -> AppResult<bool> {
        let rows = sqlx::query(
            r#"
            UPDATE chat_messages SET is_deleted = TRUE
            WHERE id = $1 AND is_deleted = FALSE
            "#,
        )
        .bind(id)
        .execute(db)
        .await?
        .rows_affected();
        Ok(rows > 0)
    }
}

#[cfg(test)]
mod tests {
    // ChatRepo test cần DB live — integration test nằm ở ngoài module.
    // Filter tests đơn giản chỉ check SQL compile (đã qua sqlx::query macro).
}
