use crate::error::{AppError, AppResult};
use crate::middleware::AuthUser;
use crate::repositories::{
    CategoryRepo, GameRepo, NewsRepo, RepoRepo, SettingsRepo, TagRepo, UserRepo,
};
use crate::state::AppState;
use axum::extract::{Form, Path, Query, State};
use axum::http::header;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

// ===================== Public JSON API v1 =====================

#[derive(Serialize)]
pub struct ApiGame {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub excerpt: String,
    pub cover_image: Option<String>,
    pub category: Option<String>,
    pub author: String,
    pub platforms: Vec<String>,
    pub view_count: i32,
    pub download_count: i32,
    pub like_count: i32,
    pub comment_count: i32,
    pub rating_avg: f64,
    pub rating_count: i32,
    pub published_at: Option<String>,
}

#[derive(Serialize)]
pub struct ApiList<T> {
    pub data: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Deserialize, Default)]
pub struct ApiListQuery {
    pub page: Option<i64>,
    pub sort: Option<String>,
    pub q: Option<String>,
    /// Số game mỗi trang — client kiểm soát (1-50, mặc định 24).
    /// Cạnh trên 50 chống AI crawler kéo toàn bảng trong 1 request.
    pub per_page: Option<i64>,
}

pub async fn games_list(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ApiListQuery>,
) -> AppResult<Response> {
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = q.per_page.unwrap_or(24).clamp(1, 50);
    let offset = (page - 1) * per_page;
    let sort = q.sort.clone().unwrap_or_else(|| "latest".into());

    // Clamp từ khóa 200 ký tự — chống pattern khổng lồ làm ILIKE chậm.
    let q_search: Option<String> =
        q.q.as_deref()
            .map(|s| s.trim().chars().take(200).collect::<String>())
            .filter(|s| !s.is_empty());
    // search + count độc lập — join! song song.
    let (cards_res, total_res) = match q_search.as_deref() {
        Some(query) => {
            tokio::join!(
                GameRepo::search(&state.db, query, None, None, &sort, per_page, offset),
                GameRepo::count_search(&state.db, query, None, None),
            )
        }
        None => {
            tokio::join!(
                GameRepo::list_published(&state.db, per_page, offset, &sort),
                GameRepo::count_published(&state.db),
            )
        }
    };
    let cards = cards_res?;
    // Bug fix: khi có search query, total phải là số kết quả khớp query
    // chứ không phải tổng số game — nếu không client tính số trang sai.
    let total = total_res.unwrap_or(0);

    let data: Vec<ApiGame> = cards
        .iter()
        .map(|g| ApiGame {
            id: g.id,
            slug: g.slug.clone(),
            title: g.title.clone(),
            excerpt: g.excerpt.clone().unwrap_or_default(),
            cover_image: g.cover_image.clone(),
            category: g.category_name.clone(),
            author: g.author_name.clone(),
            platforms: g.platforms.clone(),
            view_count: g.view_count,
            download_count: g.download_count,
            like_count: g.like_count,
            comment_count: g.comment_count,
            rating_avg: g.rating_avg_f64(),
            rating_count: g.rating_count,
            published_at: g.published_at.map(|d| d.to_rfc3339()),
        })
        .collect();

    let body = ApiList {
        data,
        total,
        page,
        per_page,
    };
    Ok(([(header::CACHE_CONTROL, "public, max-age=60")], Json(body)).into_response())
}

pub async fn game_detail(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> AppResult<Response> {
    let g = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    // 5 query độc lập (links/tags/author/category/screenshots) chạy SONG
    // SONG — trước đây tuần tự, latency = tổng 5 lần round-trip DB. Trang
    // HTML show_game đã song song hoá từ trước nhưng API JSON bị bỏ sót.
    let (links_res, tags_res, author_res, category_res, screenshots_res) = tokio::join!(
        GameRepo::get_links(&state.db, g.id),
        GameRepo::get_tags(&state.db, g.id),
        UserRepo::find_by_id(&state.db, g.user_id),
        async {
            match g.category_id {
                Some(cat_id) => CategoryRepo::find_by_id(&state.db, cat_id).await,
                None => Ok(None),
            }
        },
        GameRepo::get_screenshots(&state.db, g.id),
    );
    let links = links_res?;
    let tags = tags_res?;
    let author = author_res?;
    // Lấy thêm category & screenshots để API public đầy đủ hơn (trước đây
    // API thiếu các trường này, khiến client phải gọi thêm nhiều endpoint).
    let category = category_res?;
    let screenshots = screenshots_res?;
    let body = serde_json::json!({
        "id": g.id,
        "slug": g.slug,
        "title": g.title,
        "excerpt": g.excerpt_or(),
        "status": format!("{:?}", g.status),
        "version": g.version,
        "developer": g.developer,
        "publisher": g.publisher,
        "release_date": g.release_date.map(|d| d.format("%Y-%m-%d").to_string()),
        "age_rating": format!("{:?}", g.age_rating),
        "languages": g.languages,
        "trailer_url": g.trailer_url,
        "cover_image": g.cover_image,
        "category": category.as_ref().map(|c| serde_json::json!({
            "slug": c.slug,
            "name": c.name,
        })),
        "author": author.map(|u| serde_json::json!({
            "username": u.username,
            "display_name": u.display_name,
            "avatar_url": u.avatar_url,
        })),
        "platforms": links.iter().map(|l| serde_json::json!({
            "platform": format!("{:?}", l.platform).to_lowercase(),
            "label": l.platform.label(),
            "url": l.url,
        })).collect::<Vec<_>>(),
        "screenshots": screenshots.iter().map(|s| serde_json::json!({
            "url": s.url,
            "caption": s.caption,
            "position": s.position,
        })).collect::<Vec<_>>(),
        "tags": tags,
        "stats": {
            "views": g.view_count,
            "downloads": g.download_count,
            "likes": g.like_count,
            "comments": g.comment_count,
            "rating_avg": g.rating_avg_f64(),
            "rating_count": g.rating_count,
        },
        "created_at": g.created_at.to_rfc3339(),
        "updated_at": g.updated_at.to_rfc3339(),
        "published_at": g.published_at.map(|d| d.to_rfc3339()),
    });
    Ok(([(header::CACHE_CONTROL, "public, max-age=60")], Json(body)).into_response())
}

/// Bình luận của game dạng JSON công khai (top-level, phân trang).
///
/// Client bên ngoài có thể dựng widget bình luận mà không cào HTML.
/// Chỉ comment của game đã xuất bản; replies tải riêng qua /comments/{id}/replies.
pub async fn game_comments(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Query(q): Query<ApiListQuery>,
) -> AppResult<Response> {
    let g = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    // Chỉ game published mới công khai bình luận — đồng bộ với trang HTML.
    if g.status != crate::models::game::GameStatus::Published {
        return Err(AppError::NotFound("Game không tồn tại".into()));
    }
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 50;
    let offset = (page - 1) * per_page;
    let comments = crate::repositories::CommentRepo::list_by_game(
        &state.db, g.id,
        None, // viewer ẩn danh — is_liked luôn false trong API công khai
        per_page, offset,
    )
    .await?;
    let data: Vec<serde_json::Value> = comments
        .iter()
        .map(|c| {
            serde_json::json!({
                "id": c.id,
                "parent_id": c.parent_id,
                "content": c.content,
                "like_count": c.like_count,
                "is_pinned": c.is_pinned,
                "author": {
                    "name": c.user_name,
                    "avatar_url": c.user_avatar,
                },
                "created_at": c.created_at.to_rfc3339(),
            })
        })
        .collect();
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=60")],
        Json(serde_json::json!({
            "data": data,
            "total": g.comment_count,
            "page": page,
            "per_page": per_page,
        })),
    )
        .into_response())
}

