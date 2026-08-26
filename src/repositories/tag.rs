use crate::error::AppResult;
use crate::models::tag::Tag;
use sqlx::PgPool;

pub struct TagRepo;

impl TagRepo {
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn popular(pool: &PgPool, limit: i64) -> AppResult<Vec<Tag>> {
        let tags = sqlx::query_as::<_, Tag>(
            r"SELECT id, name, slug, usage_count, created_at
              FROM tags ORDER BY usage_count DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(tags)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_by_slug(pool: &PgPool, slug: &str) -> AppResult<Option<Tag>> {
        let t = sqlx::query_as::<_, Tag>(
            r"SELECT id, name, slug, usage_count, created_at FROM tags WHERE slug = $1",
        )
        .bind(slug)
        .fetch_optional(pool)
        .await?;
        Ok(t)
    }
}
