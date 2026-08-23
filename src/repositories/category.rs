use crate::error::AppResult;
use crate::models::category::{Category, CategoryWithCount};
use sqlx::PgPool;

pub struct CategoryRepo;

impl CategoryRepo {
    pub async fn list_all(pool: &PgPool) -> AppResult<Vec<Category>> {
        let cats = sqlx::query_as::<_, Category>(
            r#"SELECT id, name, slug, description, icon, created_at
              FROM categories ORDER BY name"#,
        )
        .fetch_all(pool)
        .await?;
        Ok(cats)
    }

    pub async fn list_with_counts(pool: &PgPool) -> AppResult<Vec<CategoryWithCount>> {
        let cats = sqlx::query_as::<_, CategoryWithCount>(
            r#"SELECT c.id, c.name, c.slug, c.description, c.icon,
                COUNT(g.id) as games_count
              FROM categories c
              LEFT JOIN games g ON g.category_id = c.id AND g.status = 'published'
              GROUP BY c.id, c.name, c.slug, c.description, c.icon
              ORDER BY c.name"#,
        )
        .fetch_all(pool)
        .await?;
        Ok(cats)
    }

    pub async fn find_by_slug(pool: &PgPool, slug: &str) -> AppResult<Option<Category>> {
        let c = sqlx::query_as::<_, Category>(
            r#"SELECT id, name, slug, description, icon, created_at FROM categories WHERE slug = $1"#,
        )
        .bind(slug)
        .fetch_optional(pool)
        .await?;
        Ok(c)
    }
}
