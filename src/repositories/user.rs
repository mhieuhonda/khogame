use crate::error::AppResult;
use crate::models::UserWithGameCount;
use crate::models::{User, UserPreference, UserStats};
use sqlx::PgPool;
use uuid::Uuid;

pub struct UserRepo;

impl UserRepo {
    pub async fn find_by_google_sub(pool: &PgPool, sub: &str) -> AppResult<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            r#"SELECT id, email, username, display_name, avatar_url, bio, google_sub,
                role, is_banned, last_seen_at, created_at, updated_at
              FROM users WHERE google_sub = $1"#,
        )
        .bind(sub)
        .fetch_optional(pool)
        .await?;
        Ok(user)
    }

    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            r#"SELECT id, email, username, display_name, avatar_url, bio, google_sub,
                role, is_banned, last_seen_at, created_at, updated_at
              FROM users WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(user)
    }

    pub async fn find_by_username(pool: &PgPool, username: &str) -> AppResult<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            r#"SELECT id, email, username, display_name, avatar_url, bio, google_sub,
                role, is_banned, last_seen_at, created_at, updated_at
              FROM users WHERE username = $1"#,
        )
        .bind(username)
        .fetch_optional(pool)
        .await?;
        Ok(user)
    }

    pub async fn create_from_google(
        pool: &PgPool,
        google_sub: &str,
        email: &str,
        name: &str,
        avatar_url: Option<&str>,
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

        let user = sqlx::query_as::<_, User>(
            r#"INSERT INTO users (email, username, display_name, avatar_url, google_sub)
              VALUES ($1, $2, $3, $4, $5)
              RETURNING id, email, username, display_name, avatar_url, bio, google_sub,
                role, is_banned, last_seen_at, created_at, updated_at"#,
        )
        .bind(email)
        .bind(&username)
        .bind(name)
        .bind(avatar_url)
        .bind(google_sub)
        .fetch_one(pool)
        .await?;

        // Create default preferences
        let _ = sqlx::query(
            "INSERT INTO user_preferences (user_id) VALUES ($1) ON CONFLICT DO NOTHING",
        )
        .bind(user.id)
        .execute(pool)
        .await;

        Ok(user)
    }

    pub async fn update_profile(
        pool: &PgPool,
        id: Uuid,
        display_name: &str,
        bio: &str,
        avatar_url: Option<&str>,
    ) -> AppResult<User> {
        // Validate avatar_url: chỉ cho phép http/https, không cho javascript:/data:
        // để tránh XSS qua avatar URL (thẻ <img src="javascript:..."> sẽ không chạy JS
        // nhưng src="data:text/html,..." hoặc scheme khác có thể lạm dụng).
        let avatar_url_safe = match avatar_url {
            Some(s) if !s.is_empty() => {
                let lower = s.to_ascii_lowercase();
                if lower.starts_with("http://") || lower.starts_with("https://") {
                    Some(s)
                } else {
                    return Err(crate::error::AppError::BadRequest(
                        "Avatar URL phải là http:// hoặc https://".into(),
                    ));
                }
            }
            _ => None,
        };

        let user = sqlx::query_as::<_, User>(
            r#"UPDATE users SET display_name = $1, bio = $2, avatar_url = COALESCE($3, avatar_url)
              WHERE id = $4
              RETURNING id, email, username, display_name, avatar_url, bio, google_sub,
                role, is_banned, last_seen_at, created_at, updated_at"#,
        )
        .bind(display_name)
        .bind(bio)
        .bind(avatar_url_safe)
        .bind(id)
        .fetch_one(pool)
        .await?;
        Ok(user)
    }

    pub async fn update_last_seen(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE users SET last_seen_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn stats(pool: &PgPool, id: Uuid) -> AppResult<UserStats> {
        let games_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM games WHERE user_id = $1 AND status = 'published'",
        )
        .bind(id)
        .fetch_one(pool)
        .await?;
        let followers_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM follows WHERE followee_id = $1")
                .bind(id)
                .fetch_one(pool)
                .await?;
        let following_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM follows WHERE follower_id = $1")
                .bind(id)
                .fetch_one(pool)
                .await?;
        Ok(UserStats {
            games_count,
            followers_count,
            following_count,
        })
    }

    pub async fn get_preferences(pool: &PgPool, user_id: Uuid) -> AppResult<UserPreference> {
        let pref = sqlx::query_as::<_, UserPreference>(
            r#"SELECT theme, email_notifications, show_online, language
              FROM user_preferences WHERE user_id = $1"#,
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        Ok(pref.unwrap_or_default())
    }

    pub async fn update_preferences(
        pool: &PgPool,
        user_id: Uuid,
        theme: &str,
        email_notif: bool,
        show_online: bool,
        language: &str,
    ) -> AppResult<()> {
        sqlx::query(
            r#"INSERT INTO user_preferences (user_id, theme, email_notifications, show_online, language)
              VALUES ($1, $2, $3, $4, $5)
              ON CONFLICT (user_id) DO UPDATE SET
                theme = EXCLUDED.theme,
                email_notifications = EXCLUDED.email_notifications,
                show_online = EXCLUDED.show_online,
                language = EXCLUDED.language"#,
        )
        .bind(user_id)
        .bind(theme)
        .bind(email_notif)
        .bind(show_online)
        .bind(language)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn list_admins(pool: &PgPool) -> AppResult<Vec<User>> {
        let users = sqlx::query_as::<_, User>(
            r#"SELECT id, email, username, display_name, avatar_url, bio, google_sub,
                role, is_banned, last_seen_at, created_at, updated_at
              FROM users WHERE role IN ('admin', 'moderator') ORDER BY created_at"#,
        )
        .fetch_all(pool)
        .await?;
        Ok(users)
    }

    /// Danh sách user cho admin (kèm số game), tìm kiếm theo tên/email
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
            r#"SELECT u.id, u.email, u.username, u.display_name, u.avatar_url, u.bio, u.google_sub,
                u.role, u.is_banned, u.last_seen_at, u.created_at, u.updated_at,
                COUNT(g.id) FILTER (WHERE g.status = 'published')::bigint AS games_count
              FROM users u
              LEFT JOIN games g ON g.user_id = u.id
              WHERE ($1 = '%%' OR u.email ILIKE $1 OR u.username ILIKE $1 OR u.display_name ILIKE $1)
              GROUP BY u.id
              ORDER BY u.created_at DESC
              LIMIT $2 OFFSET $3"#,
        )
        .bind(pattern)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(users)
    }

    pub async fn count_all(pool: &PgPool) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(pool)
            .await?;
        Ok(c)
    }

    /// Đếm user theo bộ lọc tìm kiếm (phân trang admin đúng tổng số)
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
            r#"SELECT COUNT(*) FROM users
               WHERE ($1 = '%%' OR email ILIKE $1 OR username ILIKE $1 OR display_name ILIKE $1)"#,
        )
        .bind(pattern)
        .fetch_one(pool)
        .await?;
        Ok(c)
    }

    /// Tìm user theo email (dùng cho seed admin)
    pub async fn find_by_email(pool: &PgPool, email: &str) -> AppResult<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            r#"SELECT id, email, username, display_name, avatar_url, bio, google_sub,
                role, is_banned, last_seen_at, created_at, updated_at
              FROM users WHERE email = $1"#,
        )
        .bind(email)
        .fetch_optional(pool)
        .await?;
        Ok(user)
    }

    /// Nâng cấp user lên admin nếu chưa phải (idempotent)
    pub async fn ensure_admin_by_email(pool: &PgPool, email: &str) -> AppResult<bool> {
        let res =
            sqlx::query("UPDATE users SET role = 'admin' WHERE email = $1 AND role != 'admin'")
                .bind(email)
                .execute(pool)
                .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn set_role(pool: &PgPool, user_id: Uuid, role: &str) -> AppResult<()> {
        sqlx::query("UPDATE users SET role = $1::user_role WHERE id = $2")
            .bind(role)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn set_banned(pool: &PgPool, user_id: Uuid, banned: bool) -> AppResult<()> {
        sqlx::query("UPDATE users SET is_banned = $1 WHERE id = $2")
            .bind(banned)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
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
                format!("{}_{}", base, i)
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
