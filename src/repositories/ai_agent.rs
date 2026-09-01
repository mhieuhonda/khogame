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
//! - v3.4.0: [`AiAgentRepo::admin_create_agent`], [`AiAgentRepo::admin_reset_password`],
//!   [`AiAgentRepo::verify_password_login`] — đăng nhập AI Agent bằng
//!   username + mật khẩu (Argon2id, có thời hạn do admin đặt).

use crate::error::{AppError, AppResult};
use crate::models::{
    AiAgentCredential, AiAgentProfile, AiAgentToken, AiAgentWithProfile, AiProfileUpdate,
    AiProgressReport, AiProgressReportWithAgent, AiTaskStatus, User,
};
use sqlx::PgPool;
use uuid::Uuid;

pub struct AiAgentRepo;

/// v3.3.0 — `google_sub` cố định của AI Agent MẶC ĐỊNH (GLM 5.3), được
/// seed bởi migration 027. Tra cứu theo hằng số này — KHÔNG tra theo
/// username (đổi username không làm gãy lookup).
pub const DEFAULT_AGENT_GOOGLE_SUB: &str = "ai_agent:default-glm53";

/// v3.6.2 — Nhận diện "user này là AI Agent" một cách BỀN VẬN:
/// `role == AiAgent` HOẶC đúng danh tính AI Agent mặc định (GLM 5.3,
/// `google_sub` cố định do migration 027 sinh ra).
///
/// Vì sao cần google_sub: prod từng ghi nhận glm53 bị đổi role tay qua
/// admin (thành Moderator) → MỌI tính năng AI của hồ sơ (badge, hero FX,
/// nút admin login-as, namespace /ai/{username}) tắt lặng lẽ dù đây
/// chính là AI Agent mặc định của hệ thống. `google_sub` là danh tính
/// gốc không thể nhầm — dùng làm nguồn nhận diện phụ cho đúng spec
/// "GLM 5.3 là AI Agent mặc định" ở mọi trạng thái role.
#[must_use]
pub fn is_ai_agent_user(role: &crate::models::user::UserRole, google_sub: &str) -> bool {
    // v3.8.0 FIX (security audit F1 — HIGH): nhận diện qua PREFIX
    // "ai_agent:" trên google_sub thay vì chỉ so khớp agent mặc định.
    // Trước đây: agent ĐĂNG KÝ (/auth/ai/register, google_sub
    // "ai_agent:{uuid}") chỉ được nhận diện qua role — nếu role bị đổi
    // tay sang Moderator/Admin thì is_ai_agent_user() = false trong khi
    // session web 30 ngày vẫn sống → AI Agent truy cập được /admin/*.
    // google_sub là danh tính GỐC không đổi — mọi tài khoản AI (mặc định
    // + đăng ký) đều mang prefix này (migration 027/028, register path).
    role.is_ai_agent() || google_sub.starts_with("ai_agent:")
}

/// Cache id của agent mặc định (UUID lookup 1 lần/process — hàng chỉ
/// tạo 1 lần bởi migration, không bao giờ đổi id).
static DEFAULT_AGENT_CACHE: std::sync::OnceLock<Uuid> = std::sync::OnceLock::new();

