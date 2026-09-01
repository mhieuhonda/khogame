//! v2.9.0 — Bộ sưu tập game (collections) + lịch sử xem (view_history).

use crate::error::{AppError, AppResult};
use crate::models::{GameCard, User};
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use sqlx::PgPool;
use uuid::Uuid;

/// Bộ sưu tập (row bảng collections).
#[derive(Debug, Clone, FromRow)]
pub struct Collection {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub description: String,
    pub is_public: bool,
    pub game_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Bộ sưu tập kèm thông tin chủ (list public / trên profile).
#[derive(Debug, Clone, FromRow)]
pub struct CollectionWithOwner {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub description: String,
    pub is_public: bool,
    pub game_count: i32,
    pub updated_at: DateTime<Utc>,
    pub owner_name: String,
    pub owner_username: String,
    pub owner_avatar: Option<String>,
}

pub struct CollectionRepo;

impl CollectionRepo {
    /// Tạo bộ sưu tập mới (giới hạn 20/user chống spam).
    /// # Errors
    /// Trả lỗi khi đạt giới hạn hoặc DB fail.
    ///
    /// v3.12.0 (audit logic L4): COUNT-then-INSERT không atomic — burst tạo
    /// đồng thời vượt cap vài bộ. Advisory lock theo user (pattern
    /// award_xp/trivia) xếp hàng request song song, quota bất biến.
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        title: &str,
        description: &str,
        is_public: bool,
    ) -> AppResult<Collection> {
        let mut tx = pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('col_quota:' || $1::text))")
            .bind(user_id.to_string())
            .execute(&mut *tx)
            .await?;
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM collections WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?;
        if count >= 20 {
            return Err(AppError::BadRequest(
                "Bạn chỉ có thể tạo tối đa 20 bộ sưu tập".into(),
            ));
        }
        let c = sqlx::query_as::<_, Collection>(
            r"INSERT INTO collections (user_id, title, description, is_public)
               VALUES ($1, $2, $3, $4)
               RETURNING id, user_id, title, description, is_public,
                         game_count, created_at, updated_at",
        )
        .bind(user_id)
        .bind(title)
        .bind(description)
        .bind(is_public)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(c)
    }

    /// Danh sách bộ sưu tập của user (owner xem hết, người khác chỉ public).
    /// # Errors
    /// Trả lỗi khi DB fail.
    /// v3.0.0 — Đếm bộ sưu tập của user (cho level perk giới hạn).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn count_for_user(pool: &PgPool, user_id: Uuid) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM collections WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
        Ok(c)
    }

    pub async fn list_for_user(
        pool: &PgPool,
        user_id: Uuid,
        include_private: bool,
    ) -> AppResult<Vec<Collection>> {
        let rows = sqlx::query_as::<_, Collection>(
            r"SELECT id, user_id, title, description, is_public,
                      game_count, created_at, updated_at
               FROM collections
               WHERE user_id = $1 AND ($2 OR is_public)
               ORDER BY updated_at DESC",
        )
        .bind(user_id)
        .bind(include_private)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Bộ sưu tập public của user cho trang hồ sơ (kèm owner).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn list_public_with_owner(
        pool: &PgPool,
        user_id: Uuid,
    ) -> AppResult<Vec<CollectionWithOwner>> {
        let rows = sqlx::query_as::<_, CollectionWithOwner>(
            r"SELECT c.id, c.user_id, c.title, c.description, c.is_public,
                      c.game_count, c.updated_at,
                      u.display_name AS owner_name, u.username AS owner_username,
                      u.avatar_url AS owner_avatar
               FROM collections c
               JOIN users u ON u.id = c.user_id
               WHERE c.user_id = $1 AND c.is_public
               ORDER BY c.updated_at DESC LIMIT 12",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Tìm theo id.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<Collection>> {
        let c = sqlx::query_as::<_, Collection>(
            r"SELECT id, user_id, title, description, is_public,
                      game_count, created_at, updated_at
               FROM collections WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(c)
    }

    /// Cập nhật tiêu đề/mô tả/độ công khai (chỉ chủ sở hữu).
    /// # Errors
    /// Trả lỗi khi không tìm thấy hoặc DB fail.
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        title: &str,
        description: &str,
        is_public: bool,
    ) -> AppResult<()> {
        let res = sqlx::query(
            r"UPDATE collections SET title = $3, description = $4, is_public = $5,
                     updated_at = NOW()
             WHERE id = $1 AND user_id = $2",
        )
        .bind(id)
        .bind(user_id)
        .bind(title)
        .bind(description)
        .bind(is_public)
        .execute(pool)
        .await?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound("Bộ sưu tập không tồn tại".into()));
        }
        Ok(())
    }

    /// Xóa bộ sưu tập (cascade xóa collection_games).
    /// # Errors
    /// Trả lỗi khi không tìm thấy hoặc DB fail.
    pub async fn delete(pool: &PgPool, id: Uuid, user_id: Uuid) -> AppResult<()> {
        let res = sqlx::query("DELETE FROM collections WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound("Bộ sưu tập không tồn tại".into()));
        }
        Ok(())
    }

    /// Thêm game vào bộ sưu tập (idempotent; chỉ game published).
    /// # Errors
    /// Trả lỗi khi game/collection không tồn tại hoặc DB fail.
    pub async fn add_game(pool: &PgPool, collection_id: Uuid, game_id: Uuid) -> AppResult<bool> {
        let mut tx = pool.begin().await?;
        // Kiểm tra game tồn tại + published
        let status: Option<String> =
            sqlx::query_scalar("SELECT status::text FROM games WHERE id = $1")
                .bind(game_id)
                .fetch_optional(&mut *tx)
                .await?;
        match status.as_deref() {
            Some("published") => {}
            _ => {
                return Err(AppError::BadRequest(
                    "Chỉ thêm được game đã xuất bản vào bộ sưu tập".into(),
                ));
            }
        }
        let res = sqlx::query(
            r"INSERT INTO collection_games (collection_id, game_id)
               VALUES ($1, $2)
               ON CONFLICT (collection_id, game_id) DO NOTHING",
        )
        .bind(collection_id)
        .bind(game_id)
        .execute(&mut *tx)
        .await?;
        let added = res.rows_affected() > 0;
        if added {
            sqlx::query(
                "UPDATE collections SET game_count = game_count + 1, updated_at = NOW()
                 WHERE id = $1",
            )
            .bind(collection_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(added)
    }

    /// Xóa game khỏi bộ sưu tập.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn remove_game(pool: &PgPool, collection_id: Uuid, game_id: Uuid) -> AppResult<bool> {
        let mut tx = pool.begin().await?;
        let res =
            sqlx::query("DELETE FROM collection_games WHERE collection_id = $1 AND game_id = $2")
                .bind(collection_id)
                .bind(game_id)
                .execute(&mut *tx)
                .await?;
        let removed = res.rows_affected() > 0;
        if removed {
            sqlx::query(
                "UPDATE collections
                 SET game_count = GREATEST(0, game_count - 1), updated_at = NOW()
                 WHERE id = $1",
            )
            .bind(collection_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(removed)
    }

    /// Game trong bộ sưu tập (phân trang).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn games(
        pool: &PgPool,
        collection_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<GameCard>> {
        let rows = sqlx::query_as::<_, GameCard>(
            r"SELECT g.id, g.slug, g.title, g.excerpt, g.cover_image,
                      c.name as category_name, c.slug as category_slug,
                      u.display_name as author_name, u.avatar_url as author_avatar,
                      g.view_count, g.download_count, g.like_count, g.comment_count,
                      g.rating_avg, g.rating_count,
                      COALESCE(
                        (SELECT array_agg(DISTINCT platform::text) FROM game_links WHERE game_id = g.id),
                        ARRAY[]::text[]
                      ) as platforms,
                      g.published_at
               FROM collection_games cg
               JOIN games g ON g.id = cg.game_id
               LEFT JOIN users u ON u.id = g.user_id
               LEFT JOIN categories c ON c.id = g.category_id
               WHERE cg.collection_id = $1 AND g.status = 'published'
               ORDER BY cg.added_at DESC
               LIMIT $2 OFFSET $3",
        )
        .bind(collection_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Bộ sưu tập của user có chứa game này (đánh dấu "đã thêm" trên UI).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn collections_containing_game(
        pool: &PgPool,
        user_id: Uuid,
        game_id: Uuid,
    ) -> AppResult<Vec<(Uuid, String, bool)>> {
        let rows: Vec<(Uuid, String, bool)> = sqlx::query_as(
            r"SELECT c.id, c.title,
                      EXISTS(SELECT 1 FROM collection_games cg
                             WHERE cg.collection_id = c.id AND cg.game_id = $2) AS contains
               FROM collections c
               WHERE c.user_id = $1
               ORDER BY c.updated_at DESC",
        )
        .bind(user_id)
        .bind(game_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}

/// v2.9.0 — Lịch sử xem game ("Tiếp tục xem").
pub struct ViewHistoryRepo;

impl ViewHistoryRepo {
    /// Ghi/nhất thời điểm xem game (upsert — 1 game chiếm 1 slot).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn record(pool: &PgPool, user_id: Uuid, game_id: Uuid) -> AppResult<()> {
        sqlx::query(
            r"INSERT INTO view_history (user_id, game_id)
               VALUES ($1, $2)
               ON CONFLICT (user_id, game_id)
               DO UPDATE SET viewed_at = NOW()",
        )
        .bind(user_id)
        .bind(game_id)
        .execute(pool)
        .await?;
        // Giữ tối đa 60 game/user — xoá cũ nhất khi vượt (cheap, chạy
        // inline vì chỉ là DELETE ... WHERE ctid IN (...))
        sqlx::query(
            r"DELETE FROM view_history
               WHERE user_id = $1 AND game_id NOT IN (
                   SELECT game_id FROM view_history WHERE user_id = $1
                   ORDER BY viewed_at DESC LIMIT 60
               )",
        )
        .bind(user_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Game xem gần đây (chỉ published — game ẩn/bị xoá tự loại khỏi list).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn recent(pool: &PgPool, user_id: Uuid, limit: i64) -> AppResult<Vec<GameCard>> {
        let rows = sqlx::query_as::<_, GameCard>(
            r"SELECT g.id, g.slug, g.title, g.excerpt, g.cover_image,
                      c.name as category_name, c.slug as category_slug,
                      u.display_name as author_name, u.avatar_url as author_avatar,
                      g.view_count, g.download_count, g.like_count, g.comment_count,
                      g.rating_avg, g.rating_count,
                      COALESCE(
                        (SELECT array_agg(DISTINCT platform::text) FROM game_links WHERE game_id = g.id),
                        ARRAY[]::text[]
                      ) as platforms,
                      g.published_at
               FROM view_history vh
               JOIN games g ON g.id = vh.game_id
               LEFT JOIN users u ON u.id = g.user_id
               LEFT JOIN categories c ON c.id = g.category_id
               WHERE vh.user_id = $1 AND g.status = 'published'
               ORDER BY vh.viewed_at DESC
               LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}

/// Model hiển thị dòng "đang online" cho chat panel (v2.9.0).
#[derive(Debug, Clone, FromRow, serde::Serialize)]
pub struct OnlineUser {
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub role: String,
}

/// v2.9.0 — Truy vấn thông tin user online từ danh sách UUID (chat).
pub async fn online_users_info(pool: &PgPool, ids: &[Uuid]) -> AppResult<Vec<OnlineUser>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as::<_, OnlineUser>(
        r"SELECT username, display_name, avatar_url, role::text AS role
           FROM users WHERE id = ANY($1) AND is_banned = FALSE
           ORDER BY display_name ASC",
    )
    .bind(ids)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Kiểm tra user tồn tại + không bị ban (dùng cho fallback hiển thị).
#[must_use]
pub fn user_is_viewable(u: &User) -> bool {
    !u.is_banned
}
