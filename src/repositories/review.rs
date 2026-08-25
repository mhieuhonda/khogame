use crate::error::AppResult;
use crate::models::review::ReviewWithUser;
use sqlx::PgPool;
use uuid::Uuid;

pub struct ReviewRepo;

impl ReviewRepo {
    pub async fn create_or_update(
        pool: &PgPool,
        game_id: Uuid,
        user_id: Uuid,
        title: &str,
        content: &str,
        rating: i16,
    ) -> AppResult<Uuid> {
        let id: Uuid = sqlx::query_scalar(
            r"INSERT INTO reviews (game_id, user_id, title, content, rating)
              VALUES ($1, $2, $3, $4, $5)
              ON CONFLICT (game_id, user_id) DO UPDATE SET
                title = EXCLUDED.title,
                content = EXCLUDED.content,
                rating = EXCLUDED.rating
              RETURNING id",
        )
        .bind(game_id)
        .bind(user_id)
        .bind(title)
        .bind(content)
        .bind(rating)
        .fetch_one(pool)
        .await?;

        // Notify game owner
        let owner_id: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM games WHERE id = $1")
            .bind(game_id)
            .fetch_optional(pool)
            .await?;
        if let Some(oid) = owner_id {
            if oid != user_id {
                let game_slug: String = sqlx::query_scalar("SELECT slug FROM games WHERE id = $1")
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

        Ok(id)
    }

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
                u.display_name as user_name, u.avatar_url as user_avatar,
                EXISTS(
                  SELECT 1 FROM review_helpful rh WHERE rh.review_id = r.id AND rh.user_id = $2
                ) as is_helpful
              FROM reviews r
              JOIN users u ON u.id = r.user_id
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

    pub async fn delete(pool: &PgPool, id: Uuid, user_id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM reviews WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
