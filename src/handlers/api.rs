use crate::error::{AppError, AppResult};
use crate::middleware::AuthUser;
use crate::repositories::{CategoryRepo, GameRepo, RepoRepo, SettingsRepo, TagRepo, UserRepo};
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
}

pub async fn games_list(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ApiListQuery>,
) -> AppResult<Response> {
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 24;
    let offset = (page - 1) * per_page;
    let sort = q.sort.clone().unwrap_or_else(|| "latest".into());

    let cards = match q.q.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(query) => {
            GameRepo::search(&state.db, query.trim(), None, None, &sort, per_page, offset).await?
        }
        None => GameRepo::list_published(&state.db, per_page, offset, &sort).await?,
    };
    let total = GameRepo::count_published(&state.db).await.unwrap_or(0);

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
    let links = GameRepo::get_links(&state.db, g.id).await?;
    let tags = GameRepo::get_tags(&state.db, g.id).await?;
    let author = UserRepo::find_by_id(&state.db, g.user_id).await?;
    // Lấy thêm category & screenshots để API public đầy đủ hơn (trước đây
    // API thiếu các trường này, khiến client phải gọi thêm nhiều endpoint).
    let category = if let Some(cat_id) = g.category_id {
        CategoryRepo::find_by_id(&state.db, cat_id).await?
    } else {
        None
    };
    let screenshots = GameRepo::get_screenshots(&state.db, g.id).await?;
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