pub async fn repos_list(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ApiListQuery>,
) -> AppResult<Response> {
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 30;
    let offset = (page - 1) * per_page;
    let sort = q.sort.unwrap_or_else(|| "stars".into());
    let (repos_res, total_res) = tokio::join!(
        RepoRepo::list_approved(&state.db, per_page, offset, &sort),
        RepoRepo::count_approved(&state.db),
    );
    let repos = repos_res?;
    let total = total_res.unwrap_or(0);
    let data: Vec<serde_json::Value> = repos
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "full_name": r.full_name(),
                "url": r.html_url(),
                "description": r.description,
                "language": r.primary_language,
                "stars": r.stars,
                "forks": r.forks,
                "open_issues": r.open_issues,
                "author": {
                    "username": r.author_username,
                    "display_name": r.author_name,
                },
                "linked_game": if let (Some(s), Some(t)) = (&r.game_slug, &r.game_title) {
                    serde_json::json!({"slug": s, "title": t})
                } else { serde_json::Value::Null },
                "created_at": r.created_at.to_rfc3339(),
            })
        })
        .collect();
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=120")],
        Json(ApiList {
            data,
            total,
            page,
            per_page,
        }),
    )
        .into_response())
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn stats_overview(State(state): State<Arc<AppState>>) -> AppResult<Response> {
    // 6 COUNT độc lập — join! song song (cache 60s đã giảm tần suất,
    // giờ giảm cả latency của mỗi lần cache miss). Thêm news stats.
    let (total_games, total_users, total_repos, total_downloads, total_comments, total_news) = tokio::join!(
        GameRepo::count_published(&state.db),
        UserRepo::count_all(&state.db),
        RepoRepo::count_approved(&state.db),
        async {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM downloads")
                .fetch_one(&state.db)
                .await
                .unwrap_or(0)
        },
        async {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM comments")
                .fetch_one(&state.db)
                .await
                .unwrap_or(0)
        },
        NewsRepo::count_by_status(&state.db, crate::models::news::NewsStatus::Published),
    );
    let total_games = total_games.unwrap_or(0);
    let total_users = total_users.unwrap_or(0);
    let total_repos = total_repos.unwrap_or(0);
    let total_news = total_news.unwrap_or(0);
    // Cache 60s: số liệu thống kê không cần real-time, giảm 6 COUNT(*)
    // xuống còn tối đa 1 lần/phút mỗi client.
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=60")],
        Json(serde_json::json!({
            "total_games": total_games,
            "total_users": total_users,
            "total_repos": total_repos,
            "total_downloads": total_downloads,
            "total_comments": total_comments,
            "total_news": total_news,
        })),
    )
        .into_response())
}

// ===================== Nội bộ dùng chung =====================

