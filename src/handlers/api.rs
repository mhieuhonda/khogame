use crate::error::{AppError, AppResult};
use crate::middleware::{AuthUser, CurrentUser};
use crate::repositories::{
    CategoryRepo, CommentRepo, GameRepo, RepoRepo, SettingsRepo, StatsRepo, UserRepo,
};
use crate::state::AppState;
use axum::extract::{Form, Path, Query, State};
use axum::http::header;
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
        "author": author.map(|u| serde_json::json!({
            "username": u.username,
            "display_name": u.display_name,
            "avatar_url": u.avatar_url,
        })),
        "platforms": links.iter().map(|l| serde_json::json!({
            "platform": format!("{:?}", l.platform).to_lowercase(),
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
    Ok(Json(ApiList {
        data,
        total,
        page,
        per_page,
    })
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

pub async fn rss(State(state): State<Arc<AppState>>) -> AppResult<Response> {
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
    Ok((
        [(header::CONTENT_TYPE, "application/rss+xml; charset=utf-8")],
        xml,
    )
        .into_response())
}

pub async fn sitemap(State(state): State<Arc<AppState>>) -> AppResult<Response> {
    let base = &state.config.base_url;
    let mut urls = String::new();
    urls.push_str(&format!(
        r#"  <url><loc>{}/</loc><changefreq>hourly</changefreq><priority>1.0</priority></url>
"#,
        base
    ));
    for page in [
        "/games/latest",
        "/games/trending",
        "/games/top-rated",
        "/games/downloads",
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
    Ok((
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        xml,
    )
        .into_response())
}

pub async fn robots(State(state): State<Arc<AppState>>) -> Response {
    let base = &state.config.base_url;
    let txt = format!(
        "User-agent: *\nAllow: /\nDisallow: /admin\nDisallow: /profile\nDisallow: /notifications\n\nSitemap: {}/sitemap.xml\n",
        base
    );
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], txt).into_response()
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
    (status, Json(body)).into_response()
}

// ===================== Maintenance page dùng nội bộ =====================
#[allow(dead_code)]
pub async fn current_user_count(state: &AppState) -> i64 {
    UserRepo::count_all(&state.db).await.unwrap_or(0)
}

#[allow(dead_code)]
pub async fn mention_test(state: &AppState, user: Uuid) -> AppResult<Vec<Uuid>> {
    CommentRepo::find_mentions(&state.db, "@admin hello", user).await
}

#[allow(dead_code)]
pub async fn daily_stats(
    state: &AppState,
) -> AppResult<Vec<crate::models::settings::DailyStatRow>> {
    StatsRepo::daily_last_7_days(&state.db).await
}

#[allow(dead_code)]
pub async fn require_any_user(user: Option<crate::models::user::User>) -> AppResult<()> {
    match user {
        Some(_) => Ok(()),
        None => Err(AppError::Unauthorized),
    }
}

#[allow(dead_code)]
pub async fn unused_current(_c: CurrentUser) {}
