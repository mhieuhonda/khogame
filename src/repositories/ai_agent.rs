//! Repository cho AI Agent account system.
//!
//! Bao gồm:
//! - [`AiAgentRepo::register`]: tạo tài khoản AI Agent mới (yêu cầu secret).
//! - [`AiAgentRepo::find_by_api_token`]: tra cứu AI Agent theo token hash.
//! - [`AiAgentRepo::list_for_admin`]: danh sách AI Agent cho admin.
//! - [`AiAgentRepo::find_profile_by_user_id`]: lấy hồ sơ AI.
//! - [`AiAgentRepo::update_profile`]: AI tự chỉnh hồ sơ của mình.
//! - [`AiAgentRepo::add_progress`]: AI gửi báo cáo tiến trình.
//! - [`AiAgentRepo::list_progress_recent`]: danh sách báo cáo (cho admin).
//! - [`AiAgentRepo::revoke_token`]: admin thu hồi token.

use crate::error::{AppError, AppResult};
use crate::models::{
    AiAgentProfile, AiAgentToken, AiAgentWithProfile, AiProgressReport, AiProgressReportWithAgent,
    AiTaskStatus, User,
};
use sqlx::PgPool;
use uuid::Uuid;

pub struct AiAgentRepo;

impl AiAgentRepo {
    /// Tạo tài khoản AI Agent mới. Trả về plain API token (chỉ trả 1 lần).
    ///
    /// Bước:
    /// 1. Tạo user với role='`ai_agent`', provider='`ai_agent`'.
    /// 2. Tạo hàng trong `ai_agent_profiles`.
    /// 3. Sinh token (48 bytes), hash SHA-256, lưu vào `ai_agent_tokens`.
    /// 4. Trả về plain token.
    ///
    /// Caller phải verify secret trước khi gọi hàm này.
    #[allow(clippy::too_many_arguments)]
    pub async fn register(
        pool: &PgPool,
        email: &str,
        username: &str,
        display_name: &str,
        bio: Option<&str>,
        avatar_url: Option<&str>,
        model_name: &str,
        vendor: &str,
        version: &str,
        capabilities: &[String],
        privacy_level: &str,
        accent_color: &str,
        token_label: &str,
        ip_address: Option<&str>,
        user_agent: &str,
    ) -> AppResult<String> {
        // Validate display_name + model_name không rỗng
        let display_name = display_name.trim();
        if display_name.is_empty() {
            return Err(AppError::BadRequest(
                "Tên hiển thị không được để trống".into(),
            ));
        }
        let model_name = model_name.trim();
        if model_name.is_empty() {
            return Err(AppError::BadRequest(
                "Tên model không được để trống (vd 'Ox Alpha')".into(),
            ));
        }
        // Validate avatar_url nếu có
        let avatar_url_safe = match avatar_url {
            Some(s) if !s.is_empty() => {
                let lower = s.to_ascii_lowercase();
                if lower.starts_with("http://") || lower.starts_with("https://") {
                    Some(s)
                } else {
                    return Err(AppError::BadRequest(
                        "Avatar URL phải là http:// hoặc https://".into(),
                    ));
                }
            }
            _ => None,
        };

        // Validate privacy_level
        let privacy_level = match privacy_level.to_ascii_lowercase().as_str() {
            "anonymous" => "anonymous",
            "public" | "" => "public",
            _ => {
                return Err(AppError::BadRequest(
                    "privacy_level phải là 'public' hoặc 'anonymous'".into(),
                ))
            }
        };

        // Validate accent_color (hex hoặc rỗng)
        let accent_color = if accent_color.trim().is_empty() {
            "#7c3aed"
        } else if accent_color.starts_with('#')
            && accent_color[1..].chars().all(|c| c.is_ascii_hexdigit())
            && (accent_color.len() == 7 || accent_color.len() == 4)
        {
            accent_color
        } else {
            return Err(AppError::BadRequest(
                "accent_color phải là mã hex (vd '#7c3aed')".into(),
            ));
        };

        // Username: nếu rỗng, tự sinh từ model_name
        let username_final = if username.trim().is_empty() {
            Self::slugify_model(model_name)
        } else {
            username.trim().to_string()
        };
        let username_unique = Self::ensure_unique_username(pool, &username_final).await;

        // Email unique: nếu rỗng, tự sinh
        let email_final = if email.trim().is_empty() {
            format!("ai-{username_unique}@ai-agent.local")
        } else {
            email.trim().to_string()
        };

        // Google_sub: AI không dùng Google, dùng một giá trị giả duy nhất
        // (cột google_sub có UNIQUE constraint). Dùng format "ai_agent:{uuid}".
        let google_sub = format!("ai_agent:{}", Uuid::new_v4());

        let mut tx = pool.begin().await?;

        // 1) Tạo user
        let user: User = sqlx::query_as::<_, User>(
            r"INSERT INTO users (email, username, display_name, avatar_url, bio, google_sub, role, provider)
              VALUES ($1, $2, $3, $4, $5, $6, 'ai_agent', 'ai_agent')
              RETURNING id, email, username, display_name, avatar_url, bio, google_sub,
                role, is_banned, last_seen_at, created_at, updated_at",
        )
        .bind(&email_final)
        .bind(&username_unique)
        .bind(display_name)
        .bind(avatar_url_safe)
        .bind(bio.unwrap_or("").trim())
        .bind(&google_sub)
        .fetch_one(&mut *tx)
        .await?;

        // 2) Tạo profile
        let _profile: AiAgentProfile = sqlx::query_as::<_, AiAgentProfile>(
            r"INSERT INTO ai_agent_profiles
                (user_id, model_name, vendor, version, capabilities, privacy_level, accent_color, verified)
              VALUES ($1, $2, $3, $4, $5, $6, $7, FALSE)
              RETURNING user_id, model_name, vendor, version, capabilities, privacy_level,
                accent_color, verified, last_active_at, created_at, updated_at",
        )
        .bind(user.id)
        .bind(model_name)
        .bind(vendor.trim())
        .bind(version.trim())
        .bind(capabilities)
        .bind(privacy_level)
        .bind(accent_color)
        .fetch_one(&mut *tx)
        .await?;

        // 3) Tạo default preferences
        let _ = sqlx::query(
            "INSERT INTO user_preferences (user_id) VALUES ($1) ON CONFLICT DO NOTHING",
        )
        .bind(user.id)
        .execute(&mut *tx)
        .await;

        // 4) Sinh token dài hạn (48 bytes = 96 hex chars). Plain token chỉ trả 1 lần.
        let plain_token = crate::auth::gen_ai_agent_token();
        let token_hash = crate::auth::hash_token(&plain_token);

        let _token: AiAgentToken = sqlx::query_as::<_, AiAgentToken>(
            r"INSERT INTO ai_agent_tokens (user_id, token_hash, label, ip_address, user_agent)
              VALUES ($1, $2, $3, $4, $5)
              RETURNING id, user_id, token_hash, label, revoked, last_used_at,
                expires_at, ip_address, user_agent, created_at",
        )
        .bind(user.id)
        .bind(&token_hash)
        .bind(token_label)
        .bind(ip_address)
        .bind(user_agent)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(plain_token)
    }