/// Banner thông báo toàn site (layout fetch qua htmx)
pub async fn announcement(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> AppResult<Response> {
    // 1 query lấy cả 2 key thay vì 2 query tuần tự (mỗi page view đều gọi)
    let mut map = SettingsRepo::get_map(&state.db, &["announcement", "announcement_type"]).await?;
    let text = map.remove("announcement").unwrap_or_default();
    let kind = map
        .remove("announcement_type")
        .unwrap_or_else(|| "info".into());
    // Cache browser 60s: JS fetch endpoint này trên MỌI page view —
    // không có Cache-Control thì mỗi lần chuyển trang vẫn đánh DB.
    // Announcement là setting admin đổi hiếm, trễ 1 phút chấp nhận được.
    //
    // ETag thêm 1 tầng: nếu nội dung KHÔNG đổi trong cửa sổ 60s,
    // If-None-Match khớp → 304 rỗng (vài chục byte) thay vì payload
    // JSON đầy đủ — tiết kiệm băng thông cho user quay lại liên tục.
    let etag = format!("\"{}\"", short_hash(&format!("{text}|{kind}")));
    if etag_matches(&headers, &etag) {
        return Ok((
            StatusCode::NOT_MODIFIED,
            [
                (header::ETAG, etag.as_str()),
                (header::CACHE_CONTROL, "public, max-age=60"),
            ],
        )
            .into_response());
    }
    if text.is_empty() {
        return Ok((
            [
                (header::ETAG, etag.as_str()),
                (header::CACHE_CONTROL, "public, max-age=60"),
            ],
            Json(serde_json::json!({"text": "", "kind": ""})),
        )
            .into_response());
    }
    Ok((
        [
            (header::ETAG, etag.as_str()),
            (header::CACHE_CONTROL, "public, max-age=60"),
        ],
        Json(serde_json::json!({"text": text, "kind": kind})),
    )
        .into_response())
}

/// Đồng bộ theme sáng/tối lên server
#[derive(Deserialize)]
pub struct ThemeForm {
    pub theme: String,
}

pub async fn set_theme(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Form(form): Form<ThemeForm>,
) -> AppResult<Response> {
    let theme = if form.theme == "light" {
        "light"
    } else {
        "dark"
    };
    // 1 query UPSERT chỉ cột theme — trước đây phải SELECT preferences
    // rồi UPDATE lại toàn bộ (2 round-trip + có thể ghi đè preference
    // khác nếu user vừa lưu ở tab thứ hai).
    UserRepo::update_theme_only(&state.db, user.id, theme).await?;
    Ok(Json(serde_json::json!({"ok": true, "theme": theme})).into_response())
}

/// `OpenSearch` Suggestions (application/x-suggestions+json) — format mảng
/// theo spec: \[query, \[titles\], \[descriptions\], \[urls\]\] để trình duyệt gợi
/// ý ngay trong ô tìm kiếm của thanh địa chỉ. Tái dùng query `suggest_titles`.
pub async fn opensearch_suggestions(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SuggestQuery>,
) -> AppResult<Response> {
    let query = q.q.trim();
    let query: String = query.chars().take(100).collect();
    let empty = query.chars().count() < 2;
    let suggestions = if empty {
        Vec::new()
    } else {
        GameRepo::suggest_titles(&state.db, &query, 8).await?
    };
    let titles: Vec<String> = suggestions.iter().map(|(t, _)| t.clone()).collect();
    let descs: Vec<String> = suggestions
        .iter()
        .map(|(t, _)| format!("Louis Space — {t}"))
        .collect();
    let urls: Vec<String> = suggestions
        .iter()
        .map(|(_, s)| format!("{}/games/{}", state.config.base_url, s))
        .collect();
    Ok((
        [(
            header::CONTENT_TYPE,
            "application/x-suggestions+json; charset=utf-8",
        )],
        Json(serde_json::json!([query, titles, descs, urls])),
    )
        .into_response())
}

/// Gợi ý tìm kiếm tin tức (autocomplete) — trả tối đa 8 title + slug.
/// Dùng cho dropdown khi user gõ ở ô search news.
pub async fn news_suggest(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SuggestQuery>,
) -> AppResult<Response> {
    let query = q.q.trim();
    let suggestions = if query.chars().count() < 2 {
        Vec::new()
    } else {
        NewsRepo::suggest_titles(&state.db, query, 8)
            .await
            .unwrap_or_default()
    };
    let titles: Vec<String> = suggestions.iter().map(|(t, _)| t.clone()).collect();
    let descs: Vec<String> = suggestions
        .iter()
        .map(|(t, _)| format!("Louis Space — {t}"))
        .collect();
    let urls: Vec<String> = suggestions
        .iter()
        .map(|(_, s)| format!("{}/news/{}", state.config.base_url, s))
        .collect();
    Ok((
        [(
            header::CONTENT_TYPE,
            "application/x-suggestions+json; charset=utf-8",
        )],
        Json(serde_json::json!([query, titles, descs, urls])),
    )
        .into_response())
}

/// Kiểm tra trùng tiêu đề game khi tạo mới
#[derive(Deserialize)]
pub struct DuplicateQuery {
    pub title: String,
}

/// Kiểm tra trùng tiêu đề tin tức khi tạo mới — cảnh báo user nếu
/// tin cùng tên đã có (giống `check_duplicate` của game).
pub async fn news_check_duplicate(
    State(state): State<Arc<AppState>>,
    Query(q): Query<DuplicateQuery>,
) -> AppResult<Response> {
    let title = q.title.trim();
    if title.chars().count() < 3 {
        return Ok(Json(serde_json::json!({"exists": false, "count": 0})).into_response());
    }
    // Đếm tin published + pending có cùng title (case-insensitive)
    let pattern = format!(
        "%{}%",
        crate::utils::escape_like(&title.chars().take(200).collect::<String>())
    );
    let count: i64 = sqlx::query_scalar(
        r"SELECT COUNT(*) FROM news
           WHERE status IN ('published', 'pending')
             AND title ILIKE $1 ESCAPE '\'",
    )
    .bind(&pattern)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);
    Ok(Json(serde_json::json!({
        "exists": count > 0,
        "count": count,
    }))
    .into_response())
}

/// Gợi ý tìm kiếm (autocomplete) — trả tối đa 8 title + slug khớp
/// tiền tố/chứa từ khóa. Query nhẹ (chỉ 2 cột) cho dropdown realtime.
#[derive(Deserialize)]
pub struct SuggestQuery {
    pub q: String,
}

pub async fn games_suggest(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SuggestQuery>,
) -> AppResult<Response> {
    let query = q.q.trim();
    // Tối thiểu 2 ký tự (chars) — 1 ký tự tạo pattern quá rộng, quét chậm
    // mà gợi ý ít ý nghĩa.
    let query: String = query.chars().take(100).collect();
    if query.chars().count() < 2 {
        return Ok((
            [(header::CACHE_CONTROL, "public, max-age=60")],
            Json(serde_json::json!({"data": []})),
        )
            .into_response());
    }
    let suggestions = GameRepo::suggest_titles(&state.db, &query, 8).await?;
    let data: Vec<serde_json::Value> = suggestions
        .iter()
        .map(|(title, slug)| {
            serde_json::json!({"title": title, "slug": slug, "url": format!("/games/{}", slug)})
        })
        .collect();
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=60")],
        Json(serde_json::json!({"data": data})),
    )
        .into_response())
}