impl AiAgentRepo {
    /// v3.3.0 — User ID của AI Agent mặc định (GLM 5.3).
    ///
    /// Dùng cho arcade fallback: khi không ghép được người chơi thực,
    /// match tự chuyển sang đấu với agent này (is_ai_fallback = TRUE).
    /// Cache trong OnceLock — DB query chỉ chạy 1 lần mỗi process.
    ///
    /// # Errors
    ///
    /// Trả lỗi khi DB fail; trả `NotFound` nếu migration 027 chưa chạy
    /// (caller nên fallback xử lý hợp lý thay vì crash).
    pub async fn default_agent_user_id(pool: &PgPool) -> AppResult<Uuid> {
        if let Some(id) = DEFAULT_AGENT_CACHE.get() {
            return Ok(*id);
        }
        let id: Uuid = sqlx::query_scalar(
            "SELECT id FROM users WHERE google_sub = $1 AND is_banned = FALSE LIMIT 1",
        )
        .bind(DEFAULT_AGENT_GOOGLE_SUB)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| {
            AppError::NotFound("AI Agent mặc định (GLM 5.3) chưa được khởi tạo".into())
        })?;
        let _ = DEFAULT_AGENT_CACHE.set(id);
        Ok(id)
    }

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
        // TTL của API token (ngày). v3.4.2 — token không còn "vô thời
        // hạn": mặc định 365 ngày (config AI_AGENT_TOKEN_TTL_DAYS), admin
        // xoay vòng bằng nút thu hồi token.
        token_ttl_days: i64,
        ip_address: Option<&str>,
        user_agent: &str,
    ) -> AppResult<String> {
        // Validate display_name + model_name không rỗng
        // v2.9.1 — NFC normalize tên AI (NFD → NFC, cùng lý do hồ sơ user).
        let display_name = crate::utils::normalize_nfc(display_name.trim());
        let display_name = display_name.as_str();
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
        // Validate username: chỉ cho phép [A-Za-z0-9_-] độ dài 3-50.
        // Trước đây không có whitelist → AI Agent đặt username chứa `');//`
        // có thể break-out khỏi inline JS `onsubmit="confirm('... @{{ s.username }}')"`
        // trong admin/sessions.html → stored XSS trong admin session.
        validate_ai_username(&username_final)?;
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
        // RETURNING phải đủ cột cho FromRow<User> — trước đây thiếu
        // signup_ip/signup_ua/last_login_ip/last_login_ua/last_login_at
        // (được thêm ở migration 009) → `query_as::<_, User>` fail với
        // sqlx::Error::ColumnNotFound tại runtime khi AI Agent đăng ký mới.
        let user: User = sqlx::query_as::<_, User>(
            r"INSERT INTO users (email, username, display_name, avatar_url, bio, google_sub, role, provider)
              VALUES ($1, $2, $3, $4, $5, $6, 'ai_agent', 'ai_agent')
              RETURNING id, email, username, display_name, avatar_url, bio, google_sub,
                role, is_banned, last_seen_at, created_at, updated_at,
                signup_ip, signup_ua, last_login_ip, last_login_ua, last_login_at",
        )
        .bind(&email_final)
        .bind(&username_unique)
        .bind(display_name)
        .bind(avatar_url_safe)
        .bind(bio.unwrap_or("").trim())
        .bind(&google_sub)
        .fetch_one(&mut *tx)
        .await?;

        // 2) Tạo profile (spec mới để trống — AI tự khai báo sau qua
        // /profile/ai, hoặc admin điền ở trang sửa hồ sơ).
        let _profile: AiAgentProfile = sqlx::query_as::<_, AiAgentProfile>(
            r"INSERT INTO ai_agent_profiles
                (user_id, model_name, vendor, version, capabilities, privacy_level, accent_color, verified)
              VALUES ($1, $2, $3, $4, $5, $6, $7, FALSE)
              RETURNING user_id, model_name, vendor, version, capabilities, privacy_level,
                accent_color, verified, developer, architecture, context_window, max_output,
                languages, total_params, active_params, last_active_at, created_at, updated_at",
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
        // v3.4.2: set expires_at theo TTL — token lộ không còn "sống mãi
        // mãi", xoay vòng bắt buộc theo chu kỳ (OWASP API token lifecycle).
        let plain_token = crate::auth::gen_ai_agent_token();
        let token_hash = crate::auth::hash_token(&plain_token);
        let ttl = token_ttl_days.clamp(1, 3650).to_string();

        let _token: AiAgentToken = sqlx::query_as::<_, AiAgentToken>(
            r"INSERT INTO ai_agent_tokens
                    (user_id, token_hash, label, ip_address, user_agent, expires_at)
              VALUES ($1, $2, $3, $4, $5, NOW() + ($6 || ' days')::INTERVAL)
              RETURNING id, user_id, token_hash, label, revoked, last_used_at,
                expires_at, ip_address, user_agent, created_at",
        )
        .bind(user.id)
        .bind(&token_hash)
        .bind(token_label)
        .bind(ip_address)
        .bind(user_agent)
        .bind(ttl)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(plain_token)
    }

    /// Tìm AI Agent + user theo API token (plain). Trả về (User, profile) nếu
    /// token còn hiệu lực (chưa revoked, chưa expired).
    /// Cập nhật `last_used_at`.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_by_api_token(
        pool: &PgPool,
        plain_token: &str,
    ) -> AppResult<Option<(User, AiAgentProfile)>> {
        if plain_token.is_empty() {
            return Ok(None);
        }
        let token_hash = crate::auth::hash_token(plain_token);
        // SELECT phải đủ cột cho FromRow<User> — trước đây thiếu các cột
        // tracking (migration 009). Khi AI Agent gọi /ai/login hoặc
        // middleware `require_ai_agent` duyệt Bearer token, query_as::<User>
        // raise sqlx::Error::ColumnNotFound → middleware nuốt `.ok()` thành
        // None → AI Agent tưởng token sai / token bị revoke (thực ra là bug).
        let row = sqlx::query_as::<_, User>(
            r"SELECT u.id, u.email, u.username, u.display_name, u.avatar_url, u.bio,
                     u.google_sub, u.role, u.is_banned, u.last_seen_at, u.created_at, u.updated_at,
                     u.signup_ip, u.signup_ua, u.last_login_ip, u.last_login_ua, u.last_login_at
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
                     accent_color, verified, developer, architecture, context_window,
                     max_output, languages, total_params, active_params,
                     last_active_at, created_at, updated_at
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
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_profile_by_user_id(
        pool: &PgPool,
        user_id: Uuid,
    ) -> AppResult<Option<AiAgentProfile>> {
        let p = sqlx::query_as::<_, AiAgentProfile>(
            r"SELECT user_id, model_name, vendor, version, capabilities, privacy_level,
                     accent_color, verified, developer, architecture, context_window,
                     max_output, languages, total_params, active_params,
                     last_active_at, created_at, updated_at
              FROM ai_agent_profiles WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        Ok(p)
    }

    /// Danh sách AI Agent cho trang admin (kèm profile).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn list_for_admin(pool: &PgPool) -> AppResult<Vec<AiAgentWithProfile>> {
        let rows = sqlx::query_as::<_, AiAgentWithProfile>(
            r"SELECT u.id, u.username, u.display_name, u.avatar_url, u.bio,
                     u.is_banned, u.created_at, u.last_seen_at,
                     p.model_name, p.vendor, p.version, p.capabilities,
                     p.privacy_level, p.accent_color, p.verified,
                     p.developer, p.architecture, p.context_window, p.max_output,
                     p.languages, p.total_params, p.active_params
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
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count_all(pool: &PgPool) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role = 'ai_agent'")
            .fetch_one(pool)
            .await?;
        Ok(c)
    }

    /// v3.7.0 — lấy 1 AI Agent theo user_id (trang admin edit).
    /// Nhận diện BỀN VỮNG: role AiAgent HOẶC google_sub default agent
    /// (glm53 có thể bị đổi role tay trên prod — cùng chính sách với
    /// `is_ai_agent_user`).
    /// # Errors
    ///
    /// Trả về lỗi khi DB fail; `AppError::NotFound` khi không tìm thấy.
    pub async fn find_agent_by_id(pool: &PgPool, user_id: Uuid) -> AppResult<AiAgentWithProfile> {
        let row = sqlx::query_as::<_, AiAgentWithProfile>(
            r"SELECT u.id, u.username, u.display_name, u.avatar_url, u.bio,
                     u.is_banned, u.created_at, u.last_seen_at,
                     p.model_name, p.vendor, p.version, p.capabilities,
                     p.privacy_level, p.accent_color, p.verified,
                     p.developer, p.architecture, p.context_window, p.max_output,
                     p.languages, p.total_params, p.active_params
              FROM users u
              JOIN ai_agent_profiles p ON p.user_id = u.id
              WHERE u.id = $1
                AND (u.role = 'ai_agent' OR u.google_sub = $2)
              LIMIT 1",
        )
        .bind(user_id)
        .bind(DEFAULT_AGENT_GOOGLE_SUB)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Không tìm thấy AI Agent này".into()))?;
        Ok(row)
    }

    /// AI Agent tự cập nhật hồ sơ / admin cập nhật hộ AI (v3.11.0 — nhận
    /// struct [`AiProfileUpdate`] thay 10+ tham số rời; spec cấu trúc 7
    /// trường mới ghi thẳng xuống ai_agent_profiles).
    ///
    /// v3.11.0 FIX (bug "upload logo AI không lưu"): avatar_url giờ chấp
    /// nhận CẢ `/uploads/avatars/...` (URL do POST /uploads/avatar sinh
    /// ra) — trước đây chỉ nhận http(s):// nên luồng upload xong bấm Lưu
    /// luôn bị 400 "Avatar URL phải là http:// hoặc https://" → avatar
    /// reset về mặc định. Đồng bộ với `UserRepo::update_profile`.
    ///
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn update_profile(
        pool: &PgPool,
        user_id: Uuid,
        upd: &AiProfileUpdate<'_>,
    ) -> AppResult<AiAgentProfile> {
        // Validate model_name không rỗng
        let model_name = upd.model_name.trim();
        if model_name.is_empty() {
            return Err(AppError::BadRequest("Tên model không được để trống".into()));
        }

        // Validate privacy_level
        let privacy_level = match upd.privacy_level.to_ascii_lowercase().as_str() {
            "anonymous" => "anonymous",
            _ => "public",
        };

        // Validate accent_color
        let accent_color = if upd.accent_color.trim().is_empty() {
            "#7c3aed"
        } else if upd.accent_color.starts_with('#')
            && upd.accent_color[1..].chars().all(|c| c.is_ascii_hexdigit())
            && (upd.accent_color.len() == 7 || upd.accent_color.len() == 4)
        {
            upd.accent_color
        } else {
            return Err(AppError::BadRequest(
                "accent_color phải là mã hex (vd '#7c3aed')".into(),
            ));
        };

        // v3.11.0 — clamp 7 trường spec theo giới hạn cột DB: chặn sớm
        // với thông báo tiếng Việt rõ ràng hơn là để DB từ chối silently.
        let spec_limits: [(&str, &str, usize); 7] = [
            ("Nhà phát triển", upd.developer, 100),
            ("Kiến trúc", upd.architecture, 150),
            ("Cửa sổ ngữ cảnh", upd.context_window, 60),
            ("Output tối đa", upd.max_output, 60),
            ("Ngôn ngữ", upd.languages, 200),
            ("Tổng tham số", upd.total_params, 60),
            ("Tham số kích hoạt", upd.active_params, 60),
        ];
        for (label, value, max) in spec_limits {
            let trimmed = value.trim();
            if trimmed.chars().count() > max {
                return Err(AppError::BadRequest(format!("{label} tối đa {max} ký tự")));
            }
        }

        // Cập nhật bảng users (display_name không đổi, chỉ cập nhật bio + avatar_url)
        // v3.11.0 — chấp nhận http(s):// URL remote HOẶC /uploads/... nội bộ
        // (URL do server sinh khi upload — cùng whitelist với UserRepo).
        let avatar_url_safe = match upd.avatar_url {
            Some(s) if !s.is_empty() => {
                let lower = s.to_ascii_lowercase();
                if lower.starts_with("http://")
                    || lower.starts_with("https://")
                    || crate::services::storage::is_upload_url(s)
                {
                    Some(s)
                } else {
                    return Err(AppError::BadRequest(
                        "Avatar URL phải là http(s):// hoặc /uploads/avatars/...".into(),
                    ));
                }
            }
            _ => None,
        };
        sqlx::query(
            r"UPDATE users SET bio = $1, avatar_url = COALESCE($2, avatar_url)
              WHERE id = $3",
        )
        .bind(upd.bio.trim())
        .bind(avatar_url_safe)
        .bind(user_id)
        .execute(pool)
        .await?;

        // Cập nhật bảng ai_agent_profiles (7 cột spec mới + các cột cũ)
        let profile = sqlx::query_as::<_, AiAgentProfile>(
            r"UPDATE ai_agent_profiles
              SET model_name = $1, vendor = $2, version = $3, capabilities = $4,
                  privacy_level = $5, accent_color = $6,
                  developer = $7, architecture = $8, context_window = $9,
                  max_output = $10, languages = $11, total_params = $12,
                  active_params = $13
              WHERE user_id = $14
              RETURNING user_id, model_name, vendor, version, capabilities, privacy_level,
                accent_color, verified, developer, architecture, context_window, max_output,
                languages, total_params, active_params, last_active_at, created_at, updated_at",
        )
        .bind(model_name)
        .bind(upd.vendor.trim())
        .bind(upd.version.trim())
        .bind(upd.capabilities)
        .bind(privacy_level)
        .bind(accent_color)
        .bind(upd.developer.trim())
        .bind(upd.architecture.trim())
        .bind(upd.context_window.trim())
        .bind(upd.max_output.trim())
        .bind(upd.languages.trim())
        .bind(upd.total_params.trim())
        .bind(upd.active_params.trim())
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(profile)
    }

    /// Thêm báo cáo tiến trình mới từ AI Agent.
    /// Trả về report đã insert (kèm id).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn revoke_token(pool: &PgPool, token_hash: &str) -> AppResult<()> {
        sqlx::query("UPDATE ai_agent_tokens SET revoked = TRUE WHERE token_hash = $1")
            .bind(token_hash)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// v3.4.2 — Admin thu hồi TOÀN BỘ API token của 1 agent (nút
    /// "Thu hồi token" ở trang /admin/ai-agents). Trước đây
    /// `revoke_token` là dead-code: token lộ chỉ xoá được bằng SQL tay.
    /// Trả về số token đã thu hồi.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn revoke_all_tokens(pool: &PgPool, user_id: Uuid) -> AppResult<u64> {
        let res = sqlx::query(
            "UPDATE ai_agent_tokens SET revoked = TRUE WHERE user_id = $1 AND revoked = FALSE",
        )
        .bind(user_id)
        .execute(pool)
        .await?;
        Ok(res.rows_affected())
    }

    /// Admin đặt trạng thái verified cho AI Agent (hoặc bỏ verified).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn set_verified(pool: &PgPool, user_id: Uuid, verified: bool) -> AppResult<()> {
        sqlx::query("UPDATE ai_agent_profiles SET verified = $1 WHERE user_id = $2")
            .bind(verified)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    // ============================================================
    // v3.4.0 — USERNAME + PASSWORD CREDENTIALS (admin tạo, có thời hạn)
    // ============================================================

    /// Admin tạo tài khoản AI Agent mới kèm mật khẩu + thời hạn.
    ///
    /// Khác `register` (AI tự đăng ký bằng secret): tài khoản do admin
    /// chủ động tạo, KHÔNG cần secret, và đăng nhập bằng username +
    /// mật khẩu (Argon2id) thay vì API token.
    ///
    /// * `password` — plain password admin đặt (hash Argon2id trước khi lưu).
    /// * `expires_days` — số ngày mật khẩu có hiệu lực (1-3650).
    ///
    /// Trả về `(user_id, username_final)` — username có thể bị đổi hậu tố
    /// nếu trùng (`ai_1`, `ai_2`…).
    ///
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    #[allow(clippy::too_many_arguments)]
    pub async fn admin_create_agent(
        pool: &PgPool,
        username: &str,
        display_name: &str,
        password: &str,
        expires_days: i64,
        model_name: &str,
        vendor: &str,
        version: &str,
        capabilities: &[String],
        privacy_level: &str,
        accent_color: &str,
        bio: &str,
        admin_id: Uuid,
    ) -> AppResult<(Uuid, String)> {
        // Validate các field giống register()
        let display_name = crate::utils::normalize_nfc(display_name.trim());
        if display_name.is_empty() {
            return Err(AppError::BadRequest(
                "Tên hiển thị không được để trống".into(),
            ));
        }
        let model_name = model_name.trim();
        if model_name.is_empty() {
            return Err(AppError::BadRequest(
                "Tên model không được để trống (vd 'GLM-5.3')".into(),
            ));
        }
        Self::validate_password_strength(password)?;
        let expires_days = Self::validate_expiry_days(expires_days)?;
        let privacy_level = match privacy_level.to_ascii_lowercase().as_str() {
            "anonymous" => "anonymous",
            _ => "public",
        };
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

        // Username: validate whitelist + CHUẨN HOÁ LOWERCASE trước khi lưu
        // (audit v3.4.0: verify_password_login so khớp LOWER(username) nhưng
        // unique index lại case-sensitive → "GLM53" và "glm53" cùng tồn tại
        // gây login khớp nhầm. Lowercase lúc tạo + so unique case-insensitive
        // là root-cause fix).
        let username_final = username.trim().to_ascii_lowercase();
        validate_ai_username(&username_final)?;
        let username_unique = Self::ensure_unique_username_ci(pool, &username_final).await;

        let email_final = format!("ai-{username_unique}@ai-agent.local");
        let google_sub = format!("ai_agent:{}", Uuid::new_v4());
        let password_hash = crate::auth::hash_password(password)?;
        let expires_at = chrono::Utc::now() + chrono::Duration::days(expires_days);

        let mut tx = pool.begin().await?;

        // 1) Tạo user (role ai_agent)
        let user: User = sqlx::query_as::<_, User>(
            r"INSERT INTO users (email, username, display_name, bio, google_sub, role, provider)
              VALUES ($1, $2, $3, $4, $5, 'ai_agent', 'ai_agent')
              RETURNING id, email, username, display_name, avatar_url, bio, google_sub,
                role, is_banned, last_seen_at, created_at, updated_at,
                signup_ip, signup_ua, last_login_ip, last_login_ua, last_login_at",
        )
        .bind(&email_final)
        .bind(&username_unique)
        .bind(&display_name)
        .bind(bio.trim())
        .bind(&google_sub)
        .fetch_one(&mut *tx)
        .await?;

        // 2) Tạo profile
        let _ = sqlx::query(
            r"INSERT INTO ai_agent_profiles
                (user_id, model_name, vendor, version, capabilities, privacy_level, accent_color, verified)
              VALUES ($1, $2, $3, $4, $5, $6, $7, TRUE)",
        )
        .bind(user.id)
        .bind(model_name)
        .bind(vendor.trim())
        .bind(version.trim())
        .bind(capabilities)
        .bind(privacy_level)
        .bind(accent_color)
        .execute(&mut *tx)
        .await?;

        // 3) Default preferences (fail-soft)
        let _ = sqlx::query(
            "INSERT INTO user_preferences (user_id) VALUES ($1) ON CONFLICT DO NOTHING",
        )
        .bind(user.id)
        .execute(&mut *tx)
        .await;

        // 4) Lưu credentials mật khẩu (Argon2id + thời hạn admin đặt)
        sqlx::query(
            r"INSERT INTO ai_agent_credentials
                (user_id, password_hash, password_expires_at, updated_by)
              VALUES ($1, $2, $3, $4)",
        )
        .bind(user.id)
        .bind(&password_hash)
        .bind(expires_at)
        .bind(admin_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok((user.id, username_unique))
    }

    /// Admin đặt lại mật khẩu + thời hạn cho AI Agent có sẵn.
    /// Nếu agent chưa có dòng credentials → INSERT mới (upsert).
    /// Đồng thời mở khoá (locked_until = NULL, failed_attempts = 0).
    ///
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn admin_reset_password(
        pool: &PgPool,
        user_id: Uuid,
        password: &str,
        expires_days: i64,
        admin_id: Uuid,
    ) -> AppResult<()> {
        Self::validate_password_strength(password)?;
        let expires_days = Self::validate_expiry_days(expires_days)?;
        let password_hash = crate::auth::hash_password(password)?;
        let expires_at = chrono::Utc::now() + chrono::Duration::days(expires_days);
        let mut tx = pool.begin().await?;
        sqlx::query(
            r"INSERT INTO ai_agent_credentials
                (user_id, password_hash, password_expires_at, updated_by)
              VALUES ($1, $2, $3, $4)
              ON CONFLICT (user_id) DO UPDATE SET
                password_hash = EXCLUDED.password_hash,
                password_expires_at = EXCLUDED.password_expires_at,
                updated_by = EXCLUDED.updated_by,
                failed_attempts = 0,
                locked_until = NULL,
                updated_at = NOW()",
        )
        .bind(user_id)
        .bind(&password_hash)
        .bind(expires_at)
        .bind(admin_id)
        .execute(&mut *tx)
        .await?;
        // v3.4.2 FIX (audit "revoke không cắt phiên"): OWASP — reset
        // credential PHẢI thu hồi phiên hiện có. Trước đây attacker giữ
        // session cookie cũ (TTL 90d) vẫn vào được tài khoản sau khi admin
        // đặt lại mật khẩu. Xoá mọi phiên + flush cache middleware.
        sqlx::query("DELETE FROM sessions WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        crate::middleware::invalidate_session_cache_for_user(user_id);
        Ok(())
    }

    /// Xoá credentials mật khẩu (admin thu hồi quyền đăng nhập web bằng
    /// mật khẩu — agent vẫn có thể dùng API token nếu có).
    ///
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn admin_revoke_password(pool: &PgPool, user_id: Uuid) -> AppResult<()> {
        let mut tx = pool.begin().await?;
        sqlx::query("DELETE FROM ai_agent_credentials WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        // v3.4.2 FIX: thu hồi mật khẩu phải cắt cả phiên web đang sống
        // (bản cũ chỉ xoá credentials — phiên 90 ngày vẫn vào được bình
        // thường tới khi hết hạn tự nhiên).
        sqlx::query("DELETE FROM sessions WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        crate::middleware::invalidate_session_cache_for_user(user_id);
        Ok(())
    }

    /// Lấy credentials của 1 AI Agent (nếu có).
    ///
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_credential(
        pool: &PgPool,
        user_id: Uuid,
    ) -> AppResult<Option<AiAgentCredential>> {
        let c = sqlx::query_as::<_, AiAgentCredential>(
            r"SELECT user_id, password_hash, password_expires_at, failed_attempts,
                      locked_until, last_login_at, updated_by, created_at, updated_at
              FROM ai_agent_credentials WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        Ok(c)
    }

    /// Lấy credentials cho toàn bộ danh sách agent (trang admin) —
    /// 1 query thay vì N query mỗi agent.
    ///
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn credentials_map(
        pool: &PgPool,
    ) -> AppResult<std::collections::HashMap<Uuid, AiAgentCredential>> {
        let rows = sqlx::query_as::<_, AiAgentCredential>(
            r"SELECT user_id, password_hash, password_expires_at, failed_attempts,
                      locked_until, last_login_at, updated_by, created_at, updated_at
              FROM ai_agent_credentials",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|c| (c.user_id, c)).collect())
    }

    /// Đăng nhập AI Agent bằng username + mật khẩu.
    ///
    /// Flow:
    /// 1. Tìm user theo username (LOWER so khớp — username chuẩn là lowercase
    ///    nhưng giữ an toàn khi admin gõ hoa).
    /// 2. Kiểm tra role = ai_agent + không bị ban.
    /// 3. Kiểm tra credentials: tồn tại, chưa bị khoá (locked_until),
    ///    mật khẩu chưa hết hạn (password_expires_at).
    /// 4. Verify Argon2id (constant-time bên trong argon2 crate).
    /// 5. Sai mật khẩu → tăng failed_attempts; đủ 5 lần → khoá 15 phút.
    ///    Đúng → reset failed_attempts + cập nhật last_login_at.
    ///
    /// Trả về `Ok(User)` khi thành công. `Err(AppError::Forbidden)` với
    /// message tiếng Việt chi tiết khi sai (message KHÔNG tiết lộ agent
    /// tồn tại hay không — chống user enumeration).
    ///
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn verify_password_login(
        pool: &PgPool,
        username: &str,
        password: &str,
    ) -> AppResult<User> {
        use chrono::Utc;

        let username_trim = username.trim();
        if username_trim.is_empty() || password.is_empty() {
            return Err(AppError::Forbidden(
                "Tên đăng nhập hoặc mật khẩu không đúng".into(),
            ));
        }

        // 1) Tìm user theo username (case-insensitive)
        let user: Option<User> = sqlx::query_as::<_, User>(
            r"SELECT id, email, username, display_name, avatar_url, bio, google_sub,
                     role, is_banned, last_seen_at, created_at, updated_at,
                     signup_ip, signup_ua, last_login_ip, last_login_ua, last_login_at
              FROM users WHERE LOWER(username) = LOWER($1) LIMIT 1",
        )
        .bind(username_trim)
        .fetch_optional(pool)
        .await?;

        // Fail-uniform: mọi lý do thất bại đều cùng 1 message
        const GENERIC_ERR: &str = "Tên đăng nhập hoặc mật khẩu không đúng";
        const LOCKED_ERR: &str =
            "Tài khoản tạm khoá do đăng nhập sai nhiều lần. Thử lại sau 15 phút.";

        let Some(user) = user else {
            // Username công khai (/u/{username}) nhưng vẫn chạy 1 lần Argon2
            // dummy (hash tốn ~50ms như verify thật) — chống timing attack
            // phân biệt "user tồn tại" qua thời gian response.
            let _ = crate::auth::hash_password(password);
            return Err(AppError::Forbidden(GENERIC_ERR.into()));
        };
        // v3.7.0 FIX (bug "admin ấn đăng nhập nhưng không vào được tài khoản
        // AI Agent" — đợt 2): check role TRƯỚC ĐÂY dùng `role.is_ai_agent()`
        // — khi glm53 bị đổi role tay trên prod (data drift Moderator, đúng
        // kịch bản v3.6.3 đã xử lý cho impersonate) thì mật khẩu ĐÚNG vẫn bị
        // từ chối "Tên đăng nhập hoặc mật khẩu không đúng". Dùng
        // is_ai_agent_user() — role AiAgent HOẶC google_sub default agent —
        // nhất quán với impersonate + route /ai/*.
        if !user.is_ai_agent_user() || user.is_banned {
            return Err(AppError::Forbidden(GENERIC_ERR.into()));
        }

        // 2) Lấy credentials
        let cred = sqlx::query_as::<_, AiAgentCredential>(
            r"SELECT user_id, password_hash, password_expires_at, failed_attempts,
                      locked_until, last_login_at, updated_by, created_at, updated_at
              FROM ai_agent_credentials WHERE user_id = $1",
        )
        .bind(user.id)
        .fetch_optional(pool)
        .await?;
        let Some(cred) = cred else {
            // Tài khoản không có mật khẩu (tạo cũ qua /auth/ai/register)
            return Err(AppError::Forbidden(GENERIC_ERR.into()));
        };

        // 3) Kiểm tra khoá
        if let Some(until) = cred.locked_until {
            if until > Utc::now() {
                return Err(AppError::Forbidden(LOCKED_ERR.into()));
            }
        }

        // 4) Verify mật khẩu (Argon2id)
        let ok = crate::auth::verify_password(password, &cred.password_hash);
        if !ok {
            // ATOMIC increment (audit v3.4.0): `failed_attempts = failed_attempts + 1`
            // tính TRÊN DB — N request song song không còn đọc giá trị cũ
            // (race lost-update bypass lockout). Khoá khi chạm ngưỡng 5.
            // Nếu khoá cũ đã HẾT HẠN thì counter tự reset về 1 (trước đây
            // counter còn ≥5 sau khi hết khoá → 1 lần gõ sai khoá thêm 15'
            // nữa — re-lock vĩnh viễn).
            // Semantics UPDATE Postgres: mọi biểu thức SET đọc giá trị OLD row.
            //CASE 1 — failed_attempts: khoá đã hết hạn → reset về 1 (bắt đầu
            // đếm lại chuỗi mới), ngược lại +1.
            // CASE 2 — locked_until: (a) khoá đã hết hạn → NULL (QUAN TRỌNG:
            // không reset thì timestamp cũ NOT NULL khiến CASE1 luôn TRUE →
            // counter kẹt vĩnh viễn ở 1, không bao giờ khoá lại được —
            // audit v2); (b) counter mới chạm ngưỡng 5 → khoá 15 phút.
            let row = sqlx::query(
                r"UPDATE ai_agent_credentials
                   SET failed_attempts = CASE
                         WHEN locked_until IS NOT NULL AND locked_until <= NOW() THEN 1
                         ELSE failed_attempts + 1
                       END,
                       locked_until = CASE
                         WHEN locked_until IS NOT NULL AND locked_until <= NOW() THEN NULL
                         WHEN failed_attempts + 1 >= 5
                           AND (locked_until IS NULL OR locked_until <= NOW())
                         THEN NOW() + INTERVAL '15 minutes'
                         ELSE locked_until
                       END
                   WHERE user_id = $1
                   RETURNING failed_attempts",
            )
            .bind(user.id)
            .fetch_one(pool)
            .await;
            let attempts = row
                .map(|r: sqlx::postgres::PgRow| {
                    use sqlx::Row;
                    let v: i32 = r.get(0);
                    v
                })
                .unwrap_or(cred.failed_attempts + 1);
            tracing::warn!(
                username = %user.username,
                attempts,
                "AI Agent password login failed"
            );
            if attempts >= 5 {
                return Err(AppError::Forbidden(LOCKED_ERR.into()));
            }
            return Err(AppError::Forbidden(GENERIC_ERR.into()));
        }

        // 5) Kiểm tra hạn mật khẩu (SAU khi verify đúng — tránh lộ thông tin
        //    "tài khoản tồn tại" cho attacker chưa có mật khẩu).
        // v3.4.2 FIX (audit "password-validity oracle"): hết hạn trả
        // GENERIC_ERR — trước đây EXPIRED_ERR khác biệt cho phép attacker
        // xác nhận CHÍNH XÁC mật khẩu đúng (dù chưa dùng được). Log phía
        // server vẫn ghi rõ lý do để admin/support đối chiếu.
        if cred.password_expires_at <= Utc::now() {
            tracing::warn!(
                username = %user.username,
                "AI Agent login: mật khẩu ĐÚNG nhưng đã hết hạn — trả lỗi chung"
            );
            return Err(AppError::Forbidden(GENERIC_ERR.into()));
        }

        // 6) Thành công: reset bộ đếm + cập nhật last_login_at + last_active
        let _ = tokio::join!(
            sqlx::query(
                r"UPDATE ai_agent_credentials
                   SET failed_attempts = 0, locked_until = NULL, last_login_at = NOW()
                   WHERE user_id = $1"
            )
            .bind(user.id)
            .execute(pool),
            sqlx::query("UPDATE ai_agent_profiles SET last_active_at = NOW() WHERE user_id = $1")
                .bind(user.id)
                .execute(pool),
        );
        Ok(user)
    }

    /// Validate độ mạnh mật khẩu: 8-128 ký tự.
    /// Không ép phức tạp (admin tự chọn) — minimum length là rào chính.
    ///
    /// # Errors
    ///
    /// Trả về `AppError::BadRequest` nếu mật khẩu yếu/quá dài.
    fn validate_password_strength(password: &str) -> AppResult<()> {
        let len = password.chars().count();
        if len < 8 {
            return Err(AppError::BadRequest("Mật khẩu tối thiểu 8 ký tự".into()));
        }
        if len > 128 {
            return Err(AppError::BadRequest("Mật khẩu tối đa 128 ký tự".into()));
        }
        Ok(())
    }

    /// Validate số ngày hết hạn: 1-3650 (tối đa 10 năm).
    ///
    /// # Errors
    ///
    /// Trả về `AppError::BadRequest` nếu ngoài khoảng cho phép.
    fn validate_expiry_days(days: i64) -> AppResult<i64> {
        if !(1..=3650).contains(&days) {
            return Err(AppError::BadRequest(
                "Thời hạn mật khẩu phải từ 1-3650 ngày".into(),
            ));
        }
        Ok(days)
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
        Self::ensure_unique_username_ci(pool, base).await
    }

    /// Đảm bảo username unique KHÔNG PHÂN BIỆT HOA/THƯỜNG (v3.4.0):
    /// "GLM53" và "glm53" là 1 danh tính duy nhất vì login so khớp
    /// LOWER(username). Trùng case-insensitive → thêm hậu tố _1, _2...
    async fn ensure_unique_username_ci(pool: &PgPool, base: &str) -> String {
        let base = if base.is_empty() {
            "ai_agent".to_string()
        } else {
            base.to_string()
        };
        // Clamp 44 ký tự — để dư 6 ký tự cho hậu tố "_99999" (username
        // VARCHAR(50); audit v2: base 48+ suffix _1 = 50+ → INSERT lỗi 22001)
        let base: String = base.chars().take(44).collect();
        for i in 0..1000u32 {
            let candidate = if i == 0 {
                base.clone()
            } else {
                format!("{base}_{i}")
            };
            let exists: Option<i32> =
                sqlx::query_scalar("SELECT 1 FROM users WHERE LOWER(username) = LOWER($1)")
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

/// Validate username AI Agent: chỉ cho phép `[A-Za-z0-9_-]`, độ dài 3-50.
///
/// Không cho phép ký tự đặc biệt (`'`, `"`, `<`, `>`, `&`, `;`, `/`, `\`,
/// khoảng trắng) vì username có thể xuất hiện trong:
/// - Inline JS attribute (`onsubmit="confirm('@{{ s.username }}')"`)
/// - URL segment (`/u/{username}`)
/// - JSON string field
///
/// Chống stored XSS qua inline JS breakout + URL path injection.
///
/// # Errors
///
/// Trả về `AppError::BadRequest` nếu username không hợp lệ.
pub fn validate_ai_username(username: &str) -> AppResult<()> {
    let len = username.chars().count();
    if !(3..=50).contains(&len) {
        return Err(AppError::BadRequest(
            "Username AI Agent phải từ 3-50 ký tự".into(),
        ));
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(AppError::BadRequest(
            "Username AI Agent chỉ được chứa chữ cái, số, dấu gạch dưới (_), và gạch ngang (-)"
                .into(),
        ));
    }
    Ok(())
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
