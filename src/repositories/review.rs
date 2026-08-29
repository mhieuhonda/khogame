use crate::error::AppResult;
use crate::models::review::ReviewWithUser;
use sqlx::PgPool;
use uuid::Uuid;

pub struct ReviewRepo;

impl ReviewRepo {
    /// Tạo hoặc CẬP NHẬT review của user cho game (1 user = 1 review/game,
    /// UNIQUE(game_id, user_id)). Notify chủ game (bỏ qua tự review) —
    /// CHỈ khi review mới được tạo, không phải khi edit/re-rate.
    ///
    /// v3.0.0 FIX (XP farm + spam): trả về `(id, was_insert)`. Trước đây
    /// mọi lần POST (kể cả edit review cũ) đều được coi là review mới →
    /// handler cộng +15 XP mỗi lần (reason `review` không có cap) và
    /// notify/email owner mỗi lần. Giờ phân biệt INSERT vs UPDATE qua
    /// `xmax = 0` (row mới insert có xmax = 0; row vừa UPDATE có xmax ≠ 0).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn create_or_update(
        pool: &PgPool,
        game_id: Uuid,
        user_id: Uuid,
        title: &str,
        content: &str,
        rating: i16,
    ) -> AppResult<(Uuid, bool)> {
        let (id, was_insert): (Uuid, bool) = sqlx::query_as(
            r"INSERT INTO reviews (game_id, user_id, title, content, rating)
              VALUES ($1, $2, $3, $4, $5)
              ON CONFLICT (game_id, user_id) DO UPDATE SET
                title = EXCLUDED.title,
                content = EXCLUDED.content,
                rating = EXCLUDED.rating
              RETURNING id, (xmax = 0) AS was_insert",
        )
        .bind(game_id)
        .bind(user_id)
        .bind(title)
        .bind(content)
        .bind(rating)
        .fetch_one(pool)
        .await?;

        // Notify game owner — chỉ khi review MỚI (edit không spam lại)
        if was_insert {
            let owner_id: Option<Uuid> =
                sqlx::query_scalar("SELECT user_id FROM games WHERE id = $1")
                    .bind(game_id)
                    .fetch_optional(pool)
                    .await?;
            if let Some(oid) = owner_id {
                // v3.0.0 — tôn trọng prefs của chủ game (mặc định TRUE)
                if oid != user_id
                    && crate::repositories::PrefsRepo::allows(pool, oid, "review")
                        .await
                        .unwrap_or(true)
                {
                    let game_slug: String =
                        sqlx::query_scalar("SELECT slug FROM games WHERE id = $1")
                            .bind(game_id)
                            .fetch_one(pool)
                            .await?;
                    let _ = sqlx::query(
                        r"INSERT INTO notifications (user_id, actor_id, type, title, link)
                          VALUES ($1, $2, 'review'::notification_type, $3, $4)",
                    )
                    .bind(oid)
                    .bind(user_id)
                    .bind("Có người vừa đánh giá game của bạn")
                    .bind(format!("/games/{game_slug}"))
                    .execute(pool)
                    .await;
                }
            }
        }

        Ok((id, was_insert))
    }

    /// v2.9.0 — Review của game kèm `is_helpful` của viewer (vote qua
    /// bảng review_helpful_votes — migration 021) + level người viết
    /// (JOIN user_xp_totals cho chip cấp độ).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn list_by_game(
        pool: &PgPool,
        game_id: Uuid,
        viewer_id: Option<Uuid>,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<ReviewWithUser>> {
        let reviews = sqlx::query_as::<_, ReviewWithUser>(
            r"SELECT r.id, r.game_id, r.user_id, r.title, r.content, r.rating,
                r.helpful_count, r.created_at, r.updated_at,
                u.display_name as user_name, u.username as user_username,
                u.avatar_url as user_avatar,
                EXISTS(SELECT 1 FROM review_helpful_votes h
                       WHERE h.review_id = r.id AND h.user_id = $2) AS is_helpful,
                COALESCE(x.total_xp, 0) AS author_xp
              FROM reviews r
              JOIN users u ON u.id = r.user_id
              LEFT JOIN user_xp_totals x ON x.user_id = r.user_id
              WHERE r.game_id = $1
              ORDER BY r.helpful_count DESC, r.created_at DESC
              LIMIT $3 OFFSET $4",
        )
        .bind(game_id)
        .bind(viewer_id.unwrap_or_else(Uuid::nil))
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(reviews)
    }

    /// v2.9.0 — Tổng số review của game.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count_by_game(pool: &PgPool, game_id: Uuid) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM reviews WHERE game_id = $1")
            .bind(game_id)
            .fetch_one(pool)
            .await?;
        Ok(c)
    }

    /// v2.9.0 — Toggle vote "hữu ích" cho review. Trả về (đã_vote, count_mới).
    /// Không cho vote review của chính mình.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn toggle_helpful(
        pool: &PgPool,
        review_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<(bool, i32)> {
        let mut tx = pool.begin().await?;
        // Không vote review của chính mình
        let owner: Uuid = sqlx::query_scalar("SELECT user_id FROM reviews WHERE id = $1")
            .bind(review_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| crate::error::AppError::NotFound("Review không tồn tại".into()))?;
        if owner == user_id {
            return Err(crate::error::AppError::BadRequest(
                "Không thể vote review của chính mình".into(),
            ));
        }
        // Xoá trước (toggle)
        let deleted =
            sqlx::query("DELETE FROM review_helpful_votes WHERE review_id = $1 AND user_id = $2")
                .bind(review_id)
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
        let voted = if deleted.rows_affected() > 0 {
            false
        } else {
            sqlx::query("INSERT INTO review_helpful_votes (review_id, user_id) VALUES ($1, $2)")
                .bind(review_id)
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
            true
        };
        let count: i32 = sqlx::query_scalar(
            "UPDATE reviews SET helpful_count =
                (SELECT COUNT(*) FROM review_helpful_votes WHERE review_id = $1)
             WHERE id = $1 RETURNING helpful_count",
        )
        .bind(review_id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok((voted, count))
    }

    /// v2.9.0 — Review của viewer cho game (điền form sửa).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_own(
        pool: &PgPool,
        game_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<Option<crate::models::review::Review>> {
        let r = sqlx::query_as::<_, crate::models::review::Review>(
            "SELECT id, game_id, user_id, title, content, rating, helpful_count,
                    created_at, updated_at
             FROM reviews WHERE game_id = $1 AND user_id = $2",
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        Ok(r)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    /// v3.0.0 FIX: staff (admin/mod) xóa review của NGƯỜI KHÁC trước đây
    /// no-op âm thầm (`AND user_id = $2` dùng id của staff → rows_affected
    /// = 0, handler vẫn báo thành công). Giờ truyền `is_staff`: staff xóa
    /// được review bất kỳ; đồng thời trả bool để handler 404 khi không xóa
    /// được dòng nào thay vì báo thành công giả.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn delete(pool: &PgPool, id: Uuid, user_id: Uuid, is_staff: bool) -> AppResult<bool> {
        let res = sqlx::query("DELETE FROM reviews WHERE id = $1 AND ($3 OR user_id = $2)")
            .bind(id)
            .bind(user_id)
            .bind(is_staff)
            .execute(pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }
}