pub async fn check_duplicate(
    State(state): State<Arc<AppState>>,
    Query(q): Query<DuplicateQuery>,
) -> AppResult<Response> {
    let title = q.title.trim();
    // Đếm theo KÝ TỰ (không phải byte): "Độ" là 2 chars nhưng 5 bytes
    // UTF-8 — dùng len() sẽ cho qua chuỗi 2 chữ tiếng Việt rồi bị chặn
    // ngược ở các bước sau (clamp 200 chars). JS phía client cũng đếm
    // theo UTF-16 length nên 3 là ngưỡng hợp lý cho cả hai phía.
    if title.chars().count() < 3 {
        return Ok((
            [(header::CACHE_CONTROL, "public, max-age=60")],
            Json(serde_json::json!({"similar": 0})),
        )
            .into_response());
    }
    // Clamp 200 ký tự như giới hạn title khi tạo game — chống gửi pattern
    // dài hàng chục KB làm ILIKE quét chậm (DoS nhẹ nhưng thật).
    let title: String = title.chars().take(200).collect();
    let similar = GameRepo::count_similar_title(&state.db, &title)
        .await
        .unwrap_or(0);
    // Cache 60s: input debounce 500ms nhưng user chỉnh đi chỉnh lại cùng
    // tiêu đề (thêm dấu, sửa chính tả) vẫn spam DB từng keystroke. Kết
    // quả trùng-lặp không cần real-time (chỉ là cảnh báo mềm).
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=60")],
        Json(serde_json::json!({"similar": similar})),
    )
        .into_response())
}

// ===================== SEO: RSS, Sitemap, robots =====================

pub async fn rss(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> AppResult<Response> {
    let games = GameRepo::latest_for_rss(&state.db, 20).await?;
    let base = &state.config.base_url;
    let mut items = String::new();
    for g in &games {
        let esc_title = crate::utils::xml_escape(&g.title);
        let esc_excerpt = crate::utils::xml_escape(&g.excerpt.clone().unwrap_or_default());
        // <category>/<author> theo RSS 2.0 spec — reader hiển thị nguồn
        // lọc theo thể loại/tác giả thay vì chỉ list phẳng.
        let category_tag = g
            .category_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|c| {
                format!(
                    "\n      <category>{}</category>",
                    crate::utils::xml_escape(c)
                )
            })
            .unwrap_or_default();
        let author_tag = format!(
            "\n      <author>{}</author>",
            crate::utils::xml_escape(&g.author_name)
        );
        // pubDate chỉ render khi có published_at — element rỗng
        // <pubDate></pubDate> là error theo W3C Feed Validator (một số
        // reader drop cả item). Game list RSS luôn published nên hầu
        // như luôn có giá trị; nhánh này chỉ phòng data legacy NULL.
        let pub_date_tag = g
            .published_at
            .map(|d| {
                format!(
                    "\n      <pubDate>{}</pubDate>",
                    d.format("%a, %d %b %Y %H:%M:%S +0000")
                )
            })
            .unwrap_or_default();
        items.push_str(&format!(
            r#"    <item>
      <title>{}</title>
      <link>{}/games/{}</link>
      <guid isPermaLink="true">{}/games/{}</guid>{}
      <description>{}</description>{}{}
    </item>
"#,
            esc_title,
            base,
            g.slug,
            base,
            g.slug,
            category_tag,
            esc_excerpt,
            author_tag,
            pub_date_tag
        ));
    }
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
  <channel>
    <title>Louis Space - Game mới nhất</title>
    <link>{}</link>
    <description>Nền tảng chia sẻ game độc lập cho cộng đồng Việt Nam</description>
    <language>vi</language>
    <generator>Louis Space {} (Rust/Axum)</generator>
    <atom:link href="{}/rss.xml" rel="self" type="application/rss+xml"/>
    <ttl>60</ttl>
    <lastBuildDate>{}</lastBuildDate>
{}
  </channel>
</rss>"#,
        base,
        env!("CARGO_PKG_VERSION"),
        base,
        chrono::Utc::now().format("%a, %d %b %Y %H:%M:%S +0000"),
        items
    );
    // ETag đơn giản dựa trên hash nội dung — client gửi If-None-Match khớp
    // → server trả 304 Not Modified, không cần chuyển payload XML.
    let etag = format!("\"{}\"", short_hash(&xml));
    if etag_matches(&headers, &etag) {
        return Ok((
            StatusCode::NOT_MODIFIED,
            [
                (header::ETAG, etag.as_str()),
                (header::CACHE_CONTROL, "public, max-age=600"),
            ],
        )
            .into_response());
    }
    Ok((
        [
            (header::CONTENT_TYPE, "application/rss+xml; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=600"),
            (header::ETAG, etag.as_str()),
        ],
        xml,
    )
        .into_response())
}