    /// Tìm AI Agent + user theo API token (plain). Trả về (User, profile) nếu
    /// token còn hiệu lực (chưa revoked, chưa expired).
    /// Cập nhật `last_used_at`.
    pub async fn find_by_api_token(
        pool: &PgPool,
        plain_token: &str,
    ) -> AppResult<Option<(User, AiAgentProfile)>> {
        if plain_token.is_empty() {
            return Ok(None);
        }
        let token_hash = crate::auth::hash_token(plain_token);
        let row = sqlx::query_as::<_, User>(
            r"SELECT u.id, u.email, u.username, u.display_name, u.avatar_url, u.bio,
                     u.google_sub, u.role, u.is_banned, u.last_seen_at, u.created_at, u.updated_at
              FROM ai_agent_tokens t
              JOIN users u ON u.id = t.user_id
              WHERE t.token_hash = $1
                AND t.revoked = FALSE
                AND (t.expires_at IS NULL OR t.expires_at > NOW())
                AND u.role = 'ai_agent'
                AND u.is_banned = FALSE",
        )
        .bind(&token_hash)
        .fetch_optional(pool)
        .await?;

        let Some(user) = row else {
            return Ok(None);
        };

        let profile = sqlx::query_as::<_, AiAgentProfile>(
            r"SELECT user_id, model_name, vendor, version, capabilities, privacy_level,
                     accent_color, verified, last_active_at, created_at, updated_at
              FROM ai_agent_profiles WHERE user_id = $1",
        )
        .bind(user.id)
        .fetch_optional(pool)
        .await?;
        // 2 UPDATE best-effort (last_used_at token + last_active_at profile)
        // độc lập — join! chạy đồng thời; lỗi không ảnh hưởng auth flow.
        let (u1, u2) = tokio::join!(
            sqlx::query("UPDATE ai_agent_tokens SET last_used_at = NOW() WHERE token_hash = $1")
                .bind(&token_hash)
                .execute(pool),
            sqlx::query("UPDATE ai_agent_profiles SET last_active_at = NOW() WHERE user_id = $1")
                .bind(user.id)
                .execute(pool),
        );
        let _ = (u1, u2);

        match profile {
            Some(p) => Ok(Some((user, p))),
            None => Ok(None),
        }
    }

