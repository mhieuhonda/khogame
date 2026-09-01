use crate::error::{AppError, AppResult};
use crate::models::{SocialLinks, User, UserPreference, UserStats, UserWithGameCount};
use sqlx::PgPool;
use uuid::Uuid;

pub struct UserRepo;

impl UserRepo {
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_by_google_sub(pool: &PgPool, sub: &str) -> AppResult<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            r"SELECT id, email, username, display_name, avatar_url, bio, google_sub,
                role, is_banned, last_seen_at, created_at, updated_at,
                signup_ip, signup_ua, last_login_ip, last_login_ua, last_login_at
              FROM users WHERE google_sub = $1",
        )
        .bind(sub)
        .fetch_optional(pool)
        .await?;
        Ok(user)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<User>> {
        // v3.7.0 — LEFT JOIN user_boosts lấy khung avatar đang active
        // (avatar_frame_until > NOW()) — chi phí rẻ (PK index), cho header
        // + mọi trang render khung ngay không cần query thêm.
        let user = sqlx::query_as::<_, User>(
            r"SELECT u.id, u.email, u.username, u.display_name, u.avatar_url, u.bio, u.google_sub,
                u.role, u.is_banned, u.last_seen_at, u.created_at, u.updated_at,
                u.signup_ip, u.signup_ua, u.last_login_ip, u.last_login_ua, u.last_login_at,
                b.avatar_frame AS avatar_frame
              FROM users u
              LEFT JOIN user_boosts b ON b.user_id = u.id
                AND b.avatar_frame IS NOT NULL AND b.avatar_frame_until > NOW() AND NOT b.avatar_frame_disabled
              WHERE u.id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(user)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_by_username(pool: &PgPool, username: &str) -> AppResult<Option<User>> {
        // v3.7.0 — LEFT JOIN user_boosts như find_by_id: trang hồ sơ (query
        // chính của trang) render khung avatar của CHỦ hồ sơ.
        let user = sqlx::query_as::<_, User>(
            r"SELECT u.id, u.email, u.username, u.display_name, u.avatar_url, u.bio, u.google_sub,
                u.role, u.is_banned, u.last_seen_at, u.created_at, u.updated_at,
                u.signup_ip, u.signup_ua, u.last_login_ip, u.last_login_ua, u.last_login_at,
                b.avatar_frame AS avatar_frame
              FROM users u
              LEFT JOIN user_boosts b ON b.user_id = u.id
                AND b.avatar_frame IS NOT NULL AND b.avatar_frame_until > NOW() AND NOT b.avatar_frame_disabled
              WHERE u.username = $1",
        )
        .bind(username)
        .fetch_optional(pool)
        .await?;
        Ok(user)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn create_from_google(
        pool: &PgPool,
        google_sub: &str,
        email: &str,
        name: &str,
        avatar_url: Option<&str>,
        signup_ip: Option<&str>,
        signup_ua: Option<&str>,
    ) -> AppResult<User> {
        let base_username: String = email
            .split('@')
            .next()
            .unwrap_or("user")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .take(20)
            .collect();
        let username = Self::ensure_unique_username(pool, &base_username).await;

        // v2.9.2 FIX (race OAuth callback): Google đôi khi callback 2 lần
        // gần như đồng thời (double redirect / user double-click). Trước đây
        // cả 2 đều thấy username còn trống (check-then-insert) → INSERT
        // thắng/thua: bên thua dính unique violation → AppError::Conflict →
        // user thật nhận 400 "Dữ liệu đã tồn tại" ngay lần đăng nhập đầu.
        // Giờ: unique violation → fetch lại theo google_sub (idempotent —
        // cùng tài khoản đã tạo thành công bởi request song song) hoặc thử
        // username mới với suffix ngẫu nhiên (username bị user khác chiếm).
        // Tối đa 3 lần — vượt quá thì báo lỗi thật (tránh loop vô hạn).
        for attempt in 0..3u32 {
            let candidate = if attempt == 0 {
                username.clone()
            } else {
                let suffix = Uuid::new_v4().simple().to_string();
                format!("{username}_{}", suffix.get(..4).unwrap_or("x"))
            };
            let insert = sqlx::query_as::<_, User>(
                r"INSERT INTO users (email, username, display_name, avatar_url, google_sub,
                      signup_ip, signup_ua, last_login_ip, last_login_ua, last_login_at)
                  VALUES ($1, $2, $3, $4, $5, $6, $7, $6, $7, NOW())
                  RETURNING id, email, username, display_name, avatar_url, bio, google_sub,
                    role, is_banned, last_seen_at, created_at, updated_at,
                    signup_ip, signup_ua, last_login_ip, last_login_ua, last_login_at",
            )
            .bind(email)
            .bind(&candidate)
            .bind(name)
            .bind(avatar_url)
            .bind(google_sub)
            .bind(signup_ip)
            .bind(signup_ua)
            .fetch_one(pool)
            .await;
            match insert {
                Ok(user) => {
                    // Create default preferences
                    let _ = sqlx::query(
                        "INSERT INTO user_preferences (user_id) VALUES ($1) ON CONFLICT DO NOTHING",
                    )
                    .bind(user.id)
                    .execute(pool)
                    .await;
                    return Ok(user);
                }
                Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
                    // Cùng tài khoản Google đã được tạo bởi request song song?
                    // → trả về user đó (idempotent, đăng nhập bình thường).
                    if let Some(existing) = Self::find_by_google_sub(pool, google_sub).await? {
                        return Ok(existing);
                    }
                    // Không — username bị chiếm trong race → thử suffix mới.
                    tracing::warn!(
                        "create_from_google: username '{candidate}' trùng trong race, thử lại (lần {})",
                        attempt + 1
                    );
                }
                Err(e) => return Err(e.into()),
            }
        }
        Err(AppError::Conflict(
            "Không tạo được tài khoản sau nhiều lần thử — vui lòng đăng nhập lại".into(),
        ))
    }

    /// Cập nhật `last_login_ip/ua/at` khi user đăng nhập lại.
    /// Best-effort: lỗi không block login flow.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn record_login(
        pool: &PgPool,
        user_id: Uuid,
        ip: Option<&str>,
        ua: Option<&str>,
    ) -> AppResult<()> {
        sqlx::query(
            r"UPDATE users SET
                last_login_ip = $2,
                last_login_ua = $3,
                last_login_at = NOW()
              WHERE id = $1",
        )
        .bind(user_id)
        .bind(ip)
        .bind(ua)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn update_profile(
        pool: &PgPool,
        id: Uuid,
        display_name: &str,
        bio: &str,
        avatar_url: Option<&str>,
    ) -> AppResult<User> {
        // Validate avatar_url: chỉ cho phép http/https HOẶC /uploads/
        // URL nội bộ (do server sinh khi user upload ảnh). Chặn mọi scheme
        // khác (javascript:, data:, file:) — XSS vector qua <img src>.
        let avatar_url_safe = match avatar_url {
            Some(s) if !s.is_empty() => {
                let lower = s.to_ascii_lowercase();
                if lower.starts_with("http://")
                    || lower.starts_with("https://")
                    || crate::services::storage::is_upload_url(s)
                {
                    Some(s)
                } else {
                    return Err(crate::error::AppError::BadRequest(
                        "Avatar URL phải là http(s):// hoặc /uploads/avatars/...".into(),
                    ));
                }
            }
            _ => None,
        };

        let user = sqlx::query_as::<_, User>(
            r"UPDATE users SET display_name = $1, bio = $2, avatar_url = COALESCE($3, avatar_url)
              WHERE id = $4
              RETURNING id, email, username, display_name, avatar_url, bio, google_sub,
                role, is_banned, last_seen_at, created_at, updated_at,
                signup_ip, signup_ua, last_login_ip, last_login_ua, last_login_at",
        )
        .bind(display_name)
        .bind(bio)
        .bind(avatar_url_safe)
        .bind(id)
        .fetch_one(pool)
        .await?;
        Ok(user)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn update_last_seen(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE users SET last_seen_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn stats(pool: &PgPool, id: Uuid) -> AppResult<UserStats> {
        // 3 COUNT độc lập — join! song song. stats được gọi ở trang hồ sơ
        // HTML (/u/{username}) và API (/api/v1/users/{username}) — giảm
        // 3 round-trip xuống ~1. Mỗi async block mượn pool riêng
        // (PgPool clone nội bộ là Arc nên rẻ).
        let (games_res, followers_res, following_res) = tokio::join!(
            async {
                sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM games WHERE user_id = $1 AND status = 'published'",
                )
                .bind(id)
                .fetch_one(pool)
                .await
            },
            async {
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM follows WHERE followee_id = $1")
                    .bind(id)
                    .fetch_one(pool)
                    .await
            },
            async {
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM follows WHERE follower_id = $1")
                    .bind(id)
                    .fetch_one(pool)
                    .await
            },
        );
        Ok(UserStats {
            games_count: games_res?,
            followers_count: followers_res?,
            following_count: following_res?,
        })
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn get_preferences(pool: &PgPool, user_id: Uuid) -> AppResult<UserPreference> {
        let pref = sqlx::query_as::<_, UserPreference>(
            r"SELECT theme, email_notifications, show_online, language, role_badge_effects
              FROM user_preferences WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        Ok(pref.unwrap_or_default())
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn update_preferences(
        pool: &PgPool,
        user_id: Uuid,
        theme: &str,
        email_notif: bool,
        show_online: bool,
        language: &str,
        role_badge_effects: bool,
    ) -> AppResult<()> {
        sqlx::query(
            r"INSERT INTO user_preferences (user_id, theme, email_notifications, show_online, language, role_badge_effects)
              VALUES ($1, $2, $3, $4, $5, $6)
              ON CONFLICT (user_id) DO UPDATE SET
                theme = EXCLUDED.theme,
                email_notifications = EXCLUDED.email_notifications,
                show_online = EXCLUDED.show_online,
                language = EXCLUDED.language,
                role_badge_effects = EXCLUDED.role_badge_effects",
        )
        .bind(user_id)
        .bind(theme)
        .bind(email_notif)
        .bind(show_online)
        .bind(language)
        .bind(role_badge_effects)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Chỉ đổi theme, giữ nguyên mọi preference khác — dùng cho nút
    /// dark/light toggle gọi mỗi lần user bấm. UPSERT với giá trị mặc
    /// định cho các cột khác nếu row chưa tồn tại (user chưa lưu pref
    /// lần nào).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn update_theme_only(pool: &PgPool, user_id: Uuid, theme: &str) -> AppResult<()> {
        sqlx::query(
            r"INSERT INTO user_preferences (user_id, theme)
              VALUES ($1, $2)
              ON CONFLICT (user_id) DO UPDATE SET theme = EXCLUDED.theme",
        )
        .bind(user_id)
        .bind(theme)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn list_admins(pool: &PgPool) -> AppResult<Vec<User>> {
        // SELECT đủ cột cho FromRow<User> — trước đây thiếu các cột tracking
        // (migration 009) → `query_as::<_, User>` raise ColumnNotFound.
        let users = sqlx::query_as::<_, User>(
            r"SELECT id, email, username, display_name, avatar_url, bio, google_sub,
                role, is_banned, last_seen_at, created_at, updated_at,
                signup_ip, signup_ua, last_login_ip, last_login_ua, last_login_at
              FROM users WHERE role IN ('admin', 'moderator') ORDER BY created_at",
        )
        .fetch_all(pool)
        .await?;
        Ok(users)
    }

    /// Danh sách user cho admin (kèm số game), tìm kiếm theo tên/email
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn list_for_admin(
        pool: &PgPool,
        search: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<UserWithGameCount>> {
        // Escape wildcard + clamp 200 ký tự như search công khai
        let pattern = format!(
            "%{}%",
            crate::utils::escape_like(
                &search
                    .unwrap_or_default()
                    .chars()
                    .take(200)
                    .collect::<String>()
            )
        );
        let users = sqlx::query_as::<_, UserWithGameCount>(
            r"SELECT u.id, u.email, u.username, u.display_name, u.avatar_url, u.bio, u.google_sub,
                u.role, u.is_banned, u.last_seen_at, u.created_at, u.updated_at,
                u.signup_ip, u.signup_ua, u.last_login_ip, u.last_login_ua, u.last_login_at,
                COUNT(g.id) FILTER (WHERE g.status = 'published')::bigint AS games_count
              FROM users u
              LEFT JOIN games g ON g.user_id = u.id
              WHERE ($1 = '%%' OR u.email ILIKE $1 ESCAPE '\' OR u.username ILIKE $1 ESCAPE '\' OR u.display_name ILIKE $1 ESCAPE '\')
              GROUP BY u.id
              ORDER BY u.created_at DESC
              LIMIT $2 OFFSET $3",
        )
        .bind(pattern)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(users)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count_all(pool: &PgPool) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(pool)
            .await?;
        Ok(c)
    }

    /// Username các user có hồ sơ công khai (không ban, không phải AI
    /// Agent) — cho sitemap. Ưu tiên user hoạt động gần đây, tối đa 1000
    /// URL để giữ sitemap dưới giới hạn 50k URL của Google.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn sitemap_usernames(pool: &PgPool) -> AppResult<Vec<String>> {
        let names: Vec<String> = sqlx::query_scalar(
            r"SELECT username FROM users
               WHERE NOT is_banned AND role != 'ai_agent'
               ORDER BY last_seen_at DESC NULLS LAST
               LIMIT 1000",
        )
        .fetch_all(pool)
        .await?;
        Ok(names)
    }

    /// Đếm user theo bộ lọc tìm kiếm (phân trang admin đúng tổng số)
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count_for_admin(pool: &PgPool, search: Option<&str>) -> AppResult<i64> {
        let pattern = format!(
            "%{}%",
            crate::utils::escape_like(
                &search
                    .unwrap_or_default()
                    .chars()
                    .take(200)
                    .collect::<String>()
            )
        );
        let c: i64 = sqlx::query_scalar(
            r"SELECT COUNT(*) FROM users
               WHERE ($1 = '%%' OR email ILIKE $1 ESCAPE '\' OR username ILIKE $1 ESCAPE '\' OR display_name ILIKE $1 ESCAPE '\')",
        )
        .bind(pattern)
        .fetch_one(pool)
        .await?;
        Ok(c)
    }

    /// Tìm user theo email (dùng cho seed admin)
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_by_email(pool: &PgPool, email: &str) -> AppResult<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            r"SELECT id, email, username, display_name, avatar_url, bio, google_sub,
                role, is_banned, last_seen_at, created_at, updated_at,
                signup_ip, signup_ua, last_login_ip, last_login_ua, last_login_at
              FROM users WHERE email = $1",
        )
        .bind(email)
        .fetch_optional(pool)
        .await?;
        Ok(user)
    }

    /// Nâng cấp user lên admin nếu chưa phải (idempotent)
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn ensure_admin_by_email(pool: &PgPool, email: &str) -> AppResult<bool> {
        let res =
            sqlx::query("UPDATE users SET role = 'admin' WHERE email = $1 AND role != 'admin'")
                .bind(email)
                .execute(pool)
                .await?;
        Ok(res.rows_affected() > 0)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn set_role(pool: &PgPool, user_id: Uuid, role: &str) -> AppResult<()> {
        sqlx::query("UPDATE users SET role = $1::user_role WHERE id = $2")
            .bind(role)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn set_banned(pool: &PgPool, user_id: Uuid, banned: bool) -> AppResult<()> {
        sqlx::query("UPDATE users SET is_banned = $1 WHERE id = $2")
            .bind(banned)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Đọc mạng xã hội của user (v2.7.0, migration 019 — bảng
    /// `user_social_links`).
    ///
    /// User chưa từng lưu link (chưa có row) → trả `SocialLinks` rỗng
    /// thay vì lỗi — hồ sơ vẫn phải load được. JSONB rác (sai kiểu) cũng
    /// fail-open thành struct rỗng qua `SocialLinks::from_json_value`.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn social_links(pool: &PgPool, user_id: Uuid) -> AppResult<SocialLinks> {
        let json: Option<serde_json::Value> =
            sqlx::query_scalar("SELECT links FROM user_social_links WHERE user_id = $1")
                .bind(user_id)
                .fetch_optional(pool)
                .await?;
        Ok(json
            .as_ref()
            .map(SocialLinks::from_json_value)
            .unwrap_or_default())
    }

    /// Lưu mạng xã hội của user — UPSERT 1 row (INSERT mới hoặc UPDATE
    /// `links`). Xóa hết link không xóa row (row rỗng `{}` vô hại, tránh
    /// thêm logic DELETE cho case hiếm).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn save_social_links(
        pool: &PgPool,
        user_id: Uuid,
        links: &SocialLinks,
    ) -> AppResult<()> {
        sqlx::query(
            r"INSERT INTO user_social_links (user_id, links, updated_at)
              VALUES ($1, $2, NOW())
              ON CONFLICT (user_id) DO UPDATE SET
                links = EXCLUDED.links,
                updated_at = NOW()",
        )
        .bind(user_id)
        .bind(links.to_json_value())
        .execute(pool)
        .await?;
        Ok(())
    }

    /// v3.8.0 — Trạng thái khung avatar của user (cho nút bật/tắt trên
    /// hồ sơ của chính mình). Trả về `Some((frame_id, is_visible))` khi
    /// user còn hạn khung (dù đang ẩn), `None` khi không có khung.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn avatar_frame_state(
        pool: &PgPool,
        user_id: Uuid,
    ) -> AppResult<Option<(String, bool)>> {
        let row: Option<(String, bool)> = sqlx::query_as(
            r"SELECT avatar_frame, NOT avatar_frame_disabled
               FROM user_boosts
               WHERE user_id = $1 AND avatar_frame IS NOT NULL
                 AND avatar_frame_until > NOW()",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// v3.8.0 — Bật/ẩn khung avatar của user (giữ nguyên thời hạn).
    /// `visible = false` → ẩn khung (avatar render gốc); `true` → đeo lại.
    /// Trả về Ok(false) nếu user không có khung nào đang hiệu lực.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn set_avatar_frame_visible(
        pool: &PgPool,
        user_id: Uuid,
        visible: bool,
    ) -> AppResult<bool> {
        let res = sqlx::query(
            r"UPDATE user_boosts SET avatar_frame_disabled = $2, updated_at = NOW()
               WHERE user_id = $1 AND avatar_frame IS NOT NULL
                 AND avatar_frame_until > NOW()",
        )
        .bind(user_id)
        .bind(!visible)
        .execute(pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    /// v3.12.0 — LẬT trạng thái hiển thị khung avatar bằng 1 statement
    /// `NOT avatar_frame_disabled` (atomic). Handler toggle dùng method này
    /// thay cho read-then-invert qua `set_avatar_frame_visible` — 2 request
    /// song song trước đây cùng đọc giá trị cũ rồi ghi đè nhau (mất 1 lần
    /// toggle); flip SQL-side luôn thấy giá trị mới nhất của row.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn flip_avatar_frame_visible(pool: &PgPool, user_id: Uuid) -> AppResult<bool> {
        let res = sqlx::query(
            r"UPDATE user_boosts SET avatar_frame_disabled = NOT avatar_frame_disabled,
                  updated_at = NOW()
               WHERE user_id = $1 AND avatar_frame IS NOT NULL
                 AND avatar_frame_until > NOW()",
        )
        .bind(user_id)
        .execute(pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    async fn ensure_unique_username(pool: &PgPool, base: &str) -> String {
        let base = if base.is_empty() {
            "user".to_string()
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
        format!("user_{}", Uuid::new_v4().simple())
    }
}

// Re-export to avoid warning if UserRole used elsewhere
#[allow(unused_imports)]
use crate::models::UserRole as _UserRole;
