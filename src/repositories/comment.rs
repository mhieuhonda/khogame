use crate::error::AppResult;
use crate::models::comment::CommentWithUser;
use sqlx::PgPool;
use uuid::Uuid;

pub struct CommentRepo;

impl CommentRepo {
    pub async fn create(
        pool: &PgPool,
        game_id: Uuid,
        user_id: Uuid,
        parent_id: Option<Uuid>,
        content: &str,
    ) -> AppResult<Uuid> {
        let id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO comments (game_id, user_id, parent_id, content)
              VALUES ($1, $2, $3, $4) RETURNING id"#,
        )
        .bind(game_id)
        .bind(user_id)
        .bind(parent_id)
        .bind(content)
        .fetch_one(pool)
        .await?;

        // Insert notification for game owner
        let owner_id: Option<Uuid> =
            sqlx::query_scalar("SELECT user_id FROM games WHERE id = $1")
                .bind(game_id)
                .fetch_optional(pool)
                .await?;
        if let Some(oid) = owner_id {
            if oid != user_id {
                let title = if parent_id.is_some() {
                    "Có người vừa trả lời bình luận của bạn"
                } else {
                    "Có người vừa bình luận game của bạn"
                };
                let ntype = if parent_id.is_some() { "reply" } else { "comment" };
                let game_slug: String = sqlx::query_scalar("SELECT slug FROM games WHERE id = $1")
                    .bind(game_id)
                    .fetch_one(pool)
                    .await?;
                let _ = sqlx::query(
                    r#"INSERT INTO notifications (user_id, actor_id, type, title, link)
                      VALUES ($1, $2, $3::notification_type, $4, $5)"#,
                )
                .bind(oid)
                .bind(user_id)
                .bind(ntype)
                .bind(title)
                .bind(format!("/games/{}", game_slug))
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
    ) -> AppResult<Vec<CommentWithUser>> {
        let comments = sqlx::query_as::<_, CommentWithUser>(
            r#"SELECT c.id, c.game_id, c.user_id, c.parent_id, c.content,
                c.like_count, c.is_pinned, c.created_at, c.updated_at,
                u.display_name as user_name, u.avatar_url as user_avatar,
                EXISTS(
                  SELECT 1 FROM comment_likes cl WHERE cl.comment_id = c.id AND cl.user_id = $2
                ) as is_liked
              FROM comments c
              JOIN users u ON u.id = c.user_id
              WHERE c.game_id = $1 AND c.parent_id IS NULL
              ORDER BY c.is_pinned DESC, c.created_at DESC
              LIMIT $3 OFFSET $4"#,
        )
        .bind(game_id)
        .bind(viewer_id.unwrap_or_else(Uuid::nil))
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        // For each top-level comment, load replies
        // (not ideal performance-wise but for simplicity)
        let mut result = Vec::with_capacity(comments.len());
        for mut c in comments {
            // Don't load replies here; we'll fetch them per-comment via list_replies
            // For listing, just leave empty
            // We won't pre-load replies to keep this simple
            // (HTMX will load replies lazily)
            result.push(c);
        }
        Ok(result)
    }

    pub async fn list_replies(
        pool: &PgPool,
        parent_id: Uuid,
        viewer_id: Option<Uuid>,
    ) -> AppResult<Vec<CommentWithUser>> {
        let comments = sqlx::query_as::<_, CommentWithUser>(
            r#"SELECT c.id, c.game_id, c.user_id, c.parent_id, c.content,
                c.like_count, c.is_pinned, c.created_at, c.updated_at,
                u.display_name as user_name, u.avatar_url as user_avatar,
                EXISTS(
                  SELECT 1 FROM comment_likes cl WHERE cl.comment_id = c.id AND cl.user_id = $2
                ) as is_liked
              FROM comments c
              JOIN users u ON u.id = c.user_id
              WHERE c.parent_id = $1
              ORDER BY c.created_at ASC"#,
        )
        .bind(parent_id)
        .bind(viewer_id.unwrap_or_else(Uuid::nil))
        .fetch_all(pool)
        .await?;
        Ok(comments)
    }

    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<CommentWithUser>> {
        let c = sqlx::query_as::<_, CommentWithUser>(
            r#"SELECT c.id, c.game_id, c.user_id, c.parent_id, c.content,
                c.like_count, c.is_pinned, c.created_at, c.updated_at,
                u.display_name as user_name, u.avatar_url as user_avatar,
                FALSE as is_liked
              FROM comments c
              JOIN users u ON u.id = c.user_id
              WHERE c.id = $1"#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(c)
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM comments WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn toggle_pin(pool: &PgPool, id: Uuid) -> AppResult<bool> {
        let pinned: bool = sqlx::query_scalar(
            "UPDATE comments SET is_pinned = NOT is_pinned WHERE id = $1 RETURNING is_pinned",
        )
        .bind(id)
        .fetch_one(pool)
        .await?;
        Ok(pinned)
    }

    pub async fn toggle_like(
        pool: &PgPool,
        comment_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<bool> {
        let liked: Option<bool> = sqlx::query_scalar(
            "SELECT 1 FROM comment_likes WHERE comment_id = $1 AND user_id = $2",
        )
        .bind(comment_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        if liked.is_some() {
            sqlx::query("DELETE FROM comment_likes WHERE comment_id = $1 AND user_id = $2")
                .bind(comment_id)
                .bind(user_id)
                .execute(pool)
                .await?;
            sqlx::query("UPDATE comments SET like_count = like_count - 1 WHERE id = $1")
                .bind(comment_id)
                .execute(pool)
                .await?;
            Ok(false)
        } else {
            sqlx::query(
                "INSERT INTO comment_likes (comment_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(comment_id)
            .bind(user_id)
            .execute(pool)
            .await?;
            sqlx::query("UPDATE comments SET like_count = like_count + 1 WHERE id = $1")
                .bind(comment_id)
                .execute(pool)
                .await?;
            Ok(true)
        }
    }
}
