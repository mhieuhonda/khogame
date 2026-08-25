use crate::error::AppResult;
use sqlx::PgPool;
use uuid::Uuid;

pub struct SessionRepo;

impl SessionRepo {
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        token_hash: &str,
        user_agent: &str,
        ip: Option<&str>,
        ttl_days: i64,
    ) -> AppResult<Uuid> {
        let id: Uuid = sqlx::query_scalar(
            r"INSERT INTO sessions (user_id, token_hash, user_agent, ip_address, expires_at)
              VALUES ($1, $2, $3, $4, NOW() + ($5 || ' days')::INTERVAL)
              RETURNING id",
        )
        .bind(user_id)
        .bind(token_hash)
        .bind(user_agent)
        .bind(ip)
        .bind(ttl_days.to_string())
        .fetch_one(pool)
        .await?;
        Ok(id)
    }

    pub async fn find_user_by_token(pool: &PgPool, token_hash: &str) -> AppResult<Option<Uuid>> {
        let user_id: Option<Uuid> = sqlx::query_scalar(
            r"SELECT user_id FROM sessions
              WHERE token_hash = $1 AND expires_at > NOW()",
        )
        .bind(token_hash)
        .fetch_optional(pool)
        .await?;
        Ok(user_id)
    }

    pub async fn delete(pool: &PgPool, token_hash: &str) -> AppResult<()> {
        sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
            .bind(token_hash)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn delete_all_for_user(pool: &PgPool, user_id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM sessions WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Xoá các session hết hạn. Trả về số dòng đã xoá (để janitor log).
    pub async fn cleanup_expired(pool: &PgPool) -> AppResult<u64> {
        let res = sqlx::query("DELETE FROM sessions WHERE expires_at < NOW()")
            .execute(pool)
            .await?;
        Ok(res.rows_affected())
    }

    /// Session còn hạn mới nhất kèm thông tin user — cho trang quản trị
    /// phiên. Join users để lấy `username/display_name`, chỉ session CÒN
    /// HẠN (`expires_at` > `NOW()`), sắp xếp theo tạo mới nhất.
    pub async fn list_active(
        pool: &PgPool,
        limit: i64,
    ) -> AppResult<Vec<crate::models::settings::SessionRow>> {
        let rows = sqlx::query_as::<_, crate::models::settings::SessionRow>(
            r"SELECT s.id, s.user_id, u.username, u.display_name,
                s.user_agent, s.ip_address, s.created_at, s.expires_at
              FROM sessions s
              JOIN users u ON u.id = s.user_id
              WHERE s.expires_at > NOW()
              ORDER BY s.created_at DESC
              LIMIT $1",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Xoá 1 session theo id (admin thu hồi phiên cụ thể). Chỉ đếm là
    /// thành công khi dòng tồn tại — trả false nếu id không có.
    pub async fn delete_by_id(pool: &PgPool, id: Uuid) -> AppResult<bool> {
        let res = sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    /// Đếm session còn hạn theo user — badge 'đang hoạt động' trong
    /// trang admin users.
    pub async fn count_active_for_user(pool: &PgPool, user_id: Uuid) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sessions WHERE user_id = $1 AND expires_at > NOW()",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(c)
    }

    /// Danh sách session cho 1 user (admin xem chi tiết user).
    /// Trả về cả session cũ lẫn mới (sắp xếp mới nhất trước).
    /// Limit 50 để không tràn trang khi user có nhiều login history.
    pub async fn list_for_user(
        pool: &PgPool,
        user_id: Uuid,
        limit: i64,
    ) -> AppResult<Vec<crate::models::settings::SessionRow>> {
        let rows = sqlx::query_as::<_, crate::models::settings::SessionRow>(
            r"SELECT s.id, s.user_id, u.username, u.display_name,
                s.user_agent, s.ip_address, s.created_at, s.expires_at
              FROM sessions s
              JOIN users u ON u.id = s.user_id
              WHERE s.user_id = $1
              ORDER BY s.created_at DESC
              LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}