/// GET /news.rss — RSS feed riêng cho tin tức.
/// Tách khỏi /rss.xml (game) để reader subscribe độc lập.
pub async fn news_rss(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> AppResult<Response> {
    let news = NewsRepo::list_published(&state.db, 1, 20)
        .await
        .unwrap_or_default();
    let base = &state.config.base_url;
    let mut items = String::new();
    for n in &news {
        let esc_title = crate::utils::xml_escape(&n.title);
        let esc_excerpt = crate::utils::xml_escape(&n.excerpt);
        let pub_date_tag = n
            .published_at
            .map(|d| {
                format!(
                    "\n      <pubDate>{}</pubDate>",
                    d.format("%a, %d %b %Y %H:%M:%S +0000")
                )
            })
            .unwrap_or_default();
        let category_tag = if n.category.is_empty() {
            String::new()
        } else {
            format!(
                "\n      <category>{}</category>",
                crate::utils::xml_escape(&n.category)
            )
        };
        let author_tag = format!(
            "\n      <author>{}</author>",
            crate::utils::xml_escape(&n.author_name)
        );
        items.push_str(&format!(
            r#"    <item>
      <title>{}</title>
      <link>{}/news/{}</link>
      <guid isPermaLink="true">{}/news/{}</guid>{}
      <description>{}</description>{}{}
    </item>
"#,
            esc_title,
            base,
            n.slug,
            base,
            n.slug,
            category_tag,
            esc_excerpt,
            author_tag,
            pub_date_tag
        ));
    }
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
  <channel>
    <title>Louis Space - Tin tức</title>
    <link>{}/news</link>
    <description>Tin tức game, công nghệ & cộng đồng Việt Nam</description>
    <language>vi</language>
    <generator>Louis Space {} (Rust/Axum)</generator>
    <atom:link href="{}/news.rss" rel="self" type="application/rss+xml"/>
    <ttl>60</ttl>
    <lastBuildDate>{}</lastBuildDate>
{}
  </channel>
</rss>"#,
        base,
        env!("CARGO_PKG_VERSION"),
        base,
        chrono::Utc::now().format("%a, %d %b %Y %H:%M:%S +0000"),
        items
    );
    let etag = format!("\"{}\"", short_hash(&xml));
    if etag_matches(&headers, &etag) {
        return Ok((
            StatusCode::NOT_MODIFIED,
            [
                (header::ETAG, etag.as_str()),
                (header::CACHE_CONTROL, "public, max-age=600"),
            ],
        )
            .into_response());
    }
    Ok((
        [
            (header::CONTENT_TYPE, "application/rss+xml; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=600"),
            (header::ETAG, etag.as_str()),
        ],
        xml,
    )
        .into_response())
}

pub async fn sitemap(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> AppResult<Response> {
    let base = &state.config.base_url;
    // 4 query độc lập (categories/tags/users/games) chạy SONG SONG —
    // trước đây tuần tự, mỗi lần bot/axios fetch sitemap chịu tổng 4
    // round-trip DB liền nhau.
    let (cats_res, tags_res, users_res, games_res) = tokio::join!(
        CategoryRepo::list_with_counts(&state.db),
        TagRepo::popular(&state.db, 50),
        UserRepo::sitemap_usernames(&state.db),
        GameRepo::sitemap_entries(&state.db),
    );
    let mut urls = String::new();
    urls.push_str(&format!(
        r"  <url><loc>{base}/</loc><changefreq>hourly</changefreq><priority>1.0</priority></url>
"
    ));
    for page in [
        "/games",
        "/games/latest",
        "/games/trending",
        "/games/top-rated",
        "/games/downloads",
        "/games/featured",
        "/categories",
        "/repos",
        "/search",
        "/terms",
        "/privacy",
        "/news",
    ] {
        urls.push_str(&format!(
            r"  <url><loc>{base}{page}</loc><changefreq>daily</changefreq><priority>0.8</priority></url>
"
        ));
    }
    // Escape XML mọi giá trị chèn vào <loc> — ký tự & < > trong
    // slug/username (dù hiếm) sẽ làm sitemap XML invalid, Google
    // Search Console báo lỗi parse và bỏ toàn bộ sitemap.
    if let Ok(cats) = cats_res {
        for c in cats {
            urls.push_str(&format!(
                r"  <url><loc>{}/c/{}</loc><changefreq>daily</changefreq><priority>0.6</priority></url>
",
                base,
                crate::utils::xml_escape(&c.slug)
            ));
        }
    }
    if let Ok(tags) = tags_res {
        for t in tags {
            urls.push_str(&format!(
                r"  <url><loc>{}/t/{}</loc><changefreq>weekly</changefreq><priority>0.5</priority></url>
",
                base,
                crate::utils::xml_escape(&t.slug)
            ));
        }
    }
    // Hồ sơ người dùng công khai (không ban, không AI Agent) — Google lập
    // index trang /u/{username} để tìm game theo tác giả.
    if let Ok(users) = users_res {
        for username in users {
            urls.push_str(&format!(
                r"  <url><loc>{}/u/{}</loc><changefreq>weekly</changefreq><priority>0.4</priority></url>
",
                base,
                crate::utils::xml_escape(&username)
            ));
        }
    }
    if let Ok(games) = games_res {
        for (slug, updated) in games {
            urls.push_str(&format!(
                r"  <url><loc>{}/games/{}</loc><lastmod>{}</lastmod><changefreq>weekly</changefreq><priority>0.7</priority></url>
",
                base,
                crate::utils::xml_escape(&slug),
                updated.format("%Y-%m-%d")
            ));
        }
    }
    // News URLs — 50 tin published mới nhất (tránh sitemap quá lớn)
    if let Ok(news) = NewsRepo::list_published(&state.db, 1, 50).await {
        for n in news {
            urls.push_str(&format!(
                r"  <url><loc>{}/news/{}</loc><lastmod>{}</lastmod><changefreq>weekly</changefreq><priority>0.7</priority></url>
",
                base,
                crate::utils::xml_escape(&n.slug),
                n.published_at.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default()
            ));
        }
    }
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
{urls}</urlset>"#
    );
    let etag = format!("\"{}\"", short_hash(&xml));
    if etag_matches(&headers, &etag) {
        return Ok((
            StatusCode::NOT_MODIFIED,
            [
                (header::ETAG, etag.as_str()),
                (header::CACHE_CONTROL, "public, max-age=600"),
            ],
        )
            .into_response());
    }
    Ok((
        [
            (header::CONTENT_TYPE, "application/xml; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=600"),
            (header::ETAG, etag.as_str()),
        ],
        xml,
    )
        .into_response())
}

