use crate::error::AppResult;
use crate::models::repo::{GithubRepo, GithubRepoCard, RepoStatus};
use sqlx::PgPool;
use uuid::Uuid;

pub struct RepoRepo;

const CARD_COLS: &str = r"r.id, r.owner, r.repo_name, r.description, r.primary_language,
    r.stars, r.forks, r.open_issues, r.pushed_at,
    g.slug as game_slug, g.title as game_title,
    u.display_name as author_name, u.username as author_username, u.avatar_url as author_avatar,
    r.status, r.created_at";

const CARD_JOINS: &str = r"FROM github_repos r
    LEFT JOIN games g ON g.id = r.game_id
    JOIN users u ON u.id = r.user_id";

impl RepoRepo {
    #[allow(clippy::too_many_arguments)]
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        game_id: Option<Uuid>,
        owner: &str,
        repo_name: &str,
        description: &str,
        homepage: &str,
        language: &str,
        stars: i32,
        forks: i32,
        open_issues: i32,
        pushed_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> AppResult<Uuid> {
        let id: Uuid = sqlx::query_scalar(
            r"INSERT INTO github_repos
                (user_id, game_id, owner, repo_name, description, homepage,
                 primary_language, stars, forks, open_issues, pushed_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
               ON CONFLICT (owner, repo_name) DO UPDATE SET
                 user_id = EXCLUDED.user_id,
                 game_id = EXCLUDED.game_id,
                 stars = EXCLUDED.stars,
                 forks = EXCLUDED.forks,
                 open_issues = EXCLUDED.open_issues,
                 pushed_at = EXCLUDED.pushed_at
               RETURNING id",
        )
        .bind(user_id)
        .bind(game_id)
        .bind(owner)
        .bind(repo_name)
        .bind(if description.is_empty() {
            None
        } else {
            Some(description)
        })
        .bind(if homepage.is_empty() {
            None
        } else {
            Some(homepage)
        })
        .bind(if language.is_empty() {
            None
        } else {
            Some(language)
        })
        .bind(stars)
        .bind(forks)
        .bind(open_issues)
        .bind(pushed_at)
        .fetch_one(pool)
        .await?;
        Ok(id)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn exists(pool: &PgPool, owner: &str, repo_name: &str) -> AppResult<bool> {
        let c: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM github_repos WHERE owner = $1 AND repo_name = $2)",
        )
        .bind(owner)
        .bind(repo_name)
        .fetch_one(pool)
        .await?;
        Ok(c)
    }

