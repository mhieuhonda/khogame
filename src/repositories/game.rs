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
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
            r"INSERT INTO games (
                user_id, title, slug, excerpt, content, status, version,
                developer, publisher, release_date, file_size, age_rating,
                languages, trailer_url, cover_image, category_id, published_at
              ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
              RETURNING id",
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

        // v2.6.0 — Batch INSERT screenshots: 1 query multi-row thay vì
        // N round-trip. Trước đây mỗi screenshot = 1 INSERT riêng →
        // 20 screenshots = 20 DB round-trip tuần tự, tăng latency TTFB
        // và khả năng pool exhaustion dưới tải cao.
        // Dùng QueryBuilder để an toàn bind dynamic-length args (sqlx 0.9
        // yêu cầu SqlSafeStr cho string động — QueryBuilder xử lý internally).
        let screenshots = form.screenshots_vec();
        if !screenshots.is_empty() {
            let mut builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
                "INSERT INTO game_screenshots (game_id, url, position) ",
            );
            builder.push_values(screenshots.iter().enumerate(), |mut b, (i, url)| {
                b.push_bind(id)
                    .push_bind(url)
                    .push_bind(i32::try_from(i).unwrap_or(i32::MAX));
            });
            builder.build().execute(pool).await?;
        }

        // Insert tags
        Self::sync_tags(pool, id, form.tags_vec()).await?;

        Ok(id)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
            r"UPDATE games SET
                title = $1, excerpt = $2, content = $3, status = $4, version = $5,
                developer = $6, publisher = $7, release_date = $8, file_size = $9,
                age_rating = $10, languages = $11, trailer_url = $12, cover_image = $13,
                category_id = $14,
                published_at = CASE WHEN $4 = 'published' AND published_at IS NULL THEN NOW() ELSE published_at END
              WHERE id = $15",
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

        // Replace screenshots — propagate lỗi (đồng bộ với create).
        // v2.6.0 — Batch INSERT 1 query thay vì N round-trip.
        sqlx::query("DELETE FROM game_screenshots WHERE game_id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        let screenshots = form.screenshots_vec();
        if !screenshots.is_empty() {
            let mut builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
                "INSERT INTO game_screenshots (game_id, url, position) ",
            );
            builder.push_values(screenshots.iter().enumerate(), |mut b, (i, url)| {
                b.push_bind(id)
                    .push_bind(url)
                    .push_bind(i32::try_from(i).unwrap_or(i32::MAX));
            });
            builder.build().execute(pool).await?;
        }

        // Replace tags
        Self::sync_tags(pool, id, form.tags_vec()).await?;
        Ok(())
    }

    /// Gắn tags cho game. `tags.usage_count` được tăng/giảm bởi DB trigger
    /// (`trigger_game_tag_insert/delete` trên `game_tags`), nên ở đây chỉ cần
    /// thay thế các dòng `game_tags` — KHÔNG tự cộng trừ `usage_count`.
    async fn sync_tags(pool: &PgPool, game_id: Uuid, tags: Vec<String>) -> AppResult<()> {
        // Xoá liên kết cũ (trigger tự giảm usage_count từng tag bị gỡ)
        sqlx::query("DELETE FROM game_tags WHERE game_id = $1")
            .bind(game_id)
            .execute(pool)
            .await?;

        // v2.6.0 — Dedup + collect trước, rồi batch INSERT 1 query thay vì
        // N×2 round-trip (INSERT INTO tags RETURNING id + INSERT game_tags
        // per tag). Trước đây 20 tags = 40+ sequential round-trips.
        let mut seen = std::collections::HashSet::new();
        let mut unique: Vec<(String, String)> = Vec::with_capacity(tags.len());
        for tag in tags {
            let tag = tag.trim().to_string();
            if tag.is_empty() {
                continue;
            }
            let tag_slug = slug::slugify(&tag);
            if tag_slug.is_empty() || !seen.insert(tag_slug.clone()) {
                continue;
            }
            unique.push((tag, tag_slug));
        }
        if unique.is_empty() {
            return Ok(());
        }
        // Batch upsert tags (RETURNING id) — 1 query cho tất cả tags.
        let mut tag_builder =
            sqlx::QueryBuilder::<sqlx::Postgres>::new("INSERT INTO tags (name, slug) ");
        tag_builder.push_values(unique.iter(), |mut b, (name, slug)| {
            b.push_bind(name).push_bind(slug);
        });
        tag_builder.push(" ON CONFLICT (slug) DO UPDATE SET name = EXCLUDED.name RETURNING id");
        let tag_ids: Vec<Uuid> = tag_builder
            .build_query_as::<(Uuid,)>()
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|r| r.0)
            .collect();

        if !tag_ids.is_empty() {
            // Batch INSERT game_tags — 1 query.
            let mut gt_builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
                "INSERT INTO game_tags (game_id, tag_id) ",
            );
            gt_builder.push_values(tag_ids.iter(), |mut b, tid| {
                b.push_bind(game_id).push_bind(tid);
            });
            gt_builder.push(" ON CONFLICT DO NOTHING");
            gt_builder.build().execute(pool).await?;
        }
        Ok(())
    }

    async fn sync_links(pool: &PgPool, game_id: Uuid, form: &GameForm) -> AppResult<()> {
        // v2.6.0 — Collect links, batch INSERT 1 query thay vì 5 sequential.
        let links: [(&str, Platform, &Option<String>); 5] = [
            ("android", Platform::Android, &form.android_link),
            ("ios", Platform::Ios, &form.ios_link),
            ("windows", Platform::Windows, &form.windows_link),
            ("linux", Platform::Linux, &form.linux_link),
            ("macos", Platform::Macos, &form.macos_link),
        ];
        let collected: Vec<(Platform, &str)> = links
            .into_iter()
            .filter_map(|(_n, platform, link_opt)| {
                link_opt
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .map(|url| (platform, url))
            })
            .collect();
        if collected.is_empty() {
            return Ok(());
        }
        let mut builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
            "INSERT INTO game_links (game_id, platform, url) ",
        );
        builder.push_values(collected.iter(), |mut b, (platform, url)| {
            b.push_bind(game_id)
                .push_bind(platform.clone())
                .push_bind(url);
        });
        builder.push(" ON CONFLICT (game_id, platform) DO UPDATE SET url = EXCLUDED.url");
        builder.build().execute(pool).await?;
        Ok(())
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_by_slug(pool: &PgPool, slug: &str) -> AppResult<Option<Game>> {
        let game = sqlx::query_as::<_, Game>(
            r"SELECT id, user_id, title, slug, excerpt, content, status, version,
                developer, publisher, release_date, file_size, age_rating, languages,
                trailer_url, cover_image, category_id, view_count, download_count,
                like_count, comment_count, share_count, rating_avg, rating_count,
                is_featured, published_at, created_at, updated_at
              FROM games WHERE slug = $1",
        )
        .bind(slug)
        .fetch_optional(pool)
        .await?;
        Ok(game)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<Game>> {
        let game = sqlx::query_as::<_, Game>(
            r"SELECT id, user_id, title, slug, excerpt, content, status, version,
                developer, publisher, release_date, file_size, age_rating, languages,
                trailer_url, cover_image, category_id, view_count, download_count,
                like_count, comment_count, share_count, rating_avg, rating_count,
                is_featured, published_at, created_at, updated_at
              FROM games WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(game)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn get_links(pool: &PgPool, game_id: Uuid) -> AppResult<Vec<GameLink>> {
        let links = sqlx::query_as::<_, GameLink>(
            r"SELECT id, game_id, platform, url, created_at FROM game_links WHERE game_id = $1",
        )
        .bind(game_id)
        .fetch_all(pool)
        .await?;
        Ok(links)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn get_screenshots(pool: &PgPool, game_id: Uuid) -> AppResult<Vec<GameScreenshot>> {
        let shots = sqlx::query_as::<_, GameScreenshot>(
            r"SELECT id, game_id, url, caption, position, created_at
              FROM game_screenshots WHERE game_id = $1 ORDER BY position",
        )
        .bind(game_id)
        .fetch_all(pool)
        .await?;
        Ok(shots)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn get_tags(pool: &PgPool, game_id: Uuid) -> AppResult<Vec<String>> {
        let tags: Vec<String> = sqlx::query_scalar(
            r"SELECT t.name FROM tags t
              JOIN game_tags gt ON gt.tag_id = t.id
              WHERE gt.game_id = $1 ORDER BY t.name",
        )
        .bind(game_id)
        .fetch_all(pool)
        .await?;
        Ok(tags)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn increment_view_count(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE games SET view_count = view_count + 1 WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn increment_download_count(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE games SET download_count = download_count + 1 WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn increment_share_count(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE games SET share_count = share_count + 1 WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn delete(pool: &PgPool, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM games WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn set_status(pool: &PgPool, id: Uuid, status: &str) -> AppResult<()> {
        sqlx::query("UPDATE games SET status = $1::game_status WHERE id = $2")
            .bind(status)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Xuất bản game: đặt status='published' và giữ `published_at` cũ nếu
    /// đã có (COALESCE) — để re-publish không reset mốc xuất bản gốc,
    /// ảnh hưởng đến thứ tự sort "latest" và sitemap lastmod.
    ///
    /// v3.0.0 FIX (XP farm): chỉ UPDATE khi `status <> 'published'` và trả
    /// về `true` nếu game MỚI được publish ở lần gọi này. Trước đây
    /// UPDATE vô điều kiện + caller luôn coi là "mới publish" → POST
    /// `/games/{slug}/publish` lặp lại được +50 XP mỗi lần (hook
    /// on_game_published fire lại) + spam notification cho followers.
    ///
    /// v3.5.1 FIX (XP farm vòng lặp draft→publish, HIGH): v3.0.0 chỉ chặn
    /// publish 2 lần LIÊN TIẾP — owner vẫn dựng vòng lặp qua form edit
    /// (đặt status=draft) rồi gọi `/publish` lại: mỗi vòng `status <> 'published'`
    /// đều thoả → +50 XP + notification-toàn-bộ-followers mỗi cycle (~3.000
    /// XP/phút). Giờ hook CHỈ fire khi game CHƯA TỪNG publish lần nào
    /// (`published_at IS NULL` trước update — mốc này được COALESCE bảo
    /// toàn, không reset khi về draft). Re-publish sau khi về draft vẫn
    /// set status='published' bình thường, chỉ không cộng XP/spam nữa.
    /// `SELECT ... FOR UPDATE` trong tx chống 2 request publish đua nhau.
    /// # Returns
    /// `true` nếu đây là lần publish ĐẦU TIÊN của game → caller fire hook XP.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn publish(pool: &PgPool, id: Uuid) -> AppResult<bool> {
        let mut tx = pool.begin().await?;
        let row: Option<(bool, bool)> = sqlx::query_as(
            "SELECT (status = 'published'), (published_at IS NOT NULL) \
             FROM games WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        // Game không tồn tại (hoặc bị xoá giữa chừng) → không có gì để publish.
        let Some((already_published, ever_published)) = row else {
            return Ok(false);
        };
        if !already_published {
            sqlx::query(
                "UPDATE games SET status = 'published', \
                 published_at = COALESCE(published_at, NOW()) \
                 WHERE id = $1",
            )
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        // Hook XP/notification chỉ fire khi: game vừa chuyển sang published
        // VÀ chưa từng được publish lần nào trước đó.
        Ok(!already_published && !ever_published)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn set_featured(pool: &PgPool, id: Uuid, featured: bool) -> AppResult<()> {
        sqlx::query("UPDATE games SET is_featured = $1 WHERE id = $2")
            .bind(featured)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    // ============ Listing queries ============
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
            r"SELECT g.id, g.slug, g.title, g.excerpt, g.cover_image,
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
              ORDER BY {order}
              LIMIT $1 OFFSET $2"
        );
        // order clause là hằng số nội bộ (match ở trên), an toàn injection
        let cards = sqlx::query_as::<_, GameCard>(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;
        Ok(cards)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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

        let mut sql = r"SELECT g.id, g.slug, g.title, g.excerpt, g.cover_image,
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
                AND (g.title ILIKE $1 ESCAPE '\' OR g.excerpt ILIKE $1 ESCAPE '\' OR g.content ILIKE $1 ESCAPE '\')"
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

        // Escape wildcard %/_ để user tìm theo literal (tìm "100%" không
        // còn match cả "1001", "100x"...)
        let pattern = format!("%{}%", crate::utils::escape_like(query));
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

    /// Gợi ý tiêu đề game cho autocomplete ô tìm kiếm. Chỉ title + slug
    /// (query nhẹ), ưu tiên game nhiều view. Trả về tối đa `limit` gợi ý.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn suggest_titles(
        pool: &PgPool,
        query: &str,
        limit: i64,
    ) -> AppResult<Vec<(String, String)>> {
        let pattern = format!("%{}%", crate::utils::escape_like(query));
        let rows: Vec<(String, String)> = sqlx::query_as(
            r"SELECT title, slug FROM games
               WHERE status = 'published' AND title ILIKE $1 ESCAPE '\'
               ORDER BY view_count DESC, published_at DESC NULLS LAST
               LIMIT $2",
        )
        .bind(pattern)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn by_user(
        pool: &PgPool,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<GameCard>> {
        let cards = sqlx::query_as::<_, GameCard>(
            r"SELECT g.id, g.slug, g.title, g.excerpt, g.cover_image,
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
              LIMIT $2 OFFSET $3",
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(cards)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    /// v2.9.0 — Game hot trong 7 ngày qua (từ daily_stats — view + download
    /// có trọng số). Dùng cho "Game của tuần" + tab leaderboard.
    /// Fallback về view_count tổng nếu daily_stats trống (site mới).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn hot_this_week(pool: &PgPool, limit: i64) -> AppResult<Vec<GameCard>> {
        let cards = sqlx::query_as::<_, GameCard>(
            r"SELECT g.id, g.slug, g.title, g.excerpt, g.cover_image,
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
                AND EXISTS (
                    SELECT 1 FROM daily_stats ds
                    WHERE ds.game_id = g.id
                      AND ds.day >= (NOW() AT TIME ZONE 'Asia/Ho_Chi_Minh')::date - 7
                )
              ORDER BY (
                    SELECT COALESCE(SUM(ds.views + 2 * ds.downloads), 0)
                    FROM daily_stats ds
                    WHERE ds.game_id = g.id
                      AND ds.day >= (NOW() AT TIME ZONE 'Asia/Ho_Chi_Minh')::date - 7
                  ) DESC, g.view_count DESC
              LIMIT $1",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;
        if !cards.is_empty() {
            return Ok(cards);
        }
        // Fallback: site chưa có daily_stats — dùng view_count tổng
        Self::list_published(pool, limit, 0, "trending").await
    }

    /// v2.9.0 — Game NGẪU NHIÊN cho mục khám phá (sidebar / nút random).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn random_published(pool: &PgPool, limit: i64) -> AppResult<Vec<GameCard>> {
        let cards = sqlx::query_as::<_, GameCard>(
            r"SELECT g.id, g.slug, g.title, g.excerpt, g.cover_image,
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
              ORDER BY random()
              LIMIT $1",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(cards)
    }

    /// v2.9.0 — "Dành cho bạn": game published cùng thể loại với các game
    /// user đã like/bookmark, loại game đã xem. Dựa trên sở thích thực tế.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn recommended_for_user(
        pool: &PgPool,
        user_id: Uuid,
        limit: i64,
    ) -> AppResult<Vec<GameCard>> {
        let cards = sqlx::query_as::<_, GameCard>(
            r"SELECT g.id, g.slug, g.title, g.excerpt, g.cover_image,
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
                AND g.user_id <> $1
                AND g.category_id IS NOT NULL
                AND g.category_id IN (
                    -- Thể loại user tương tác (like hoặc bookmark)
                    SELECT DISTINCT g2.category_id FROM games g2
                    WHERE g2.category_id IS NOT NULL AND (
                        EXISTS (SELECT 1 FROM likes l
                                WHERE l.game_id = g2.id AND l.user_id = $1)
                        OR EXISTS (SELECT 1 FROM bookmarks b
                                   WHERE b.game_id = g2.id AND b.user_id = $1)
                    )
                )
                AND NOT EXISTS (
                    SELECT 1 FROM view_history vh
                    WHERE vh.game_id = g.id AND vh.user_id = $1
                )
                AND NOT EXISTS (
                    SELECT 1 FROM likes l2
                    WHERE l2.game_id = g.id AND l2.user_id = $1
                )
              ORDER BY g.rating_avg DESC, g.view_count DESC
              LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
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
                r"SELECT g.id, g.slug, g.title, g.excerpt, g.cover_image,
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
                  ORDER BY g.view_count DESC LIMIT $3",
            )
            .bind(game_id)
            .bind(cat)
            .bind(limit)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as::<_, GameCard>(
                r"SELECT g.id, g.slug, g.title, g.excerpt, g.cover_image,
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
                  ORDER BY g.download_count DESC LIMIT $2",
            )
            .bind(game_id)
            .bind(limit)
            .fetch_all(pool)
            .await?
        };
        Ok(cards)
    }

    /// Game theo tag — hỗ trợ sort động (trước đây ORDER BY cứng
    /// `published_at` DESC trong khi template vẫn render sort links).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn by_tag(
        pool: &PgPool,
        tag_slug: &str,
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
            r"SELECT g.id, g.slug, g.title, g.excerpt, g.cover_image,
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
              ORDER BY {order}
              LIMIT $2 OFFSET $3"
        );
        // order clause là hằng số nội bộ (match ở trên), an toàn injection
        let cards = sqlx::query_as::<_, GameCard>(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(tag_slug)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;
        Ok(cards)
    }

    /// Game theo thể loại — hỗ trợ sort động (trước đây ORDER BY cứng
    /// `published_at` DESC trong khi template vẫn render sort links).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn by_category(
        pool: &PgPool,
        cat_slug: &str,
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
            r"SELECT g.id, g.slug, g.title, g.excerpt, g.cover_image,
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
              ORDER BY {order}
              LIMIT $2 OFFSET $3"
        );
        // order clause là hằng số nội bộ (match ở trên), an toàn injection
        let cards = sqlx::query_as::<_, GameCard>(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(cat_slug)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;
        Ok(cards)
    }

    /// Kiểm tra chính xác 1 slug đã tồn tại hay chưa — dùng EXISTS thay
    /// vì COUNT(*) để Postgres dừng ngay khi tìm thấy dòng đầu (hàm này
    /// chạy trong vòng lặp sinh slug duy nhất lúc tạo game).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn slug_exists(pool: &PgPool, slug: &str) -> AppResult<bool> {
        let c: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM games WHERE slug = $1)")
            .bind(slug)
            .fetch_one(pool)
            .await?;
        Ok(c)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count_published(pool: &PgPool) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM games WHERE status = 'published'")
            .fetch_one(pool)
            .await?;
        Ok(c)
    }

    /// Số kết quả khớp bộ lọc search (để phân trang đúng)
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count_search(
        pool: &PgPool,
        query: &str,
        category_slug: Option<&str>,
        platform: Option<&str>,
    ) -> AppResult<i64> {
        let mut sql = String::from(
            r"SELECT COUNT(*) FROM games g
               LEFT JOIN categories c ON c.id = g.category_id
               WHERE g.status = 'published'
                 AND (g.title ILIKE $1 ESCAPE '\' OR g.excerpt ILIKE $1 ESCAPE '\' OR g.content ILIKE $1 ESCAPE '\')",
        );
        if category_slug.is_some() {
            sql.push_str(" AND c.slug = $2");
        }
        if platform.is_some() {
            sql.push_str(&format!(
                " AND EXISTS (SELECT 1 FROM game_links gl WHERE gl.game_id = g.id AND gl.platform = ${})",
                if category_slug.is_some() { 3 } else { 2 }
            ));
        }
        let pattern = format!("%{}%", crate::utils::escape_like(query));
        let mut q = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(sql.as_str())).bind(pattern);
        if let Some(cs) = category_slug {
            q = q.bind(cs);
        }
        if let Some(p) = platform {
            let plat = Platform::from_str(p).ok_or_else(|| {
                crate::error::AppError::BadRequest("Platform không hợp lệ".into())
            })?;
            q = q.bind(plat);
        }
        Ok(q.fetch_one(pool).await?)
    }

    /// Tổng số game trong 1 thể loại (published) để phân trang
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count_by_category(pool: &PgPool, cat_slug: &str) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar(
            r"SELECT COUNT(*) FROM games g
               JOIN categories c ON c.id = g.category_id
               WHERE c.slug = $1 AND g.status = 'published'",
        )
        .bind(cat_slug)
        .fetch_one(pool)
        .await?;
        Ok(c)
    }

    /// Tổng số game mang 1 tag (published) để phân trang
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count_by_tag(pool: &PgPool, tag_slug: &str) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar(
            r"SELECT COUNT(*) FROM games g
               JOIN game_tags gt ON gt.game_id = g.id
               JOIN tags t ON t.id = gt.tag_id
               WHERE t.slug = $1 AND g.status = 'published'",
        )
        .bind(tag_slug)
        .fetch_one(pool)
        .await?;
        Ok(c)
    }

    /// Tổng số game nổi bật để phân trang
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count_featured(pool: &PgPool) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM games WHERE status = 'published' AND is_featured = TRUE",
        )
        .fetch_one(pool)
        .await?;
        Ok(c)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn featured(pool: &PgPool, limit: i64, offset: i64) -> AppResult<Vec<GameCard>> {
        let cards = sqlx::query_as::<_, GameCard>(
            r"SELECT g.id, g.slug, g.title, g.excerpt, g.cover_image,
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
              ORDER BY g.published_at DESC NULLS LAST LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(cards)
    }

    /// v3.0.0 — "NGƯỜI CHƠI KHÁC CŨNG THÍCH": co-occurrence qua bảng
    /// likes — user thích game này cũng thích những game nào. Top 6,
    /// loại trừ chính game hiện tại.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn also_liked(pool: &PgPool, game_id: Uuid, limit: i64) -> AppResult<Vec<GameCard>> {
        let cards = sqlx::query_as::<_, GameCard>(
            r"SELECT g.id, g.slug, g.title, g.excerpt, g.cover_image,
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
              WHERE g.status = 'published' AND g.id IN (
                SELECT l2.game_id FROM likes l1
                JOIN likes l2 ON l2.user_id = l1.user_id
                WHERE l1.game_id = $1 AND l2.game_id <> $1
                GROUP BY l2.game_id
                ORDER BY COUNT(*) DESC
                LIMIT $2
              )",
        )
        .bind(game_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(cards)
    }

    /// v3.0.0 — GAME CỦA NGÀY: deterministic theo ngày VN (hashtext của
    /// id + ngày) — cùng ngày ai cũng thấy cùng 1 game, khác ngày khác
    /// game → lý do quay lại mỗi ngày. Rất rẻ (1 query, index scan).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn game_of_the_day(pool: &PgPool) -> AppResult<Option<GameCard>> {
        let sql = format!(
            r"SELECT g.id, g.slug, g.title, g.excerpt, g.cover_image,
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
              ORDER BY hashtext(g.id::text || {}::text)
              LIMIT 1",
            crate::utils::SQL_TODAY_VN
        );
        let card = sqlx::query_as::<_, GameCard>(sqlx::AssertSqlSafe(sql.as_str()))
            .fetch_optional(pool)
            .await?;
        Ok(card)
    }

    /// v3.0.0 — GAME SẮP RA MẤT: release_date >= hôm nay (giờ VN), gần nhất
    /// trước. Section "Sắp ra mắt" trên homepage — tạo cảm giác "sắp có
    /// gì đó xảy ra" giữ user quay lại theo dõi.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn upcoming_releases(
        pool: &PgPool,
        limit: i64,
    ) -> AppResult<Vec<crate::models::retention::UpcomingGame>> {
        let sql = format!(
            r"SELECT slug, title, cover_image, release_date
              FROM games
              WHERE status = 'published'
                AND release_date IS NOT NULL
                AND release_date >= {}
              ORDER BY release_date ASC
              LIMIT $1",
            crate::utils::SQL_TODAY_VN
        );
        let rows = sqlx::query_as::<_, crate::models::retention::UpcomingGame>(
            sqlx::AssertSqlSafe(sql.as_str()),
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn pending_reports_count(pool: &PgPool) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM reports WHERE status = 'pending'")
            .fetch_one(pool)
            .await?;
        Ok(c)
    }

    // ===== Admin & quản lý =====

    /// Danh sách game cho admin (mọi trạng thái, kèm filter)
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn admin_list(
        pool: &PgPool,
        status: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<AdminGameRow>> {
        let rows = match status {
            Some(s) if !s.is_empty() => {
                sqlx::query_as::<_, AdminGameRow>(
                    r"SELECT g.id, g.slug, g.title, g.status, g.view_count, g.download_count,
                    g.like_count, g.comment_count, g.is_featured, g.created_at,
                    u.display_name as author_name, c.name as category_name
                  FROM games g JOIN users u ON u.id = g.user_id
                  LEFT JOIN categories c ON c.id = g.category_id
                  WHERE g.status = $1::game_status
                  ORDER BY g.created_at DESC LIMIT $2 OFFSET $3",
                )
                .bind(s)
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?
            }
            _ => {
                sqlx::query_as::<_, AdminGameRow>(
                    r"SELECT g.id, g.slug, g.title, g.status, g.view_count, g.download_count,
                    g.like_count, g.comment_count, g.is_featured, g.created_at,
                    u.display_name as author_name, c.name as category_name
                  FROM games g JOIN users u ON u.id = g.user_id
                  LEFT JOIN categories c ON c.id = g.category_id
                  ORDER BY g.created_at DESC LIMIT $1 OFFSET $2",
                )
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?
            }
        };
        Ok(rows)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count_by_status(pool: &PgPool) -> AppResult<Vec<(String, i64)>> {
        let rows: Vec<(String, i64)> =
            sqlx::query_as("SELECT status::text, COUNT(*)::bigint FROM games GROUP BY status")
                .fetch_all(pool)
                .await?;
        Ok(rows)
    }

    /// Tổng số game theo bộ lọc trạng thái (phân trang admin đúng)
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count_admin(pool: &PgPool, status: Option<&str>) -> AppResult<i64> {
        match status {
            Some(s) if !s.is_empty() => {
                let c: i64 =
                    sqlx::query_scalar("SELECT COUNT(*) FROM games WHERE status = $1::game_status")
                        .bind(s)
                        .fetch_one(pool)
                        .await?;
                Ok(c)
            }
            _ => {
                let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM games")
                    .fetch_one(pool)
                    .await?;
                Ok(c)
            }
        }
    }

    /// Game của 1 user (kể cả draft/hidden) cho trang "Game của tôi" —
    /// phân trang LIMIT/OFFSET (trước đây trả TOÀN BỘ không giới hạn,
    /// user 500 game = 1 query nặng + bảng HTML khổng lồ).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn all_by_user(
        pool: &PgPool,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<AdminGameRow>> {
        let rows = sqlx::query_as::<_, AdminGameRow>(
            r"SELECT g.id, g.slug, g.title, g.status, g.view_count, g.download_count,
                g.like_count, g.comment_count, g.is_featured, g.created_at,
                u.display_name as author_name, c.name as category_name
              FROM games g JOIN users u ON u.id = g.user_id
              LEFT JOIN categories c ON c.id = g.category_id
              WHERE g.user_id = $1
              ORDER BY g.created_at DESC
              LIMIT $2 OFFSET $3",
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Tổng số game của user (phân trang trang "Game của tôi").
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count_all_by_user(pool: &PgPool, user_id: Uuid) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM games WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
        Ok(c)
    }

    /// Slug + `updated_at` cho sitemap
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn latest_for_rss(pool: &PgPool, limit: i64) -> AppResult<Vec<GameCard>> {
        Self::list_published(pool, limit, 0, "latest").await
    }

    /// Đếm game trùng tiêu đề (cảnh báo khi tạo)
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count_similar_title(pool: &PgPool, title: &str) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM games WHERE status = 'published' AND title ILIKE $1 ESCAPE '\\'",
        )
        .bind(format!("%{}%", crate::utils::escape_like(title)))
        .fetch_one(pool)
        .await?;
        Ok(c)
    }
}