/// So sánh `ETag` server với If-None-Match header của client.
/// Hỗ trợ wildcard `*` (luôn khớp) và danh sách `ETag` cách nhau bởi `,`.
fn etag_matches(headers: &axum::http::HeaderMap, etag: &str) -> bool {
    let Some(inm) = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    if inm.trim() == "*" {
        return true;
    }
    inm.split(',').any(|e| e.trim() == etag)
}

/// Hash ngắn (16 hex chars) cho `ETag`. Dùng xxHash sẽ nhanh hơn nhưng
/// thêm dependency; SHA-256 của `sha2` đã có sẵn, cắt 16 ký tự đầu là đủ
/// chống collision cho mục đích cache validation (không phải mật khẩu).
fn short_hash(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    hex::encode(&h.finalize()[..8])
}

pub async fn robots(State(state): State<Arc<AppState>>) -> Response {
    let base = &state.config.base_url;
    let txt = format!(
        "User-agent: *\nAllow: /\nDisallow: /admin\nDisallow: /profile\nDisallow: /notifications\nDisallow: /bookmarks\nDisallow: /my-games\nDisallow: /my-news\nDisallow: /news/new\nDisallow: /news/*/edit\nDisallow: /auth\nDisallow: /api/\nDisallow: /ai\n\nSitemap: {base}/sitemap.xml\n"
    );
    (
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            // robots.txt đổi rất hiếm — cache 1h giảm request vô ích
            (header::CACHE_CONTROL, "public, max-age=3600"),
        ],
        txt,
    )
        .into_response()
}

/// `OpenSearch` description XML — cho phép trình duyệt thêm Louis Space vào
/// ô tìm kiếm của thanh địa chỉ.
pub async fn opensearch(State(state): State<Arc<AppState>>) -> Response {
    let base = &state.config.base_url;
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<OpenSearchDescription xmlns="http://a9.com/-/spec/opensearch/1.1/"
                       xmlns:moz="http://www.mozilla.org/2006/browser/search/">
  <ShortName>Louis Space</ShortName>
  <Description>Tìm kiếm game độc lập trên Louis Space</Description>
  <InputEncoding>UTF-8</InputEncoding>
  <OutputEncoding>UTF-8</OutputEncoding>
  <Image width="32" height="32" type="image/svg+xml">{base}/static/img/favicon.svg</Image>
  <Url type="text/html" method="get" template="{base}/search?q={{searchTerms}}"/>
  <Url type="application/x-suggestions+json" rel="suggestions" template="{base}/opensearch-suggest?q={{searchTerms}}"/>
  <Query role="example" searchTerms="puzzle"/>
  <moz:SearchForm>{base}/search</moz:SearchForm>
</OpenSearchDescription>"#
    );
    (
        [(
            header::CONTENT_TYPE,
            "application/opensearchdescription+xml; charset=utf-8",
        )],
        xml,
    )
        .into_response()
}

/// security.txt — RFC 9116, đặt tại /.well-known/security.txt
/// Cung cấp thông tin liên hệ để nhà nghiên cứu bảo mật báo lỗ hổng.
pub async fn security_txt(State(state): State<Arc<AppState>>) -> Response {
    let base = &state.config.base_url;
    let admin_email = &state.config.admin_email;
    let txt = format!(
        "Contact: mailto:{admin_email}\nExpires: {expires}\nPreferred-Languages: vi, en\nCanonical: {base}/.well-known/security.txt\n",
        admin_email = admin_email,
        expires = chrono::Utc::now()
            .checked_add_months(chrono::Months::new(6))
            .map(|d| d.format("%Y-%m-%dT00:00:00.000Z").to_string())
            .unwrap_or_default(),
        base = base,
    );
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], txt).into_response()
}

/// PWA manifest — đặt tại /manifest.json (quy ước W3C). Trả về JSON
/// từ static/manifest.json, có thể cache lâu vì nội dung hiếm khi đổi.
pub async fn manifest() -> Response {
    let json = include_str!("../../static/manifest.json");
    (
        [
            (
                header::CONTENT_TYPE,
                "application/manifest+json; charset=utf-8",
            ),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        json,
    )
        .into_response()
}

// ===================== Health nâng cao =====================

/// Thời điểm process khởi động (dùng cho uptime trong health check).
/// Lazy-init một lần duy nhất khi được gọi đầu tiên.
static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

pub async fn health_detail(State(state): State<Arc<AppState>>) -> Response {
    let started = START_TIME.get_or_init(std::time::Instant::now);
    let uptime_secs = started.elapsed().as_secs();
    let pool = &state.db;
    let db_ok = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(pool)
        .await
        .is_ok();
    // Pool stats giúp phát hiện connection leak / cạn pool trên prod
    // mà không cần kết nối trực tiếp vào PostgreSQL để chạy pg_stat_activity.
    let pool_size = pool.size();
    let pool_idle = pool.num_idle() as u32;
    let in_use = pool_size.saturating_sub(pool_idle);
    let (status, body) = if db_ok {
        (
            axum::http::StatusCode::OK,
            serde_json::json!({
                "status": "ok",
                "version": env!("CARGO_PKG_VERSION"),
                "database": "up",
                "pool": {
                    "size": pool_size,
                    "idle": pool_idle,
                    "in_use": in_use,
                },
                "uptime_secs": uptime_secs,
                "time": chrono::Utc::now().to_rfc3339(),
            }),
        )
    } else {
        (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            serde_json::json!({
                "status": "degraded",
                "version": env!("CARGO_PKG_VERSION"),
                "database": "down",
                "pool": {
                    "size": pool_size,
                    "idle": pool_idle,
                    "in_use": in_use,
                },
                "uptime_secs": uptime_secs,
                "time": chrono::Utc::now().to_rfc3339(),
            }),
        )
    };
    // Health endpoint không cache — monitor (Coolify/Kubernetes) cần trạng
    // thái thời gian thực, không phải snapshot cũ.
    (
        status,
        [(header::CACHE_CONTROL, "no-store, max-age=0")],
        Json(body),
    )
        .into_response()
}

// ===================== Catalog endpoints: tags & categories =====================
// Cung cấp danh sách tag và thể loại qua JSON API để client-side search,
// autocomplete và cross-linking không cần phải tải trang HTML đầy đủ.

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn tags_list(State(state): State<Arc<AppState>>) -> AppResult<Response> {
    let tags = TagRepo::popular(&state.db, 100).await.unwrap_or_default();
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=300")],
        Json(serde_json::json!({"data": tags})),
    )
        .into_response())
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn categories_list(State(state): State<Arc<AppState>>) -> AppResult<Response> {
    let cats = CategoryRepo::list_with_counts(&state.db).await?;
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=300")],
        Json(serde_json::json!({"data": cats})),
    )
        .into_response())
}

