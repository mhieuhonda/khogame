use crate::error::AppResult;
use crate::models::notification::NotificationWithActor;
use sqlx::PgPool;
use uuid::Uuid;

pub struct NotificationRepo;

impl NotificationRepo {
    pub async fn list_for_user(
        pool: &PgPool,
        user_id: Uuid,
        limit: i64,
        offset: i64,
        only_unread: bool,
    ) -> AppResult<Vec<NotificationWithActor>> {
        let sql = if only_unread {
            r#"SELECT n.id, n.user_id, n.actor_id, n.type, n.title, n.content, n.link, n.is_read, n.created_at,
                u.display_name as actor_name, u.avatar_url as actor_avatar
              FROM notifications n
              LEFT JOIN users u ON u.id = n.actor_id
              WHERE n.user_id = $1 AND n.is_read = FALSE
              ORDER BY n.created_at DESC LIMIT $2 OFFSET $3"#
        } else {
            r#"SELECT n.id, n.user_id, n.actor_id, n.type, n.title, n.content, n.link, n.is_read, n.created_at,
                u.display_name as actor_name, u.avatar_url as actor_avatar
              FROM notifications n
              LEFT JOIN users u ON u.id = n.actor_id
              WHERE n.user_id = $1
              ORDER BY n.created_at DESC LIMIT $2 OFFSET $3"#
        };
        let items = sqlx::query_as::<_, NotificationWithActor>(sql)
            .bind(user_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;
        Ok(items)
    }

    pub async fn unread_count(pool: &PgPool, user_id: Uuid) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND is_read = FALSE",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(c)
    }

    pub async fn mark_read(pool: &PgPool, id: Uuid, user_id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE notifications SET is_read = TRUE WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn mark_all_read(pool: &PgPool, user_id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE notifications SET is_read = TRUE WHERE user_id = $1 AND is_read = FALSE")
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn create_system(
        pool: &PgPool,
        user_id: Uuid,
        title: &str,
        content: &str,
        link: &str,
    ) -> AppResult<()> {
        sqlx::query(
            r#"INSERT INTO notifications (user_id, type, title, content, link)
              VALUES ($1, 'system'::notification_type, $2, $3, $4)"#,
        )
        .bind(user_id)
        .bind(title)
        .bind(content)
        .bind(link)
        .execute(pool)
        .await?;
        Ok(())
    }
}
