use crate::error::{AppError, AppResult};
use crate::models::news_category::{NewsCategory, NewsCategoryWithCount};
use sqlx::PgPool;
use uuid::Uuid;

pub struct NewsCategoryRepo;

impl NewsCategoryRepo {
    /// Liệt kê tất cả category active — sắp theo sort_order rồi name.
    /// Dùng cho `<select>` trong form /news/new và trang public /news.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn list_active(pool: &PgPool) -> AppResult<Vec<NewsCategory>> {
        let cats = sqlx::query_as::<_, NewsCategory>(
            r"SELECT id, name, slug, description, icon, sort_order, is_active,
                     created_at, updated_at
              FROM news_categories
              WHERE is_active = TRUE
              ORDER BY sort_order ASC, name ASC",
        )
        .fetch_all(pool)
        .await?;
        Ok(cats)
    }

    /// Liệt kê TẤT cả category (kể cả inactive) — chỉ admin dùng.
    /// Kèm count số tin thuộc category để admin biết xoá có an toàn không.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn list_all_with_counts(pool: &PgPool) -> AppResult<Vec<NewsCategoryWithCount>> {
        let cats = sqlx::query_as::<_, NewsCategoryWithCount>(
            r"SELECT c.id, c.name, c.slug, c.description, c.icon, c.sort_order,
                     c.is_active, c.created_at, c.updated_at,
                     COUNT(n.id)::bigint AS news_count
              FROM news_categories c
              LEFT JOIN news n ON n.category = c.slug
              GROUP BY c.id, c.name, c.slug, c.description, c.icon, c.sort_order,
                       c.is_active, c.created_at, c.updated_at
              ORDER BY c.sort_order ASC, c.name ASC",
        )
        .fetch_all(pool)
        .await?;
        Ok(cats)
    }

    /// Tìm category theo slug — dùng cho trang public /news/category/{slug}
    /// (route tương lai) và validate khi user submit form /news/new.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_by_slug(pool: &PgPool, slug: &str) -> AppResult<Option<NewsCategory>> {
        let c = sqlx::query_as::<_, NewsCategory>(
            r"SELECT id, name, slug, description, icon, sort_order, is_active,
                     created_at, updated_at
              FROM news_categories WHERE slug = $1",
        )
        .bind(slug)
        .fetch_optional(pool)
        .await?;
        Ok(c)
    }

    /// Tạo category mới. Trả Conflict nếu slug đã tồn tại —
    /// slug auto-sinh từ name qua `slug::slugify`, 2 name khác có thể
    /// ra cùng slug (vd "Tin Game" và "tin game" → "tin-game").
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn create(
        pool: &PgPool,
        name: &str,
        slug: &str,
        description: &str,
        icon: &str,
    ) -> AppResult<Uuid> {
        let id: Uuid = sqlx::query_scalar(
            r"INSERT INTO news_categories (name, slug, description, icon)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT (slug) DO NOTHING
               RETURNING id",
        )
        .bind(name)
        .bind(slug)
        .bind(description)
        .bind(icon)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| {
            AppError::Conflict(format!(
                "Thể loại tin tức với đường dẫn '{slug}' đã tồn tại (có thể trùng tên sau khi bỏ dấu). Hãy đổi tên khác."
            ))
        })?;
        Ok(id)
    }

    /// Update category — không đổi slug (URL ổn định). Nếu admin muốn đổi
    /// slug, phải xoá và tạo lại (để tránh broken URL).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        name: &str,
        description: &str,
        icon: &str,
        sort_order: i32,
        is_active: bool,
    ) -> AppResult<()> {
        sqlx::query(
            r"UPDATE news_categories
               SET name = $1, description = $2, icon = $3,
                   sort_order = $4, is_active = $5
               WHERE id = $6",
        )
        .bind(name)
        .bind(description)
        .bind(icon)
        .bind(sort_order)
        .bind(is_active)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Xoá category vĩnh viễn. Tin cũ có category=slug sẽ giữ giá trị text
    /// (news.category là VARCHAR, không FK) → tin cũ vẫn render với label
    /// rỗng hoặc fallback "— Không phân loại —".
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn delete(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM news_categories WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Tìm theo id — dùng cho admin edit form.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<NewsCategory>> {
        let c = sqlx::query_as::<_, NewsCategory>(
            r"SELECT id, name, slug, description, icon, sort_order, is_active,
                     created_at, updated_at
              FROM news_categories WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(c)
    }
}