    /// Tìm repo theo owner/name (dùng kiểm tra trùng khi đăng ký — chống
    /// user khác chiếm quyền sở hữu entry đã có qua ON CONFLICT UPDATE).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_by_owner_name(
        pool: &PgPool,
        owner: &str,
        repo_name: &str,
    ) -> AppResult<Option<GithubRepo>> {
        let repo = sqlx::query_as::<_, GithubRepo>(
            r"SELECT id, user_id, game_id, owner, repo_name, description, homepage,
                primary_language, stars, forks, open_issues, pushed_at, status,
                created_at, updated_at
              FROM github_repos WHERE owner = $1 AND repo_name = $2",
        )
        .bind(owner)
        .bind(repo_name)
        .fetch_optional(pool)
        .await?;
        Ok(repo)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn list_approved(
        pool: &PgPool,
        limit: i64,
        offset: i64,
        sort: &str,
    ) -> AppResult<Vec<GithubRepoCard>> {
        let order = match sort {
            "stars" => "r.stars DESC",
            "recent" => "r.created_at DESC",
            _ => "r.stars DESC, r.updated_at DESC",
        };
        let sql = format!(
            r"SELECT {CARD_COLS} {CARD_JOINS} WHERE r.status = 'approved' ORDER BY {order} LIMIT $1 OFFSET $2"
        );
        let rows = sqlx::query_as::<_, GithubRepoCard>(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;
        Ok(rows)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn list_by_user(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<GithubRepoCard>> {
        let sql = format!(
            r"SELECT {CARD_COLS} {CARD_JOINS} WHERE r.user_id = $1 AND r.status != 'hidden' ORDER BY r.created_at DESC"
        );
        let rows = sqlx::query_as::<_, GithubRepoCard>(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .fetch_all(pool)
            .await?;
        Ok(rows)
    }

    /// Cho admin: tất cả repos, filter theo status + phân trang
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn list_admin(
        pool: &PgPool,
        status: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<GithubRepoCard>> {
        // Chuẩn hoá status: None / "" / whitespace → None (không filter)
        let status = status.filter(|s| !s.trim().is_empty());
        let sql = match status {
            Some(_) => format!(
                r"SELECT {CARD_COLS} {CARD_JOINS} WHERE r.status = $1::repo_status ORDER BY r.updated_at DESC LIMIT $2 OFFSET $3"
            ),
            None => format!(
                r"SELECT {CARD_COLS} {CARD_JOINS} ORDER BY r.updated_at DESC LIMIT $1 OFFSET $2"
            ),
        };
        let rows = match status {
            Some(s) => {
                sqlx::query_as::<_, GithubRepoCard>(sqlx::AssertSqlSafe(sql.as_str()))
                    .bind(s)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(pool)
                    .await?
            }
            None => {
                sqlx::query_as::<_, GithubRepoCard>(sqlx::AssertSqlSafe(sql.as_str()))
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(pool)
                    .await?
            }
        };
        Ok(rows)
    }

    /// Đếm repos theo bộ lọc — phân trang admin repos đúng tổng.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count_admin(pool: &PgPool, status: Option<&str>) -> AppResult<i64> {
        let status = status.filter(|s| !s.trim().is_empty());
        let c: i64 = match status {
            Some(s) => {
                sqlx::query_scalar("SELECT COUNT(*) FROM github_repos WHERE status::text = $1")
                    .bind(s)
                    .fetch_one(pool)
                    .await?
            }
            None => {
                sqlx::query_scalar("SELECT COUNT(*) FROM github_repos")
                    .fetch_one(pool)
                    .await?
            }
        };
        Ok(c)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<GithubRepo>> {
        let repo = sqlx::query_as::<_, GithubRepo>(
            r"SELECT id, user_id, game_id, owner, repo_name, description, homepage,
                primary_language, stars, forks, open_issues, pushed_at, status,
                created_at, updated_at
              FROM github_repos WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(repo)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn set_status(pool: &PgPool, id: Uuid, status: &str) -> AppResult<()> {
        sqlx::query("UPDATE github_repos SET status = $1::repo_status WHERE id = $2")
            .bind(status)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn delete(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM github_repos WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn refresh_all_stars(pool: &PgPool) -> AppResult<Vec<(Uuid, String, String)>> {
        let rows = sqlx::query_as::<_, (Uuid, String, String)>(
            "SELECT id, owner, repo_name FROM github_repos WHERE status = 'approved' LIMIT 500",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    #[allow(clippy::too_many_arguments)]
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn update_meta(
        pool: &PgPool,
        id: Uuid,
        description: &str,
        homepage: &str,
        language: &str,
        stars: i32,
        forks: i32,
        open_issues: i32,
        pushed_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> AppResult<()> {
        sqlx::query(
            r"UPDATE github_repos SET
                description = NULLIF($2,''), homepage = NULLIF($3,''),
                primary_language = NULLIF($4,''), stars = $5, forks = $6,
                open_issues = $7, pushed_at = $8
              WHERE id = $1",
        )
        .bind(id)
        .bind(description)
        .bind(homepage)
        .bind(language)
        .bind(stars)
        .bind(forks)
        .bind(open_issues)
        .bind(pushed_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count_approved(pool: &PgPool) -> AppResult<i64> {
        let c: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM github_repos WHERE status = 'approved'")
                .fetch_one(pool)
                .await?;
        Ok(c)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn pending_count(pool: &PgPool) -> AppResult<i64> {
        let c: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM github_repos WHERE status = 'pending'")
                .fetch_one(pool)
                .await?;
        Ok(c)
    }

    #[must_use]
    pub fn parse_github_url(url: &str) -> Option<(String, String)> {
        // Chấp nhận: owner/repo, https://github.com/owner/repo, https://github.com/owner/repo.git
        let trimmed = url.trim().trim_end_matches(".git");
        let path = if let Some(rest) = trimmed
            .strip_prefix("https://github.com/")
            .or_else(|| trimmed.strip_prefix("http://github.com/"))
        {
            rest.to_string()
        } else if trimmed.starts_with("github.com/") {
            trimmed.trim_start_matches("github.com/").to_string()
        } else if !trimmed.contains('/') && trimmed.split('/').count() == 1 {
            // Không phải dạng owner/repo
            if trimmed.contains(' ') || trimmed.is_empty() {
                return None;
            }
            return None;
        } else {
            trimmed.to_string()
        };
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if parts.len() < 2 {
            return None;
        }
        let owner = parts[0].to_string();
        let name = parts[1].split_whitespace().next().unwrap_or("").to_string();
        if owner.is_empty() || name.is_empty() {
            return None;
        }
        // Validate ký tự hợp lệ của GitHub. Ngoài charset, chặn thêm
        // segment "." và ".." — charset có cho phép dấu chấm nên bản trước
        // chấp nhận owner "..": parse_github_url("../etc/passwd") trả về
        // Some(("..", "etc")) và URL API trở thành /repos/../etc
        // (vector path traversal, phát hiện qua unit test).
        let valid = |s: &str| {
            !s.is_empty()
                && s != "."
                && s != ".."
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        };
        if !valid(&owner) || !valid(&name) {
            return None;
        }
        Some((owner, name))
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn status_from_str(s: &str) -> Option<RepoStatus> {
        match s {
            "pending" => Some(RepoStatus::Pending),
            "approved" => Some(RepoStatus::Approved),
            "hidden" => Some(RepoStatus::Hidden),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_github_url_full_https() {
        assert_eq!(
            RepoRepo::parse_github_url("https://github.com/mhieuhonda/khogame"),
            Some(("mhieuhonda".into(), "khogame".into()))
        );
        // Có .git đuôi
        assert_eq!(
            RepoRepo::parse_github_url("https://github.com/user/repo.git"),
            Some(("user".into(), "repo".into()))
        );
        // http (không khuyến nghị nhưng chấp nhận)
        assert_eq!(
            RepoRepo::parse_github_url("http://github.com/user/repo"),
            Some(("user".into(), "repo".into()))
        );
    }

    #[test]
    fn test_parse_github_url_short_forms() {
        // owner/repo thuần
        assert_eq!(
            RepoRepo::parse_github_url("owner/repo"),
            Some(("owner".into(), "repo".into()))
        );
        // github.com/owner/repo
        assert_eq!(
            RepoRepo::parse_github_url("github.com/owner/repo"),
            Some(("owner".into(), "repo".into()))
        );
        // Khoảng trắng 2 đầu
        assert_eq!(
            RepoRepo::parse_github_url("  https://github.com/o/r  "),
            Some(("o".into(), "r".into()))
        );
        // Path sâu hơn (lấy 2 phần đầu)
        assert_eq!(
            RepoRepo::parse_github_url("https://github.com/o/r/tree/main"),
            Some(("o".into(), "r".into()))
        );
    }

    #[test]
    fn test_parse_github_url_rejects_invalid() {
        // Không có gì
        assert_eq!(RepoRepo::parse_github_url(""), None);
        // Chỉ 1 từ, không có slash
        assert_eq!(RepoRepo::parse_github_url("onlyme"), None);
        // Ký tự lạ trong owner (path traversal / query injection)
        assert_eq!(RepoRepo::parse_github_url("../etc/passwd"), None);
        assert_eq!(
            RepoRepo::parse_github_url("https://github.com/o w n/e r"),
            None
        );
        assert_eq!(RepoRepo::parse_github_url("owner/repo?x=1"), None);
        // Host khác giả mạo github
        assert_eq!(
            RepoRepo::parse_github_url("https://evil.com/owner/repo"),
            None
        );
    }

    #[test]
    fn test_parse_github_url_blocks_dot_dot() {
        // owner chứa ".." — chống path traversal khi build URL API
        assert_eq!(RepoRepo::parse_github_url("../repo"), None);
        assert_eq!(RepoRepo::parse_github_url("owner/.."), None);
    }

    #[test]
    fn test_status_from_str() {
        assert!(RepoRepo::status_from_str("pending").is_some());
        assert!(RepoRepo::status_from_str("approved").is_some());
        assert!(RepoRepo::status_from_str("hidden").is_some());
        assert!(RepoRepo::status_from_str("banned").is_none());
    }
}