/// GET /api/v1/news — danh sách tin tức đã published (public).
/// Hỗ trợ ?page=N và ?category=game|tech|industry|esports|community|review|update|other.
pub async fn news_list(
    State(state): State<Arc<AppState>>,
    Query(params): Query<NewsListApiParams>,
) -> AppResult<Response> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = 12i64;
    let category = params.category.as_deref().unwrap_or("");
    let items = if category.is_empty() {
        NewsRepo::list_published(&state.db, page, per_page).await?
    } else {
        NewsRepo::list_by_category(&state.db, category, page, per_page).await?
    };
    let total = if category.is_empty() {
        NewsRepo::count_published(&state.db).await.unwrap_or(0)
    } else {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM news WHERE status = 'published' AND category = $1",
        )
        .bind(category)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0)
    };
    let total_pages = ((total + per_page - 1) / per_page).max(1);
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=120")],
        Json(serde_json::json!({
            "data": items,
            "page": page,
            "per_page": per_page,
            "total": total,
            "total_pages": total_pages,
        })),
    )
        .into_response())
}

#[derive(Deserialize)]
pub struct NewsListApiParams {
    pub page: Option<i64>,
    pub category: Option<String>,
}

/// GET /api/v1/news/{slug} — chi tiết 1 bài tin tức (public).
pub async fn news_detail(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> AppResult<Response> {
    let news = NewsRepo::find_by_slug_public(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Tin tức không tồn tại".into()))?;
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=60")],
        Json(serde_json::json!(news)),
    )
        .into_response())
}

/// Hồ sơ user công khai — cho phép client bên ngoài hiển thị thông tin
/// tác giả game mà không cần cào HTML. Chỉ trả field công khai:
/// username, `display_name`, `avatar_url`, bio, role, stats (số game,
/// follower, following). Không trả email hay session info nhạy cảm.
pub async fn user_profile(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
) -> AppResult<Response> {
    let user = UserRepo::find_by_username(&state.db, &username)
        .await?
        .ok_or_else(|| AppError::NotFound("Người dùng không tồn tại".into()))?;
    if user.is_banned {
        return Err(AppError::NotFound("Người dùng không tồn tại".into()));
    }
    let stats = UserRepo::stats(&state.db, user.id).await?;
    let body = serde_json::json!({
        "username": user.username,
        "display_name": user.display_name,
        "avatar_url": user.avatar_url,
        "bio": user.bio,
        "role": format!("{:?}", user.role).to_lowercase(),
        "created_at": user.created_at.to_rfc3339(),
        "last_seen_at": user.last_seen_at.map(|d| d.to_rfc3339()),
        "stats": {
            "games_count": stats.games_count,
            "followers_count": stats.followers_count,
            "following_count": stats.following_count,
        },
    });
    Ok(([(header::CACHE_CONTROL, "public, max-age=120")], Json(body)).into_response())
}

/// Map một `GameCard` thành JSON object dùng chung cho các endpoint liệt
/// kê game (`by_category`, `by_tag`, related). Trước đây 3 handler lặp lại
/// cùng khối ~20 dòng này — thêm field mới phải sửa 3 chỗ dễ lệch nhau.
fn game_card_to_json(g: &crate::models::game::GameCard) -> serde_json::Value {
    serde_json::json!({
        "slug": g.slug,
        "title": g.title,
        "excerpt": g.excerpt,
        "cover_image": g.cover_image,
        "category": g.category_name,
        "category_slug": g.category_slug,
        "author": g.author_name,
        "platforms": g.platforms,
        "view_count": g.view_count,
        "download_count": g.download_count,
        "like_count": g.like_count,
        "rating_avg": g.rating_avg_f64(),
        "rating_count": g.rating_count,
        "published_at": g.published_at.map(|d| d.to_rfc3339()),
    })
}

/// Game liên quan — cùng category (hoặc top downloads nếu không có
/// category). Lợi cho sidebar "Related games" ở client bên ngoài.
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn game_related(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> AppResult<Response> {
    let g = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    let related = GameRepo::related(&state.db, g.id, g.category_id, 10).await?;
    let data: Vec<serde_json::Value> = related.iter().map(game_card_to_json).collect();
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=300")],
        Json(serde_json::json!({"data": data})),
    )
        .into_response())
}