pub async fn repos_list(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ApiListQuery>,
) -> AppResult<Response> {
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 30;
    let offset = (page - 1) * per_page;
    let sort = q.sort.unwrap_or_else(|| "stars".into());
    let repos = RepoRepo::list_approved(&state.db, per_page, offset, &sort).await?;
    let total = RepoRepo::count_approved(&state.db).await.unwrap_or(0);
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

pub async fn stats_overview(State(state): State<Arc<AppState>>) -> AppResult<Response> {
    let total_games = GameRepo::count_published(&state.db).await.unwrap_or(0);
    let total_users = UserRepo::count_all(&state.db).await.unwrap_or(0);
    let total_repos = RepoRepo::count_approved(&state.db).await.unwrap_or(0);
    let total_downloads: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
    let total_comments: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM comments")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
    Ok(Json(serde_json::json!({
        "total_games": total_games,
        "total_users": total_users,
        "total_repos": total_repos,
        "total_downloads": total_downloads,
        "total_comments": total_comments,
    }))
    .into_response())
}

// ===================== Nội bộ dùng chung =====================

/// Banner thông báo toàn site (layout fetch qua htmx)
pub async fn announcement(State(state): State<Arc<AppState>>) -> AppResult<Response> {
    let text = SettingsRepo::get(&state.db, "announcement")
        .await?
        .unwrap_or_default();
    if text.is_empty() {
        return Ok(Json(serde_json::json!({"text": "", "kind": ""})).into_response());
    }
    let kind = SettingsRepo::get(&state.db, "announcement_type")
        .await?
        .unwrap_or_else(|| "info".into());
    Ok(Json(serde_json::json!({"text": text, "kind": kind})).into_response())
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
    let pref = UserRepo::get_preferences(&state.db, user.id).await?;
    UserRepo::update_preferences(
        &state.db,
        user.id,
        theme,
        pref.email_notifications,
        pref.show_online,
        &pref.language,
    )
    .await?;
    Ok(Json(serde_json::json!({"ok": true, "theme": theme})).into_response())
}

/// Kiểm tra trùng tiêu đề game khi tạo mới
#[derive(Deserialize)]
pub struct DuplicateQuery {
    pub title: String,
}

pub async fn check_duplicate(
    State(state): State<Arc<AppState>>,
    Query(q): Query<DuplicateQuery>,
) -> AppResult<Response> {
    if q.title.trim().len() < 3 {
        return Ok(Json(serde_json::json!({"similar": 0})).into_response());
    }
    let similar = GameRepo::count_similar_title(&state.db, q.title.trim())
        .await
        .unwrap_or(0);
    Ok(Json(serde_json::json!({"similar": similar})).into_response())
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
        let esc_title = crate::utils::html_escape(&g.title);
        let esc_excerpt = crate::utils::html_escape(&g.excerpt.clone().unwrap_or_default());
        items.push_str(&format!(
            r#"    <item>
      <title>{}</title>
      <link>{}/games/{}</link>
      <guid isPermaLink="true">{}/games/{}</guid>
      <description>{}</description>
      <pubDate>{}</pubDate>
    </item>
"#,
            esc_title,
            base,
            g.slug,
            base,
            g.slug,
            esc_excerpt,
            g.published_at
                .map(|d| d.format("%a, %d %b %Y %H:%M:%S +0000").to_string())
                .unwrap_or_default()
        ));
    }
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Kho Game - Game mới nhất</title>
    <link>{}</link>
    <description>Nền tảng chia sẻ game độc lập cho cộng đồng Việt Nam</description>
    <language>vi</language>
    <lastBuildDate>{}</lastBuildDate>
{}
  </channel>
</rss>"#,
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

pub async fn sitemap(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> AppResult<Response> {
    let base = &state.config.base_url;
    let mut urls = String::new();
    urls.push_str(&format!(
        r#"  <url><loc>{}/</loc><changefreq>hourly</changefreq><priority>1.0</priority></url>
"#,
        base
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
    ] {
        urls.push_str(&format!(
            r#"  <url><loc>{}{}</loc><changefreq>daily</changefreq><priority>0.8</priority></url>
"#,
            base, page
        ));
    }
    if let Ok(cats) = CategoryRepo::list_with_counts(&state.db).await {
        for c in cats {
            urls.push_str(&format!(
                r#"  <url><loc>{}/c/{}</loc><changefreq>daily</changefreq><priority>0.6</priority></url>
"#,
                base, c.slug
            ));
        }
    }
    if let Ok(tags) = TagRepo::popular(&state.db, 50).await {
        for t in tags {
            urls.push_str(&format!(
                r#"  <url><loc>{}/t/{}</loc><changefreq>weekly</changefreq><priority>0.5</priority></url>
"#,
                base, t.slug
            ));
        }
    }
    if let Ok(games) = GameRepo::sitemap_entries(&state.db).await {
        for (slug, updated) in games {
            urls.push_str(&format!(
                r#"  <url><loc>{}/games/{}</loc><lastmod>{}</lastmod><changefreq>weekly</changefreq><priority>0.7</priority></url>
"#,
                base,
                slug,
                updated.format("%Y-%m-%d")
            ));
        }
    }
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
{}</urlset>"#,
        urls
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

/// So sánh ETag server với If-None-Match header của client.
/// Hỗ trợ wildcard `*` (luôn khớp) và danh sách ETag cách nhau bởi `,`.
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

/// Hash ngắn (16 hex chars) cho ETag. Dùng xxHash sẽ nhanh hơn nhưng
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
        "User-agent: *\nAllow: /\nDisallow: /admin\nDisallow: /profile\nDisallow: /notifications\nDisallow: /api/\n\nSitemap: {}/sitemap.xml\n",
        base
    );
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], txt).into_response()
}

