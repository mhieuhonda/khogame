use crate::error::AppResult;
use crate::models::game::{
    AgeRating, Game, GameCard, GameForm, GameLink, GameScreenshot, GameStatus, Platform,
};
use crate::models::AdminGameRow;
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

pub struct GameRepo;

impl GameRepo {
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        form: &GameForm,
        slug: &str,
    ) -> AppResult<Uuid> {
        let status = GameStatus::from_str(&form.status);
        let age_rating = AgeRating::from_str(&form.age_rating);
        let release_date = form
            .release_date
            .as_deref()
            .and_then(crate::utils::parse_date);
        let category_id = form
            .category_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .and_then(|s| Uuid::parse_str(s).ok());
        let languages = if form.languages_vec().is_empty() {
            vec!["vi".to_string()]
        } else {
            form.languages_vec()
        };
        let published_at = if matches!(status, GameStatus::Published) {
            Some(Utc::now())
        } else {
            None
        };

        let id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO games (
                user_id, title, slug, excerpt, content, status, version,
                developer, publisher, release_date, file_size, age_rating,
                languages, trailer_url, cover_image, category_id, published_at
              ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
              RETURNING id"#,
        )
        .bind(user_id)
        .bind(&form.title)
        .bind(slug)
        .bind(&form.excerpt)
        .bind(&form.content)
        .bind(status)
        .bind(&form.version)
        .bind(&form.developer)
        .bind(&form.publisher)
        .bind(release_date)
        .bind(&form.file_size)
        .bind(age_rating)
        .bind(&languages)
        .bind(&form.trailer_url)
        .bind(&form.cover_image)
        .bind(category_id)
        .bind(published_at)
        .fetch_one(pool)
        .await?;

        // Insert links
        Self::sync_links(pool, id, form).await?;

        // Insert screenshots
        for (i, url) in form.screenshots_vec().iter().enumerate() {
            let _ = sqlx::query(
                r#"INSERT INTO game_screenshots (game_id, url, position) VALUES ($1, $2, $3)"#,
            )
            .bind(id)
            .bind(url)
            .bind(i as i32)
            .execute(pool)
            .await;
        }

        // Insert tags
        for tag in form.tags_vec() {
            let tag = tag.trim();
            if tag.is_empty() {
                continue;
            }
            let slug = slug::slugify(tag);
            let _ = sqlx::query(
                r#"INSERT INTO tags (name, slug) VALUES ($1, $2)
                   ON CONFLICT (slug) DO UPDATE SET usage_count = tags.usage_count"#,
            )
            .bind(tag)
            .bind(&slug)
            .execute(pool)
            .await;
            let tag_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM tags WHERE slug = $1")
                .bind(&slug)
                .fetch_optional(pool)
                .await?;
            if let Some(tid) = tag_id {
                let _ = sqlx::query(
                    "INSERT INTO game_tags (game_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                )
                .bind(id)
                .bind(tid)
                .execute(pool)
                .await;
            }
        }

        Ok(id)
    }

    pub async fn update(pool: &PgPool, id: Uuid, form: &GameForm) -> AppResult<()> {
        let status = GameStatus::from_str(&form.status);
        let age_rating = AgeRating::from_str(&form.age_rating);
        let release_date = form
            .release_date
            .as_deref()
            .and_then(crate::utils::parse_date);
        let category_id = form
            .category_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .and_then(|s| Uuid::parse_str(s).ok());
        let languages = if form.languages_vec().is_empty() {
            vec!["vi".to_string()]
        } else {
            form.languages_vec()
        };

        sqlx::query(
            r#"UPDATE games SET
                title = $1, excerpt = $2, content = $3, status = $4, version = $5,
                developer = $6, publisher = $7, release_date = $8, file_size = $9,
                age_rating = $10, languages = $11, trailer_url = $12, cover_image = $13,
                category_id = $14,
                published_at = CASE WHEN $4 = 'published' AND published_at IS NULL THEN NOW() ELSE published_at END
              WHERE id = $15"#,
        )
        .bind(&form.title)
        .bind(&form.excerpt)
        .bind(&form.content)
        .bind(status)
        .bind(&form.version)
        .bind(&form.developer)
        .bind(&form.publisher)
        .bind(release_date)
        .bind(&form.file_size)
        .bind(age_rating)
        .bind(&languages)
        .bind(&form.trailer_url)
        .bind(&form.cover_image)
        .bind(category_id)
        .bind(id)
        .execute(pool)
        .await?;

        // Replace links
        sqlx::query("DELETE FROM game_links WHERE game_id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Self::sync_links(pool, id, form).await?;

        // Replace screenshots
        sqlx::query("DELETE FROM game_screenshots WHERE game_id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        for (i, url) in form.screenshots_vec().iter().enumerate() {
            let _ = sqlx::query(
                r#"INSERT INTO game_screenshots (game_id, url, position) VALUES ($1, $2, $3)"#,
            )
            .bind(id)
            .bind(url)
            .bind(i as i32)
            .execute(pool)
            .await;
        }

        // Replace tags
        sqlx::query("DELETE FROM game_tags WHERE game_id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        for tag in form.tags_vec() {
            let tag = tag.trim();
            if tag.is_empty() {
                continue;
            }
            let slug = slug::slugify(tag);
            let _ = sqlx::query(
                r#"INSERT INTO tags (name, slug) VALUES ($1, $2)
                   ON CONFLICT (slug) DO UPDATE SET usage_count = tags.usage_count"#,
            )
            .bind(tag)
            .bind(&slug)
            .execute(pool)
            .await;
            let tag_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM tags WHERE slug = $1")
                .bind(&slug)
                .fetch_optional(pool)
                .await?;
            if let Some(tid) = tag_id {
                let _ = sqlx::query(
                    "INSERT INTO game_tags (game_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                )
                .bind(id)
                .bind(tid)
                .execute(pool)
                .await;
            }
        }
        Ok(())
    }

    async fn sync_links(pool: &PgPool, game_id: Uuid, form: &GameForm) -> AppResult<()> {
        let links: [(&str, Platform, &Option<String>); 5] = [
            ("android", Platform::Android, &form.android_link),
            ("ios", Platform::Ios, &form.ios_link),
            ("windows", Platform::Windows, &form.windows_link),
            ("linux", Platform::Linux, &form.linux_link),
            ("macos", Platform::Macos, &form.macos_link),
        ];
        for (_name, platform, link_opt) in links {
            if let Some(url) = link_opt.as_deref().filter(|s| !s.is_empty()) {
                let _ = sqlx::query(
                    r#"INSERT INTO game_links (game_id, platform, url) VALUES ($1, $2, $3)
                       ON CONFLICT (game_id, platform) DO UPDATE SET url = EXCLUDED.url"#,
                )
                .bind(game_id)
                .bind(platform)
                .bind(url)
                .execute(pool)
                .await;
            }
        }
        Ok(())
    }

    pub async fn find_by_slug(pool: &PgPool, slug: &str) -> AppResult<Option<Game>> {
        let game = sqlx::query_as::<_, Game>(
            r#"SELECT id, user_id, title, slug, excerpt, content, status, version,
                developer, publisher, release_date, file_size, age_rating, languages,
                trailer_url, cover_image, category_id, view_count, download_count,
                like_count, comment_count, share_count, rating_avg, rating_count,
                is_featured, published_at, created_at, updated_at
              FROM games WHERE slug = $1"#,
        )
        .bind(slug)
        .fetch_optional(pool)
        .await?;
        Ok(game)
    }

    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<Game>> {
        let game = sqlx::query_as::<_, Game>(
            r#"SELECT id, user_id, title, slug, excerpt, content, status, version,
                developer, publisher, release_date, file_size, age_rating, languages,
                trailer_url, cover_image, category_id, view_count, download_count,
                like_count, comment_count, share_count, rating_avg, rating_count,
                is_featured, published_at, created_at, updated_at
              FROM games WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(game)
    }

    pub async fn get_links(pool: &PgPool, game_id: Uuid) -> AppResult<Vec<GameLink>> {
        let links = sqlx::query_as::<_, GameLink>(
            r#"SELECT id, game_id, platform, url, created_at FROM game_links WHERE game_id = $1"#,
        )
        .bind(game_id)
        .fetch_all(pool)
        .await?;
        Ok(links)
    }

    pub async fn get_link_for_platform(
        pool: &PgPool,
        game_id: Uuid,
        platform: &Platform,
    ) -> AppResult<Option<String>> {
        let url: Option<String> =
            sqlx::query_scalar("SELECT url FROM game_links WHERE game_id = $1 AND platform = $2")
                .bind(game_id)
                .bind(platform)
                .fetch_optional(pool)
                .await?;
        Ok(url)
    }

    pub async fn get_screenshots(pool: &PgPool, game_id: Uuid) -> AppResult<Vec<GameScreenshot>> {
        let shots = sqlx::query_as::<_, GameScreenshot>(
            r#"SELECT id, game_id, url, caption, position, created_at
              FROM game_screenshots WHERE game_id = $1 ORDER BY position"#,
        )
        .bind(game_id)
        .fetch_all(pool)
        .await?;
        Ok(shots)
    }

    pub async fn get_tags(pool: &PgPool, game_id: Uuid) -> AppResult<Vec<String>> {
        let tags: Vec<String> = sqlx::query_scalar(
            r#"SELECT t.name FROM tags t
              JOIN game_tags gt ON gt.tag_id = t.id
              WHERE gt.game_id = $1 ORDER BY t.name"#,
        )
        .bind(game_id)
        .fetch_all(pool)
        .await?;
        Ok(tags)
    }

    pub async fn increment_view_count(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE games SET view_count = view_count + 1 WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn increment_download_count(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE games SET download_count = download_count + 1 WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn increment_share_count(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE games SET share_count = share_count + 1 WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM games WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn set_status(pool: &PgPool, id: Uuid, status: &str) -> AppResult<()> {
        sqlx::query("UPDATE games SET status = $1::game_status WHERE id = $2")
            .bind(status)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn set_featured(pool: &PgPool, id: Uuid, featured: bool) -> AppResult<()> {
        sqlx::query("UPDATE games SET is_featured = $1 WHERE id = $2")
            .bind(featured)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    // ============ Listing queries ============
    pub async fn list_published(
        pool: &PgPool,
        limit: i64,
        offset: i64,
        sort: &str,
    ) -> AppResult<Vec<GameCard>> {
        let order = match sort {
            "trending" => "g.view_count DESC",
            "downloads" => "g.download_count DESC",
            "top_rated" => "g.rating_avg DESC, g.rating_count DESC",
            "liked" => "g.like_count DESC",
            _ => "g.published_at DESC NULLS LAST",
        };
        let sql = format!(
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
              FROM games g
              LEFT JOIN users u ON u.id = g.user_id
              LEFT JOIN categories c ON c.id = g.category_id
              WHERE g.status = 'published'
              ORDER BY {}
              LIMIT $1 OFFSET $2"#,
            order
        );
        // order clause là hằng số nội bộ (match ở trên), an toàn injection
        let cards = sqlx::query_as::<_, GameCard>(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;
        Ok(cards)
    }

    pub async fn search(
        pool: &PgPool,
        query: &str,
        category_slug: Option<&str>,
        platform: Option<&str>,
        sort: &str,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<GameCard>> {
        let order = match sort {
            "trending" => "g.view_count DESC",
            "downloads" => "g.download_count DESC",
            "top_rated" => "g.rating_avg DESC, g.rating_count DESC",
            "liked" => "g.like_count DESC",
            _ => "g.published_at DESC NULLS LAST",
        };

        let mut sql = r#"SELECT g.id, g.slug, g.title, g.excerpt, g.cover_image,
                c.name as category_name, c.slug as category_slug,
                u.display_name as author_name, u.avatar_url as author_avatar,
                g.view_count, g.download_count, g.like_count, g.comment_count,
                g.rating_avg, g.rating_count,
                COALESCE(
                  (SELECT array_agg(DISTINCT platform::text) FROM game_links WHERE game_id = g.id),
                  ARRAY[]::text[]
                ) as platforms,
                g.published_at
              FROM games g
              LEFT JOIN users u ON u.id = g.user_id
              LEFT JOIN categories c ON c.id = g.category_id
              WHERE g.status = 'published'
                AND (g.title ILIKE $1 OR g.excerpt ILIKE $1 OR g.content ILIKE $1)"#
            .to_string();
        if category_slug.is_some() {
            sql.push_str(" AND c.slug = $2");
        }
        if platform.is_some() {
            sql.push_str(&format!(
                " AND EXISTS (SELECT 1 FROM game_links gl WHERE gl.game_id = g.id AND gl.platform = ${})",
                if category_slug.is_some() { 3 } else { 2 }
            ));
        }
        sql.push_str(&format!(
            " ORDER BY {} LIMIT ${} OFFSET ${}",
            order,
            if category_slug.is_some() && platform.is_some() {
                4
            } else if category_slug.is_some() || platform.is_some() {
                3
            } else {
                2
            },
            if category_slug.is_some() && platform.is_some() {
                5
            } else if category_slug.is_some() || platform.is_some() {
                4
            } else {
                3
            }
        ));

        let pattern = format!("%{}%", query);
        // order/where clause được ghép từ hằng số nội bộ, an toàn injection
        let mut q = sqlx::query_as::<_, GameCard>(sqlx::AssertSqlSafe(sql.as_str())).bind(pattern);
        if let Some(cs) = category_slug {
            q = q.bind(cs);
        }
        if let Some(p) = platform {
            // p is platform string like "android" - we need to bind the enum
            let plat = Platform::from_str(p).ok_or_else(|| {
                crate::error::AppError::BadRequest("Platform không hợp lệ".into())
            })?;
            q = q.bind(plat);
        }
        q = q.bind(limit).bind(offset);
        let cards = q.fetch_all(pool).await?;
        Ok(cards)
    }

    pub async fn by_user(
        pool: &PgPool,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<GameCard>> {
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
              FROM games g
              LEFT JOIN users u ON u.id = g.user_id
              LEFT JOIN categories c ON c.id = g.category_id
              WHERE g.user_id = $1 AND g.status = 'published'
              ORDER BY g.published_at DESC NULLS LAST
              LIMIT $2 OFFSET $3"#,
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(cards)
    }

    pub async fn related(
        pool: &PgPool,
        game_id: Uuid,
        category_id: Option<Uuid>,
        limit: i64,
    ) -> AppResult<Vec<GameCard>> {
        let cards = if let Some(cat) = category_id {
            sqlx::query_as::<_, GameCard>(
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
                  FROM games g
                  LEFT JOIN users u ON u.id = g.user_id
                  LEFT JOIN categories c ON c.id = g.category_id
                  WHERE g.id != $1 AND g.category_id = $2 AND g.status = 'published'
                  ORDER BY g.view_count DESC LIMIT $3"#,
            )
            .bind(game_id)
            .bind(cat)
            .bind(limit)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as::<_, GameCard>(
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
                  FROM games g
                  LEFT JOIN users u ON u.id = g.user_id
                  LEFT JOIN categories c ON c.id = g.category_id
                  WHERE g.id != $1 AND g.status = 'published'
                  ORDER BY g.download_count DESC LIMIT $2"#,
            )
            .bind(game_id)
            .bind(limit)
            .fetch_all(pool)
            .await?
        };
        Ok(cards)
    }

    pub async fn by_tag(
        pool: &PgPool,
        tag_slug: &str,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<GameCard>> {
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
              FROM games g
              LEFT JOIN users u ON u.id = g.user_id
              LEFT JOIN categories c ON c.id = g.category_id
              JOIN game_tags gt ON gt.game_id = g.id
              JOIN tags t ON t.id = gt.tag_id
              WHERE t.slug = $1 AND g.status = 'published'
              ORDER BY g.published_at DESC NULLS LAST
              LIMIT $2 OFFSET $3"#,
        )
        .bind(tag_slug)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(cards)
    }

    pub async fn by_category(
        pool: &PgPool,
        cat_slug: &str,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<GameCard>> {
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
              FROM games g
              LEFT JOIN users u ON u.id = g.user_id
              LEFT JOIN categories c ON c.id = g.category_id
              WHERE c.slug = $1 AND g.status = 'published'
              ORDER BY g.published_at DESC NULLS LAST
              LIMIT $2 OFFSET $3"#,
        )
        .bind(cat_slug)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(cards)
    }

    pub async fn count_slug(pool: &PgPool, slug_base: &str) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM games WHERE slug LIKE $1")
            .bind(format!("{}%", slug_base))
            .fetch_one(pool)
            .await?;
        Ok(count)
    }

    pub async fn count_published(pool: &PgPool) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM games WHERE status = 'published'")
            .fetch_one(pool)
            .await?;
        Ok(c)
    }

    pub async fn featured(pool: &PgPool, limit: i64) -> AppResult<Vec<GameCard>> {
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
              FROM games g
              LEFT JOIN users u ON u.id = g.user_id
              LEFT JOIN categories c ON c.id = g.category_id
              WHERE g.status = 'published' AND g.is_featured = TRUE
              ORDER BY g.published_at DESC NULLS LAST LIMIT $1"#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(cards)
    }

    pub async fn pending_reports_count(pool: &PgPool) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM reports WHERE status = 'pending'")
            .fetch_one(pool)
            .await?;
        Ok(c)
    }

    // ===== Admin & quản lý =====

    /// Danh sách game cho admin (mọi trạng thái, kèm filter)
    pub async fn admin_list(
        pool: &PgPool,
        status: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<AdminGameRow>> {
        let rows = match status {
            Some(s) if !s.is_empty() => {
                sqlx::query_as::<_, AdminGameRow>(
                    r#"SELECT g.id, g.slug, g.title, g.status, g.view_count, g.download_count,
                    g.like_count, g.comment_count, g.is_featured, g.created_at,
                    u.display_name as author_name, c.name as category_name
                  FROM games g JOIN users u ON u.id = g.user_id
                  LEFT JOIN categories c ON c.id = g.category_id
                  WHERE g.status = $1
                  ORDER BY g.created_at DESC LIMIT $2 OFFSET $3"#,
                )
                .bind(s)
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?
            }
            _ => {
                sqlx::query_as::<_, AdminGameRow>(
                    r#"SELECT g.id, g.slug, g.title, g.status, g.view_count, g.download_count,
                    g.like_count, g.comment_count, g.is_featured, g.created_at,
                    u.display_name as author_name, c.name as category_name
                  FROM games g JOIN users u ON u.id = g.user_id
                  LEFT JOIN categories c ON c.id = g.category_id
                  ORDER BY g.created_at DESC LIMIT $1 OFFSET $2"#,
                )
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?
            }
        };
        Ok(rows)
    }

    pub async fn count_by_status(pool: &PgPool) -> AppResult<Vec<(String, i64)>> {
        let rows: Vec<(String, i64)> =
            sqlx::query_as("SELECT status::text, COUNT(*)::bigint FROM games GROUP BY status")
                .fetch_all(pool)
                .await?;
        Ok(rows)
    }

    /// Tất cả game của 1 user (kể cả draft/hidden) cho trang "Game của tôi"
    pub async fn all_by_user(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<AdminGameRow>> {
        let rows = sqlx::query_as::<_, AdminGameRow>(
            r#"SELECT g.id, g.slug, g.title, g.status, g.view_count, g.download_count,
                g.like_count, g.comment_count, g.is_featured, g.created_at,
                u.display_name as author_name, c.name as category_name
              FROM games g JOIN users u ON u.id = g.user_id
              LEFT JOIN categories c ON c.id = g.category_id
              WHERE g.user_id = $1
              ORDER BY g.created_at DESC"#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Slug + updated_at cho sitemap
    pub async fn sitemap_entries(
        pool: &PgPool,
    ) -> AppResult<Vec<(String, chrono::DateTime<chrono::Utc>)>> {
        let rows: Vec<(String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
            "SELECT slug, updated_at FROM games WHERE status = 'published' ORDER BY updated_at DESC LIMIT 5000",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Game mới nhất cho RSS
    pub async fn latest_for_rss(pool: &PgPool, limit: i64) -> AppResult<Vec<GameCard>> {
        Self::list_published(pool, limit, 0, "latest").await
    }

    /// Đếm game trùng tiêu đề (cảnh báo khi tạo)
    pub async fn count_similar_title(pool: &PgPool, title: &str) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM games WHERE status = 'published' AND title ILIKE $1",
        )
        .bind(format!("%{}%", title))
        .fetch_one(pool)
        .await?;
        Ok(c)
    }
}