/// Liệt kê game theo thể loại — JSON API cho client bên ngoài lọc game
/// theo category mà không cần cào trang HTML.
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn games_by_category(
    State(state): State<Arc<AppState>>,
    Path(cat_slug): Path<String>,
    Query(q): Query<ApiListQuery>,
) -> AppResult<Response> {
    let cat = CategoryRepo::find_by_slug(&state.db, &cat_slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Thể loại không tồn tại".into()))?;
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 24;
    let offset = (page - 1) * per_page;
    // games + count độc lập — join! song song
    let (games_res, total_res) = tokio::join!(
        GameRepo::by_category(
            &state.db,
            &cat_slug,
            per_page,
            offset,
            q.sort.as_deref().unwrap_or("latest"),
        ),
        GameRepo::count_by_category(&state.db, &cat_slug),
    );
    let games = games_res?;
    let total = total_res.unwrap_or(0);
    let data: Vec<serde_json::Value> = games.iter().map(game_card_to_json).collect();
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=300")],
        Json(serde_json::json!({
            "category": {"slug": cat.slug, "name": cat.name},
            "data": data,
            "total": total,
            "page": page,
            "per_page": per_page,
        })),
    )
        .into_response())
}

/// Liệt kê game theo tag — JSON API cho client bên ngoài.
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn games_by_tag(
    State(state): State<Arc<AppState>>,
    Path(tag_slug): Path<String>,
    Query(q): Query<ApiListQuery>,
) -> AppResult<Response> {
    let tag = TagRepo::find_by_slug(&state.db, &tag_slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Tag không tồn tại".into()))?;
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 24;
    let offset = (page - 1) * per_page;
    // games + count độc lập — join! song song
    let (games_res, total_res) = tokio::join!(
        GameRepo::by_tag(
            &state.db,
            &tag_slug,
            per_page,
            offset,
            q.sort.as_deref().unwrap_or("latest"),
        ),
        GameRepo::count_by_tag(&state.db, &tag_slug),
    );
    let games = games_res?;
    let total = total_res.unwrap_or(0);
    let data: Vec<serde_json::Value> = games.iter().map(game_card_to_json).collect();
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=300")],
        Json(serde_json::json!({
            "tag": {"slug": tag.slug, "name": tag.name},
            "data": data,
            "total": total,
            "page": page,
            "per_page": per_page,
        })),
    )
        .into_response())
}

/// Discovery endpoint: liệt kê tất cả endpoint API có sẵn, kèm method
/// và mô tả ngắn. Tiện cho client bên ngoài tự khám phá API mà không
/// phải đọc doc. Cache 1 giờ (rarely changes).
pub async fn root(State(state): State<Arc<AppState>>) -> Response {
    let base = state.config.base_url.clone();
    let endpoints = serde_json::json!({
        "name": "Louis Space API",
        "version": env!("CARGO_PKG_VERSION"),
        "base_url": format!("{}/api/v1", base),
        "documentation": format!("{}/api/v1", base),
        "endpoints": [
            {"method": "GET", "path": "/api/v1", "description": "Discovery — danh sách endpoint"},
            {"method": "GET", "path": "/api/v1/games", "description": "Danh sách game (phân trang: page, per_page 1-50; sort; search q)"},
            {"method": "GET", "path": "/api/v1/games/{slug}", "description": "Chi tiết game"},
            {"method": "GET", "path": "/api/v1/games/{slug}/related", "description": "Game liên quan"},
            {"method": "GET", "path": "/api/v1/games/{slug}/comments", "description": "Bình luận của game (phân trang)"},
            {"method": "GET", "path": "/api/v1/categories", "description": "Danh sách thể loại"},
            {"method": "GET", "path": "/api/v1/categories/{slug}/games", "description": "Game theo thể loại"},
            {"method": "GET", "path": "/api/v1/tags", "description": "Top tag phổ biến"},
            {"method": "GET", "path": "/api/v1/tags/{slug}/games", "description": "Game theo tag"},
            {"method": "GET", "path": "/api/v1/users/{username}", "description": "Hồ sơ user công khai"},
            {"method": "GET", "path": "/api/v1/repos", "description": "Repo GitHub đã duyệt"},
            {"method": "GET", "path": "/api/v1/news", "description": "Tin tức đã xuất bản (page, category)"},
            {"method": "GET", "path": "/api/v1/news/{slug}", "description": "Chi tiết 1 bài tin tức"},
            {"method": "GET", "path": "/api/v1/stats", "description": "Thống kê tổng quan"},
            {"method": "GET", "path": "/api/v1/health", "description": "Health check (no-store)"},
        ],
    });
    (
        [(header::CACHE_CONTROL, "public, max-age=3600")],
        Json(endpoints),
    )
        .into_response()
}

// ===================== Maintenance page dùng nội bộ =====================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn test_etag_matches_exact() {
        let mut h = HeaderMap::new();
        h.insert(header::IF_NONE_MATCH, "\"abc123\"".parse().unwrap());
        assert!(etag_matches(&h, "\"abc123\""));
        assert!(!etag_matches(&h, "\"different\""));
    }

    #[test]
    fn test_etag_matches_wildcard() {
        let mut h = HeaderMap::new();
        h.insert(header::IF_NONE_MATCH, "*".parse().unwrap());
        assert!(etag_matches(&h, "\"anything\""));
    }

    #[test]
    fn test_etag_matches_list() {
        let mut h = HeaderMap::new();
        h.insert(
            header::IF_NONE_MATCH,
            "\"etag1\", \"etag2\", \"etag3\"".parse().unwrap(),
        );
        assert!(etag_matches(&h, "\"etag2\""));
        assert!(!etag_matches(&h, "\"etag4\""));
    }

    #[test]
    fn test_etag_matches_missing_header() {
        let h = HeaderMap::new();
        assert!(!etag_matches(&h, "\"abc\""));
    }

    #[test]
    fn test_short_hash_deterministic() {
        // Hash phải deterministic cho cùng input
        let h1 = short_hash("hello world");
        let h2 = short_hash("hello world");
        assert_eq!(h1, h2);
        // Input khác → hash khác
        let h3 = short_hash("hello world!");
        assert_ne!(h1, h3);
        // Đúng 16 hex chars
        assert_eq!(h1.len(), 16);
        assert!(h1.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
