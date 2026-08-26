use crate::error::{AppError, AppResult};
use crate::models::news::{
    News, NewsComment, NewsCommentWithAuthor, NewsForAdmin, NewsStatus, NewsWithAuthor,
};
use sqlx::PgPool;
use uuid::Uuid;

pub struct NewsRepo;

/// Form dữ liệu tạo/sửa tin tức (xây dựng ở handler rồi truyền xuống repo).
#[derive(Debug, Clone)]
pub struct NewsForm {
    pub title: String,
    pub excerpt: String,
    pub content: String,
    pub cover_image: Option<String>,
    pub category: String,
    pub source_url: String,
    pub source_name: String,
}

impl NewsRepo {
    /// Tạo tin tức mới. Mặc định status = 'pending' — admin phải duyệt.
    /// `author_ip` và `author_ua` lưu lại để admin truy vết nếu cần.
    /// Trả về id của tin vừa tạo.
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        form: &NewsForm,
        slug: &str,
        author_ip: Option<&str>,
        author_ua: Option<&str>,
    ) -> AppResult<Uuid> {
        let id: Uuid = sqlx::query_scalar(
            r"INSERT INTO news (
                user_id, title, slug, excerpt, content, cover_image,
                category, source_url, source_name, status,
                author_ip, author_ua
              ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'pending', $10, $11)
              RETURNING id",
        )
        .bind(user_id)
        .bind(&form.title)
        .bind(slug)
        .bind(&form.excerpt)
        .bind(&form.content)
        .bind(&form.cover_image)
        .bind(&form.category)
        .bind(&form.source_url)
        .bind(&form.source_name)
        .bind(author_ip)
        .bind(author_ua)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            // 23505 = unique_violation (slug trùng) — map thành Conflict
            if let sqlx::Error::Database(ref db) = e {
                if db.code().as_deref() == Some("23505") {
                    return AppError::Conflict(format!(
                        "Tin tức với đường dẫn '{slug}' đã tồn tại. Hãy đổi tiêu đề."
                    ));
                }
            }
            e.into()
        })?;
        Ok(id)
    }

    /// Cập nhật tin tức. Chỉ tác giả hoặc admin được gọi (kiểm tra ở handler).
    /// Không cho phép đổi status qua form edit — status chỉ thay đổi qua
    /// approve/reject/archive endpoints riêng.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn update(pool: &PgPool, id: Uuid, form: &NewsForm) -> AppResult<()> {
        sqlx::query(
            r"UPDATE news SET
                title = $1, excerpt = $2, content = $3, cover_image = $4,
                category = $5, source_url = $6, source_name = $7
              WHERE id = $8",
        )
        .bind(&form.title)
        .bind(&form.excerpt)
        .bind(&form.content)
        .bind(&form.cover_image)
        .bind(&form.category)
        .bind(&form.source_url)
        .bind(&form.source_name)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Lấy danh sách tin đã published, phân trang, mới nhất trước.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn list_published(
        pool: &PgPool,
        page: i64,
        per_page: i64,
    ) -> AppResult<Vec<NewsWithAuthor>> {
        let offset = (page - 1).max(0) * per_page;
        let items = sqlx::query_as::<_, NewsWithAuthor>(
            r"SELECT n.id, n.user_id, n.title, n.slug, n.excerpt, n.content,
                     n.cover_image, n.category, n.source_url, n.source_name,
                     n.status, n.view_count, n.like_count,
                     n.comment_count, n.is_featured, n.published_at, n.created_at,
                     u.display_name AS author_name, u.username AS author_username,
                     u.avatar_url AS author_avatar
              FROM news n
              JOIN users u ON u.id = n.user_id
              WHERE n.status = 'published'
              ORDER BY n.is_featured DESC, n.published_at DESC NULLS LAST, n.created_at DESC
              LIMIT $1 OFFSET $2",
        )
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(items)
    }

    /// Đếm tổng số tin đã published (cho phân trang).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count_published(pool: &PgPool) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM news WHERE status = 'published'")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }

    /// Lấy tin nổi bật (`is_featured=true`, status=published).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn list_featured(pool: &PgPool, limit: i64) -> AppResult<Vec<NewsWithAuthor>> {
        let items = sqlx::query_as::<_, NewsWithAuthor>(
            r"SELECT n.id, n.user_id, n.title, n.slug, n.excerpt, n.content,
                     n.cover_image, n.category, n.source_url, n.source_name,
                     n.status, n.view_count, n.like_count,
                     n.comment_count, n.is_featured, n.published_at, n.created_at,
                     u.display_name AS author_name, u.username AS author_username,
                     u.avatar_url AS author_avatar
              FROM news n
              JOIN users u ON u.id = n.user_id
              WHERE n.status = 'published' AND n.is_featured = TRUE
              ORDER BY n.published_at DESC NULLS LAST
              LIMIT $1",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(items)
    }

    /// Lọc theo category (status=published).
    pub async fn list_by_category(
        pool: &PgPool,
        category: &str,
        page: i64,
        per_page: i64,
    ) -> AppResult<Vec<NewsWithAuthor>> {
        let offset = (page - 1).max(0) * per_page;
        let items = sqlx::query_as::<_, NewsWithAuthor>(
            r"SELECT n.id, n.user_id, n.title, n.slug, n.excerpt, n.content,
                     n.cover_image, n.category, n.source_url, n.source_name,
                     n.status, n.view_count, n.like_count,
                     n.comment_count, n.is_featured, n.published_at, n.created_at,
                     u.display_name AS author_name, u.username AS author_username,
                     u.avatar_url AS author_avatar
              FROM news n
              JOIN users u ON u.id = n.user_id
              WHERE n.status = 'published' AND n.category = $1
              ORDER BY n.published_at DESC NULLS LAST, n.created_at DESC
              LIMIT $2 OFFSET $3",
        )
        .bind(category)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(items)
    }

    /// Tìm kiếm full-text dùng trgm index.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn search(
        pool: &PgPool,
        query: &str,
        page: i64,
        per_page: i64,
    ) -> AppResult<Vec<NewsWithAuthor>> {
        let offset = (page - 1).max(0) * per_page;
        let pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
        let items = sqlx::query_as::<_, NewsWithAuthor>(
            r"SELECT n.id, n.user_id, n.title, n.slug, n.excerpt, n.content,
                     n.cover_image, n.category, n.source_url, n.source_name,
                     n.status, n.view_count, n.like_count,
                     n.comment_count, n.is_featured, n.published_at, n.created_at,
                     u.display_name AS author_name, u.username AS author_username,
                     u.avatar_url AS author_avatar
              FROM news n
              JOIN users u ON u.id = n.user_id
              WHERE n.status = 'published'
                AND (n.title ILIKE $1 OR n.content ILIKE $1)
              ORDER BY n.published_at DESC NULLS LAST
              LIMIT $2 OFFSET $3",
        )
        .bind(&pattern)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(items)
    }

    /// Lấy chi tiết tin theo slug (chỉ published hoặc archived cho người dùng thường).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_by_slug_public(
        pool: &PgPool,
        slug: &str,
    ) -> AppResult<Option<NewsWithAuthor>> {
        let item = sqlx::query_as::<_, NewsWithAuthor>(
            r"SELECT n.id, n.user_id, n.title, n.slug, n.excerpt, n.content,
                     n.cover_image, n.category, n.source_url, n.source_name,
                     n.status, n.view_count, n.like_count,
                     n.comment_count, n.is_featured, n.published_at, n.created_at,
                     u.display_name AS author_name, u.username AS author_username,
                     u.avatar_url AS author_avatar
              FROM news n
              JOIN users u ON u.id = n.user_id
              WHERE n.slug = $1
                AND n.status IN ('published', 'archived')",
        )
        .bind(slug)
        .fetch_optional(pool)
        .await?;
        Ok(item)
    }

    /// Lấy tin theo slug — không check status (admin xem).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_by_slug_admin(pool: &PgPool, slug: &str) -> AppResult<Option<NewsForAdmin>> {
        let item = sqlx::query_as::<_, NewsForAdmin>(
            r"SELECT n.*, u.display_name AS author_name,
                     u.username AS author_username, u.email AS author_email,
                     u.avatar_url AS author_avatar
              FROM news n
              JOIN users u ON u.id = n.user_id
              WHERE n.slug = $1",
        )
        .bind(slug)
        .fetch_optional(pool)
        .await?;
        Ok(item)
    }

    /// Lấy tin theo id — dùng cho admin actions (approve/reject/archive).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<News>> {
        let item = sqlx::query_as::<_, News>(
            r"SELECT id, user_id, title, slug, excerpt, content, cover_image,
                     category, source_url, source_name, status, author_ip, author_ua,
                     reviewed_by, review_note, view_count, like_count, comment_count,
                     is_featured, published_at, created_at, updated_at
              FROM news WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(item)
    }

    /// Bump `view_count` +1. Best-effort, không ảnh hưởng request chính.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn increment_views(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE news SET view_count = view_count + 1 WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    // ===== Admin: workflow duyệt =====

    /// Danh sách tin đang chờ duyệt (status='pending'), mới trước.
    /// Trả về `NewsForAdmin` để admin xem được IP/UA/email tác giả.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn list_pending(
        pool: &PgPool,
        page: i64,
        per_page: i64,
    ) -> AppResult<Vec<NewsForAdmin>> {
        let offset = (page - 1).max(0) * per_page;
        let items = sqlx::query_as::<_, NewsForAdmin>(
            r"SELECT n.*, u.display_name AS author_name,
                     u.username AS author_username, u.email AS author_email,
                     u.avatar_url AS author_avatar
              FROM news n
              JOIN users u ON u.id = n.user_id
              WHERE n.status = 'pending'
              ORDER BY n.created_at ASC
              LIMIT $1 OFFSET $2",
        )
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(items)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count_pending(pool: &PgPool) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM news WHERE status = 'pending'")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }

    /// Duyệt tin: pending → published, set `published_at` + `reviewed_by`.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn approve(pool: &PgPool, id: Uuid, admin_id: Uuid) -> AppResult<()> {
        sqlx::query(
            r"UPDATE news SET
                status = 'published',
                published_at = COALESCE(published_at, NOW()),
                reviewed_by = $2,
                review_note = ''
              WHERE id = $1",
        )
        .bind(id)
        .bind(admin_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Từ chối tin: pending → rejected, kèm `review_note`.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn reject(pool: &PgPool, id: Uuid, admin_id: Uuid, note: &str) -> AppResult<()> {
        sqlx::query(
            r"UPDATE news SET
                status = 'rejected',
                reviewed_by = $2,
                review_note = $3
              WHERE id = $1",
        )
        .bind(id)
        .bind(admin_id)
        .bind(note)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Lưu trữ: published → archived (ẩn khỏi list chính, vẫn xem được qua link).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn archive(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE news SET status = 'archived' WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Toggle `is_featured` (chỉ tác động tới published).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn set_featured(pool: &PgPool, id: Uuid, featured: bool) -> AppResult<()> {
        sqlx::query(
            r"UPDATE news SET is_featured = $2
              WHERE id = $1 AND status = 'published'",
        )
        .bind(id)
        .bind(featured)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Xoá tin (chỉ admin hoặc tác giả).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn delete(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM news WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Lấy tin của một user (cho trang my-news).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn list_by_user(
        pool: &PgPool,
        user_id: Uuid,
        include_unpublished: bool,
    ) -> AppResult<Vec<News>> {
        let items = if include_unpublished {
            sqlx::query_as::<_, News>(
                r"SELECT id, user_id, title, slug, excerpt, content, cover_image,
                          category, source_url, source_name, status, author_ip, author_ua,
                          reviewed_by, review_note, view_count, like_count, comment_count,
                          is_featured, published_at, created_at, updated_at
                   FROM news WHERE user_id = $1
                   ORDER BY created_at DESC",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as::<_, News>(
                r"SELECT id, user_id, title, slug, excerpt, content, cover_image,
                          category, source_url, source_name, status, author_ip, author_ua,
                          reviewed_by, review_note, view_count, like_count, comment_count,
                          is_featured, published_at, created_at, updated_at
                   FROM news WHERE user_id = $1 AND status IN ('published', 'archived')
                   ORDER BY published_at DESC NULLS LAST, created_at DESC",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await?
        };
        Ok(items)
    }

    // ===== Likes =====

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn toggle_like(pool: &PgPool, user_id: Uuid, news_id: Uuid) -> AppResult<bool> {
        // DELETE-first pattern: tránh double-increment khi race condition.
        // Trả về true nếu đã like (sau khi INSERT), false nếu đã unlike (sau khi DELETE).
        let mut tx = pool.begin().await?;
        let deleted: u64 =
            sqlx::query("DELETE FROM news_likes WHERE user_id = $1 AND news_id = $2")
                .bind(user_id)
                .bind(news_id)
                .execute(&mut *tx)
                .await?
                .rows_affected();
        let liked = if deleted == 0 {
            sqlx::query(
                "INSERT INTO news_likes (user_id, news_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(user_id)
            .bind(news_id)
            .execute(&mut *tx)
            .await?;
            true
        } else {
            false
        };
        tx.commit().await?;
        Ok(liked)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn has_liked(pool: &PgPool, user_id: Uuid, news_id: Uuid) -> AppResult<bool> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM news_likes WHERE user_id = $1 AND news_id = $2)",
        )
        .bind(user_id)
        .bind(news_id)
        .fetch_one(pool)
        .await?;
        Ok(exists)
    }

    // ===== Comments =====

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn create_comment(
        pool: &PgPool,
        news_id: Uuid,
        user_id: Uuid,
        parent_id: Option<Uuid>,
        content: &str,
    ) -> AppResult<Uuid> {
        let id: Uuid = sqlx::query_scalar(
            r"INSERT INTO news_comments (news_id, user_id, parent_id, content)
              VALUES ($1, $2, $3, $4) RETURNING id",
        )
        .bind(news_id)
        .bind(user_id)
        .bind(parent_id)
        .bind(content)
        .fetch_one(pool)
        .await?;
        Ok(id)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn list_comments(
        pool: &PgPool,
        news_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<NewsCommentWithAuthor>> {
        let items = sqlx::query_as::<_, NewsCommentWithAuthor>(
            r"SELECT c.id, c.news_id, c.user_id, c.parent_id, c.content,
                     c.like_count, c.is_pinned, c.created_at,
                     u.display_name AS author_name, u.username AS author_username,
                     u.avatar_url AS author_avatar
              FROM news_comments c
              JOIN users u ON u.id = c.user_id
              WHERE c.news_id = $1 AND c.parent_id IS NULL
              ORDER BY c.is_pinned DESC, c.created_at DESC
              LIMIT $2 OFFSET $3",
        )
        .bind(news_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(items)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn list_replies(
        pool: &PgPool,
        parent_id: Uuid,
    ) -> AppResult<Vec<NewsCommentWithAuthor>> {
        let items = sqlx::query_as::<_, NewsCommentWithAuthor>(
            r"SELECT c.id, c.news_id, c.user_id, c.parent_id, c.content,
                     c.like_count, c.is_pinned, c.created_at,
                     u.display_name AS author_name, u.username AS author_username,
                     u.avatar_url AS author_avatar
              FROM news_comments c
              JOIN users u ON u.id = c.user_id
              WHERE c.parent_id = $1
              ORDER BY c.created_at ASC",
        )
        .bind(parent_id)
        .fetch_all(pool)
        .await?;
        Ok(items)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn delete_comment(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        is_admin: bool,
    ) -> AppResult<()> {
        let affected = if is_admin {
            sqlx::query("DELETE FROM news_comments WHERE id = $1")
                .bind(id)
                .execute(pool)
                .await?
                .rows_affected()
        } else {
            sqlx::query("DELETE FROM news_comments WHERE id = $1 AND user_id = $2")
                .bind(id)
                .bind(user_id)
                .execute(pool)
                .await?
                .rows_affected()
        };
        if affected == 0 {
            return Err(AppError::NotFound(
                "Bình luận không tồn tại hoặc không phải của bạn".into(),
            ));
        }
        Ok(())
    }

    /// Lấy 1 `NewsComment` theo id (kiểm tra quyền).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_comment_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<NewsComment>> {
        let item = sqlx::query_as::<_, NewsComment>(
            r"SELECT id, news_id, user_id, parent_id, content, like_count,
                     is_pinned, created_at, updated_at
              FROM news_comments WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(item)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn toggle_comment_like(
        pool: &PgPool,
        user_id: Uuid,
        comment_id: Uuid,
    ) -> AppResult<bool> {
        let mut tx = pool.begin().await?;
        let deleted: u64 =
            sqlx::query("DELETE FROM news_comment_likes WHERE user_id = $1 AND comment_id = $2")
                .bind(user_id)
                .bind(comment_id)
                .execute(&mut *tx)
                .await?
                .rows_affected();
        let liked = if deleted == 0 {
            sqlx::query(
                "INSERT INTO news_comment_likes (user_id, comment_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(user_id)
            .bind(comment_id)
            .execute(&mut *tx)
            .await?;
            true
        } else {
            false
        };
        tx.commit().await?;
        Ok(liked)
    }

    // ===== Stats cho admin dashboard =====

    /// Đếm tin theo status (cho admin dashboard).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count_by_status(pool: &PgPool, status: NewsStatus) -> AppResult<i64> {
        let status_str = match status {
            NewsStatus::Draft => "draft",
            NewsStatus::Pending => "pending",
            NewsStatus::Published => "published",
            NewsStatus::Archived => "archived",
            NewsStatus::Rejected => "rejected",
        };
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM news WHERE status = $1::news_status")
                .bind(status_str)
                .fetch_one(pool)
                .await?;
        Ok(count)
    }

    /// Top tác giả tin tức (cho dashboard).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn top_authors(pool: &PgPool, limit: i64) -> AppResult<Vec<(Uuid, String, i64)>> {
        let rows: Vec<(Uuid, String, i64)> = sqlx::query_as(
            r"SELECT n.user_id, u.display_name, COUNT(*)::BIGINT AS cnt
              FROM news n JOIN users u ON u.id = n.user_id
              WHERE n.status = 'published'
              GROUP BY n.user_id, u.display_name
              ORDER BY cnt DESC
              LIMIT $1",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Gợi ý tiêu đề tin tức khi user gõ vào ô search.
    /// Trả về Vec<(title, slug)> cho tối đa `limit` kết quả, chỉ tin published.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn suggest_titles(
        pool: &PgPool,
        query: &str,
        limit: i64,
    ) -> AppResult<Vec<(String, String)>> {
        // Escape wildcard + clamp 100 ký tự như search công khai
        let q: String = query.chars().take(100).collect();
        let pattern = format!("%{}%", q.replace('%', "\\%").replace('_', "\\_"));
        let rows: Vec<(String, String)> = sqlx::query_as(
            r"SELECT title, slug FROM news
              WHERE status = 'published' AND title ILIKE $1
              ORDER BY published_at DESC NULLS LAST
              LIMIT $2",
        )
        .bind(&pattern)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Repository test cần DB thật → skip trong CI.
    // Unit test cho NewsStatus nằm ở src/models/news.rs.
    // Đây chỉ là placeholder để CI không fail khi check `cargo test --no-run`.
    #[test]
    fn news_repo_compiles() {
        // Compile-only check: struct tồn tại, không có method nào thiếu.
        let _ = std::any::type_name::<NewsRepo>;
    }
}
