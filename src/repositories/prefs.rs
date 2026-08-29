//! v3.0.0 — Repository tùy chọn thông báo per-user.
//!
//! Vắng row = mặc định bật mọi loại thông báo in-app + digest TUẦN TẮT
//! (opt-in) — không thay đổi hành vi hiện tại của user cũ.

use crate::error::AppResult;
use crate::models::retention::NotificationPrefs;
use sqlx::PgPool;
use uuid::Uuid;

pub struct PrefsRepo;

impl PrefsRepo {
    /// Lấy prefs (default khi chưa có row).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn get(pool: &PgPool, user_id: Uuid) -> AppResult<NotificationPrefs> {
        let prefs = sqlx::query_as::<_, NotificationPrefs>(
            "SELECT user_id, inapp_follow, inapp_new_game, inapp_review,
                    inapp_mention, weekly_digest
             FROM user_notification_prefs WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        Ok(prefs.unwrap_or_else(|| NotificationPrefs {
            user_id,
            ..NotificationPrefs::default()
        }))
    }

    /// Lưu prefs (upsert).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn save(
        pool: &PgPool,
        user_id: Uuid,
        inapp_follow: bool,
        inapp_new_game: bool,
        inapp_review: bool,
        inapp_mention: bool,
        weekly_digest: bool,
    ) -> AppResult<()> {
        sqlx::query(
            r#"INSERT INTO user_notification_prefs
                 (user_id, inapp_follow, inapp_new_game, inapp_review,
                  inapp_mention, weekly_digest)
               VALUES ($1, $2, $3, $4, $5, $6)
               ON CONFLICT (user_id) DO UPDATE SET
                 inapp_follow = EXCLUDED.inapp_follow,
                 inapp_new_game = EXCLUDED.inapp_new_game,
                 inapp_review = EXCLUDED.inapp_review,
                 inapp_mention = EXCLUDED.inapp_mention,
                 weekly_digest = EXCLUDED.weekly_digest,
                 updated_at = NOW()"#,
        )
        .bind(user_id)
        .bind(inapp_follow)
        .bind(inapp_new_game)
        .bind(inapp_review)
        .bind(inapp_mention)
        .bind(weekly_digest)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Check nhanh 1 loại in-app notification có được phép không.
    /// `kind` ∈ follow | new_game | review | mention. Mặc định TRUE.
    /// # Errors
    /// Trả lỗi khi DB fail (caller nên best-effort).
    pub async fn allows(pool: &PgPool, user_id: Uuid, kind: &str) -> AppResult<bool> {
        let col = match kind {
            "follow" => "inapp_follow",
            "new_game" => "inapp_new_game",
            "review" => "inapp_review",
            "mention" => "inapp_mention",
            _ => return Ok(true),
        };
        // Tên cột từ whitelist match ở trên — không có input user.
        let sql = format!(
            "SELECT COALESCE((SELECT {col} FROM user_notification_prefs
                              WHERE user_id = $1), TRUE)"
        );
        let allowed: bool = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .fetch_one(pool)
            .await?;
        Ok(allowed)
    }
}