    /// Lấy hồ sơ AI Agent theo `user_id` (công khai, dùng cho trang profile).
    pub async fn find_profile_by_user_id(
        pool: &PgPool,
        user_id: Uuid,
    ) -> AppResult<Option<AiAgentProfile>> {
        let p = sqlx::query_as::<_, AiAgentProfile>(
            r"SELECT user_id, model_name, vendor, version, capabilities, privacy_level,
                     accent_color, verified, last_active_at, created_at, updated_at
              FROM ai_agent_profiles WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        Ok(p)
    }

    /// Danh sách AI Agent cho trang admin (kèm profile).
    pub async fn list_for_admin(pool: &PgPool) -> AppResult<Vec<AiAgentWithProfile>> {
        let rows = sqlx::query_as::<_, AiAgentWithProfile>(
            r"SELECT u.id, u.username, u.display_name, u.avatar_url, u.bio,
                     u.is_banned, u.created_at, u.last_seen_at,
                     p.model_name, p.vendor, p.version, p.capabilities,
                     p.privacy_level, p.accent_color, p.verified
              FROM users u
              JOIN ai_agent_profiles p ON p.user_id = u.id
              WHERE u.role = 'ai_agent'
              ORDER BY u.created_at DESC",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Đếm số AI Agent (cho dashboard admin).
    pub async fn count_all(pool: &PgPool) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role = 'ai_agent'")
            .fetch_one(pool)
            .await?;
        Ok(c)
    }

    /// AI Agent tự cập nhật hồ sơ của mình.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_profile(
        pool: &PgPool,
        user_id: Uuid,
        model_name: &str,
        vendor: &str,
        version: &str,
        capabilities: &[String],
        privacy_level: &str,
        accent_color: &str,
        bio: &str,
        avatar_url: Option<&str>,
    ) -> AppResult<AiAgentProfile> {
        // Validate model_name không rỗng
        let model_name = model_name.trim();
        if model_name.is_empty() {
            return Err(AppError::BadRequest("Tên model không được để trống".into()));
        }

        // Validate privacy_level
        let privacy_level = match privacy_level.to_ascii_lowercase().as_str() {
            "anonymous" => "anonymous",
            _ => "public",
        };

        // Validate accent_color
        let accent_color = if accent_color.trim().is_empty() {
            "#7c3aed"
        } else if accent_color.starts_with('#')
            && accent_color[1..].chars().all(|c| c.is_ascii_hexdigit())
            && (accent_color.len() == 7 || accent_color.len() == 4)
        {
            accent_color
        } else {
            return Err(AppError::BadRequest(
                "accent_color phải là mã hex (vd '#7c3aed')".into(),
            ));
        };

        // Cập nhật bảng users (display_name không đổi, chỉ cập nhật bio + avatar_url)
        let avatar_url_safe = match avatar_url {
            Some(s) if !s.is_empty() => {
                let lower = s.to_ascii_lowercase();
                if lower.starts_with("http://") || lower.starts_with("https://") {
                    Some(s)
                } else {
                    return Err(AppError::BadRequest(
                        "Avatar URL phải là http:// hoặc https://".into(),
                    ));
                }
            }
            _ => None,
        };
        sqlx::query(
            r"UPDATE users SET bio = $1, avatar_url = COALESCE($2, avatar_url)
              WHERE id = $3",
        )
        .bind(bio.trim())
        .bind(avatar_url_safe)
        .bind(user_id)
        .execute(pool)
        .await?;

        // Cập nhật bảng ai_agent_profiles
        let profile = sqlx::query_as::<_, AiAgentProfile>(
            r"UPDATE ai_agent_profiles
              SET model_name = $1, vendor = $2, version = $3, capabilities = $4,
                  privacy_level = $5, accent_color = $6
              WHERE user_id = $7
              RETURNING user_id, model_name, vendor, version, capabilities, privacy_level,
                accent_color, verified, last_active_at, created_at, updated_at",
        )
        .bind(model_name)
        .bind(vendor.trim())
        .bind(version.trim())
        .bind(capabilities)
        .bind(privacy_level)
        .bind(accent_color)
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(profile)
    }

    /// Thêm báo cáo tiến trình mới từ AI Agent.
    /// Trả về report đã insert (kèm id).
    #[allow(clippy::too_many_arguments)]
    pub async fn add_progress(
        pool: &PgPool,
        agent_id: Uuid,
        task: &str,
        action: &str,
        percentage: i16,
        status: &AiTaskStatus,
        message: &str,
        metadata: Option<&serde_json::Value>,
        ip_address: Option<&str>,
    ) -> AppResult<AiProgressReport> {
        let task = task.trim();
        if task.is_empty() {
            return Err(AppError::BadRequest("Task không được để trống".into()));
        }
        let percentage = percentage.clamp(0, 100);
        let metadata_json = metadata.cloned().unwrap_or(serde_json::Value::Null);
        let report = sqlx::query_as::<_, AiProgressReport>(
            r"INSERT INTO ai_progress_reports
                (agent_id, task, action, percentage, status, message, metadata, ip_address)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
              RETURNING id, agent_id, task, action, percentage, status, message,
                metadata, ip_address, created_at, updated_at",
        )
        .bind(agent_id)
        .bind(task)
        .bind(action.trim())
        .bind(percentage)
        .bind(status)
        .bind(message.trim())
        .bind(&metadata_json)
        .bind(ip_address)
        .fetch_one(pool)
        .await?;
        Ok(report)
    }

    /// Danh sách báo cáo tiến trình gần đây (kèm thông tin AI) cho trang admin.
    pub async fn list_progress_recent(
        pool: &PgPool,
        limit: i64,
    ) -> AppResult<Vec<AiProgressReportWithAgent>> {
        let rows = sqlx::query_as::<_, AiProgressReportWithAgent>(
            r"SELECT r.id, r.agent_id, r.task, r.action, r.percentage, r.status,
                     r.message, r.metadata, r.ip_address, r.created_at, r.updated_at,
                     u.username AS agent_username,
                     u.display_name AS agent_display_name,
                     u.avatar_url AS agent_avatar_url,
                     p.model_name AS agent_model_name,
                     p.vendor AS agent_vendor
              FROM ai_progress_reports r
              JOIN users u ON u.id = r.agent_id
              LEFT JOIN ai_agent_profiles p ON p.user_id = r.agent_id
              ORDER BY r.created_at DESC
              LIMIT $1",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Danh sách báo cáo tiến trình của một AI Agent cụ thể.
    pub async fn list_progress_for_agent(
        pool: &PgPool,
        agent_id: Uuid,
        limit: i64,
    ) -> AppResult<Vec<AiProgressReport>> {
        let rows = sqlx::query_as::<_, AiProgressReport>(
            r"SELECT id, agent_id, task, action, percentage, status, message,
                     metadata, ip_address, created_at, updated_at
              FROM ai_progress_reports
              WHERE agent_id = $1
              ORDER BY created_at DESC
              LIMIT $2",
        )
        .bind(agent_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Thu hồi token (admin hoặc AI tự thu hồi).
    pub async fn revoke_token(pool: &PgPool, token_hash: &str) -> AppResult<()> {
        sqlx::query("UPDATE ai_agent_tokens SET revoked = TRUE WHERE token_hash = $1")
            .bind(token_hash)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Admin đặt trạng thái verified cho AI Agent (hoặc bỏ verified).
    pub async fn set_verified(pool: &PgPool, user_id: Uuid, verified: bool) -> AppResult<()> {
        sqlx::query("UPDATE ai_agent_profiles SET verified = $1 WHERE user_id = $2")
            .bind(verified)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Sinh username từ `model_name` (slug đơn giản).
    fn slugify_model(model_name: &str) -> String {
        let s: String = model_name
            .trim()
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect::<String>()
            .to_ascii_lowercase();
        if s.is_empty() {
            "ai_agent".to_string()
        } else {
            format!("ai_{s}")
        }
    }

    /// Đảm bảo username không trùng lặp — thêm hậu tố _1, _2, ...
    async fn ensure_unique_username(pool: &PgPool, base: &str) -> String {
        let base = if base.is_empty() {
            "ai_agent".to_string()
        } else {
            base.to_string()
        };
        for i in 0..1000u32 {
            let candidate = if i == 0 {
                base.clone()
            } else {
                format!("{base}_{i}")
            };
            let exists: Option<i32> = sqlx::query_scalar("SELECT 1 FROM users WHERE username = $1")
                .bind(&candidate)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();
            if exists.is_none() {
                return candidate;
            }
        }
        format!("ai_agent_{}", Uuid::new_v4().simple())
    }
}

// Re-export UserRole để caller không phải import riêng
#[allow(unused_imports)]
use crate::models::UserRole as _UserRole;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify_model() {
        // Chữ thường + prefix ai_
        assert_eq!(AiAgentRepo::slugify_model("GPT-4o"), "ai_gpt-4o");
        assert_eq!(AiAgentRepo::slugify_model("Claude"), "ai_claude");
        // Ký tự lạ bị loại
        assert_eq!(AiAgentRepo::slugify_model("Model X!@#"), "ai_modelx");
        // Khoảng trắng bị loại
        assert_eq!(AiAgentRepo::slugify_model("Ox Alpha"), "ai_oxalpha");
        // Rỗng / chỉ ký tự lạ → fallback
        assert_eq!(AiAgentRepo::slugify_model(""), "ai_agent");
        assert_eq!(AiAgentRepo::slugify_model("!!!"), "ai_agent");
        // Tiếng Việt: chữ có dấu là alphanumeric → giữ nguyên lowercase
        let s = AiAgentRepo::slugify_model("Trí Tuệ");
        assert!(s.starts_with("ai_"));
    }

    #[test]
    fn test_slugify_model_no_uppercase_leak() {
        // Không được chứa ký tự hoa (username chuẩn hoá)
        let s = AiAgentRepo::slugify_model("DeepSeek V3");
        assert_eq!(s, "ai_deepseekv3");
        assert!(s.chars().all(|c| !c.is_ascii_uppercase()));
    }
}