/// OpenSearch description XML — cho phép trình duyệt thêm Kho Game vào
/// ô tìm kiếm của thanh địa chỉ.
pub async fn opensearch(State(state): State<Arc<AppState>>) -> Response {
    let base = &state.config.base_url;
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<OpenSearchDescription xmlns="http://a9.com/-/spec/opensearch/1.1/"
                       xmlns:moz="http://www.mozilla.org/2006/browser/search/">
  <ShortName>Kho Game</ShortName>
  <Description>Tìm kiếm game độc lập trên Kho Game</Description>
  <InputEncoding>UTF-8</InputEncoding>
  <OutputEncoding>UTF-8</OutputEncoding>
  <Image width="32" height="32" type="image/svg+xml">{base}/static/img/favicon.svg</Image>
  <Url type="text/html" method="get" template="{base}/search?q={{searchTerms}}"/>
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

pub async fn health_detail(State(state): State<Arc<AppState>>) -> Response {
    let db_ok = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db)
        .await
        .is_ok();
    let (status, body) = if db_ok {
        (
            axum::http::StatusCode::OK,
            serde_json::json!({
                "status": "ok",
                "version": env!("CARGO_PKG_VERSION"),
                "database": "up",
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

pub async fn tags_list(State(state): State<Arc<AppState>>) -> AppResult<Response> {
    let tags = TagRepo::popular(&state.db, 100).await.unwrap_or_default();
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=300")],
        Json(serde_json::json!({"data": tags})),
    )
        .into_response())
}

pub async fn categories_list(State(state): State<Arc<AppState>>) -> AppResult<Response> {
    let cats = CategoryRepo::list_with_counts(&state.db).await?;
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=300")],
        Json(serde_json::json!({"data": cats})),
    )
        .into_response())
}

/// Hồ sơ user công khai — cho phép client bên ngoài hiển thị thông tin
/// tác giả game mà không cần cào HTML. Chỉ trả field công khai:
/// username, display_name, avatar_url, bio, role, stats (số game,
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

/// Game liên quan — cùng category (hoặc top downloads nếu không có
/// category). Lợi cho sidebar "Related games" ở client bên ngoài.
pub async fn game_related(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> AppResult<Response> {
    let g = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    let related = GameRepo::related(&state.db, g.id, g.category_id, 10).await?;
    let data: Vec<serde_json::Value> = related
        .iter()
        .map(|g| {
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
            })
        })
        .collect();
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=300")],
        Json(serde_json::json!({"data": data})),
    )
        .into_response())
}

/// Liệt kê game theo thể loại — JSON API cho client bên ngoài lọc game
/// theo category mà không cần cào trang HTML.
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
    let games = GameRepo::by_category(&state.db, &cat_slug, per_page, offset).await?;
    let total = GameRepo::count_by_category(&state.db, &cat_slug)
        .await
        .unwrap_or(0);
    let data: Vec<serde_json::Value> = games
        .iter()
        .map(|g| {
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
        })
        .collect();
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
    let games = GameRepo::by_tag(&state.db, &tag_slug, per_page, offset).await?;
    let total = GameRepo::count_by_tag(&state.db, &tag_slug)
        .await
        .unwrap_or(0);
    let data: Vec<serde_json::Value> = games
        .iter()
        .map(|g| {
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
        })
        .collect();
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
        "name": "Kho Game API",
        "version": env!("CARGO_PKG_VERSION"),
        "base_url": format!("{}/api/v1", base),
        "documentation": format!("{}/api/v1", base),
        "endpoints": [
            {"method": "GET", "path": "/api/v1", "description": "Discovery — danh sách endpoint"},
            {"method": "GET", "path": "/api/v1/games", "description": "Danh sách game (có phân trang, sort, search)"},
            {"method": "GET", "path": "/api/v1/games/{slug}", "description": "Chi tiết game"},
            {"method": "GET", "path": "/api/v1/games/{slug}/related", "description": "Game liên quan"},
            {"method": "GET", "path": "/api/v1/categories", "description": "Danh sách thể loại"},
            {"method": "GET", "path": "/api/v1/categories/{slug}/games", "description": "Game theo thể loại"},
            {"method": "GET", "path": "/api/v1/tags", "description": "Top tag phổ biến"},
            {"method": "GET", "path": "/api/v1/tags/{slug}/games", "description": "Game theo tag"},
            {"method": "GET", "path": "/api/v1/users/{username}", "description": "Hồ sơ user công khai"},
            {"method": "GET", "path": "/api/v1/repos", "description": "Repo GitHub đã duyệt"},
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
