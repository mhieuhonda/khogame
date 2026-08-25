use crate::error::AppResult;
use crate::models::category::Category;
use crate::models::CategoryWithCount;
use sqlx::PgPool;
use uuid::Uuid;

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

    // ===== CRUD cho admin =====
    /// Tạo category mới. Trả Conflict nếu slug đã tồn tại — trước đây
    /// ON CONFLICT DO UPDATE SET name âm thầm ĐỔI TÊN category cũ khi
    /// admin tạo category có tên khác nhưng slugify trùng (vd 'GIẢI TRÍ'
    /// và 'Giải Trí' cùng ra slug 'giai-tri'): user tưởng đã tạo category
    /// mới, thật ra vừa ghi đè tên category đang dùng.
    pub async fn create(
        pool: &PgPool,
        name: &str,
        slug: &str,
        description: &str,
        icon: &str,
    ) -> AppResult<Uuid> {
        let id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO categories (name, slug, description, icon)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT (slug) DO NOTHING
               RETURNING id"#,
        )
        .bind(name)
        .bind(slug)
        .bind(description)
        .bind(icon)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| {
            crate::error::AppError::Conflict(format!(
                "Thể loại với đường dẫn '{}' đã tồn tại (có thể trùng tên sau khi bỏ dấu). Hãy đổi tên khác.",
                slug
            ))
        })?;
        Ok(id)
    }

    pub async fn update(
        pool: &PgPool,
        id: uuid::Uuid,
        name: &str,
        description: &str,
        icon: &str,
    ) -> AppResult<()> {
        sqlx::query("UPDATE categories SET name = $1, description = $2, icon = $3 WHERE id = $4")
            .bind(name)
            .bind(description)
            .bind(icon)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn delete(pool: &PgPool, id: uuid::Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM categories WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn find_by_id(pool: &PgPool, id: uuid::Uuid) -> AppResult<Option<Category>> {
        let c = sqlx::query_as::<_, Category>(
            r#"SELECT id, name, slug, description, icon, created_at FROM categories WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(c)
    }
}
