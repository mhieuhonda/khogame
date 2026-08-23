use crate::error::AppResult;
use crate::models::interaction::{Bookmark, Follow};
use sqlx::PgPool;
use uuid::Uuid;

pub struct InteractionRepo;

impl InteractionRepo {
    pub async fn toggle_like(pool: &PgPool, game_id: Uuid, user_id: Uuid) -> AppResult<bool> {
        let liked: Option<i32> = sqlx::query_scalar(
            "SELECT 1 FROM likes WHERE game_id = $1 AND user_id = $2",
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        if liked.is_some() {
            sqlx::query("DELETE FROM likes WHERE game_id = $1 AND user_id = $2")
                .bind(game_id)
                .bind(user_id)
                .execute(pool)
                .await?;
            Ok(false)
        } else {
            sqlx::query("INSERT INTO likes (game_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                .bind(game_id)
                .bind(user_id)
                .execute(pool)
                .await?;
            Ok(true)
        }
    }

    pub async fn is_liked(pool: &PgPool, game_id: Uuid, user_id: Uuid) -> AppResult<bool> {
        let r: Option<i32> = sqlx::query_scalar(
            "SELECT 1 FROM likes WHERE game_id = $1 AND user_id = $2",
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        Ok(r.is_some())
    }

    pub async fn toggle_bookmark(pool: &PgPool, game_id: Uuid, user_id: Uuid) -> AppResult<bool> {
        let exists: Option<i32> = sqlx::query_scalar(
            "SELECT 1 FROM bookmarks WHERE game_id = $1 AND user_id = $2",
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        if exists.is_some() {
            sqlx::query("DELETE FROM bookmarks WHERE game_id = $1 AND user_id = $2")
                .bind(game_id)
                .bind(user_id)
                .execute(pool)
                .await?;
            Ok(false)
        } else {
            sqlx::query(
                "INSERT INTO bookmarks (game_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(game_id)
            .bind(user_id)
            .execute(pool)
            .await?;
            Ok(true)
        }
    }

    pub async fn is_bookmarked(pool: &PgPool, game_id: Uuid, user_id: Uuid) -> AppResult<bool> {
        let r: Option<i32> = sqlx::query_scalar(
            "SELECT 1 FROM bookmarks WHERE game_id = $1 AND user_id = $2",
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        Ok(r.is_some())
    }

    pub async fn bookmarks_for_user(
        pool: &PgPool,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<crate::models::GameCard>> {
        use crate::models::GameCard;
        let cards = sqlx::query_as::<_, GameCard>(
            r#"SELECT g.id, g.slug, g.title, g.excerpt, g.cover_image,
                c.name as category_name, c.slug as category_slug,
                u.display_name as author_name, u.avatar_url as author_avatar,
                g.view_count, g.download_count, g.like_count, g.comment_count,
                g.rating_avg, g.rating_count,
                COALESCE(
                  (SELECT array_agg(DISTINCT platform::text) FROM game_links WHERE game_id = g.id),
                  ARRAY[]::text[]
                ) as platforms,
                g.published_at
              FROM bookmarks b
              JOIN games g ON g.id = b.game_id
              LEFT JOIN users u ON u.id = g.user_id
              LEFT JOIN categories c ON c.id = g.category_id
              WHERE b.user_id = $1 AND g.status = 'published'
              ORDER BY b.created_at DESC
              LIMIT $2 OFFSET $3"#,
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(cards)
    }

    pub async fn toggle_follow(
        pool: &PgPool,
        follower_id: Uuid,
        followee_id: Uuid,
    ) -> AppResult<bool> {
        if follower_id == followee_id {
            return Ok(false);
        }
        let exists: Option<i32> = sqlx::query_scalar(
            "SELECT 1 FROM follows WHERE follower_id = $1 AND followee_id = $2",
        )
        .bind(follower_id)
        .bind(followee_id)
        .fetch_optional(pool)
        .await?;
        if exists.is_some() {
            sqlx::query("DELETE FROM follows WHERE follower_id = $1 AND followee_id = $2")
                .bind(follower_id)
                .bind(followee_id)
                .execute(pool)
                .await?;
            Ok(false)
        } else {
            sqlx::query(
                "INSERT INTO follows (follower_id, followee_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(follower_id)
            .bind(followee_id)
            .execute(pool)
            .await?;
            // Notify followee
            let _ = sqlx::query(
                r#"INSERT INTO notifications (user_id, actor_id, type, title, link)
                  VALUES ($1, $2, 'follow'::notification_type, $3, $4)"#,
            )
            .bind(followee_id)
            .bind(follower_id)
            .bind("Có người mới theo dõi bạn")
            .bind(format!("/u/{}", Self::get_username(pool, follower_id).await.unwrap_or_default()))
            .execute(pool)
            .await;
            Ok(true)
        }
    }

    pub async fn is_following(
        pool: &PgPool,
        follower_id: Uuid,
        followee_id: Uuid,
    ) -> AppResult<bool> {
        let r: Option<i32> = sqlx::query_scalar(
            "SELECT 1 FROM follows WHERE follower_id = $1 AND followee_id = $2",
        )
        .bind(follower_id)
        .bind(followee_id)
        .fetch_optional(pool)
        .await?;
        Ok(r.is_some())
    }

    pub async fn set_rating(
        pool: &PgPool,
        game_id: Uuid,
        user_id: Uuid,
        score: i16,
    ) -> AppResult<()> {
        sqlx::query(
            r#"INSERT INTO ratings (game_id, user_id, score)
              VALUES ($1, $2, $3)
              ON CONFLICT (game_id, user_id) DO UPDATE SET score = EXCLUDED.score"#,
        )
        .bind(game_id)
        .bind(user_id)
        .bind(score)
        .execute(pool)
        .await?;

        // Recompute game rating_avg / rating_count
        sqlx::query(
            r#"UPDATE games SET
                rating_avg = (SELECT COALESCE(AVG(score), 0) FROM ratings WHERE game_id = $1),
                rating_count = (SELECT COUNT(*) FROM ratings WHERE game_id = $1)
              WHERE id = $1"#,
        )
        .bind(game_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn get_user_rating(
        pool: &PgPool,
        game_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<Option<i16>> {
        let r: Option<i16> = sqlx::query_scalar(
            "SELECT score FROM ratings WHERE game_id = $1 AND user_id = $2",
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        Ok(r)
    }

    pub async fn record_download(
        pool: &PgPool,
        game_id: Uuid,
        user_id: Option<Uuid>,
        platform: &str,
        ip: Option<&str>,
    ) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO downloads (game_id, user_id, platform, ip_address) VALUES ($1, $2, $3, $4)",
        )
        .bind(game_id)
        .bind(user_id)
        .bind(platform)
        .bind(ip)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn record_share(
        pool: &PgPool,
        game_id: Uuid,
        user_id: Option<Uuid>,
        platform: &str,
    ) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO shares (game_id, user_id, platform) VALUES ($1, $2, $3)",
        )
        .bind(game_id)
        .bind(user_id)
        .bind(platform)
        .execute(pool)
        .await?;
        Ok(())
    }

    async fn get_username(pool: &PgPool, user_id: Uuid) -> AppResult<String> {
        let n: String = sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
        Ok(n)
    }

    pub async fn followers_count(pool: &PgPool, user_id: Uuid) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM follows WHERE followee_id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
        Ok(c)
    }
}
