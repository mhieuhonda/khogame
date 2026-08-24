use crate::error::{AppError, AppResult};
use crate::handlers::auth::unread_count;
use crate::middleware::{AuthUser, CurrentUser};
use crate::models::game::{GameForm, GameStatus, Platform};
use crate::models::report::ReportReason;
use crate::repositories::{CategoryRepo, GameRepo, InteractionRepo, ReportRepo, TagRepo};
use crate::state::AppState;
use crate::templates::*;
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;
use std::sync::Arc;

/// Helper to get unread notification count for an optional user
async fn unread_for(state: &AppState, user: Option<&crate::models::user::User>) -> i64 {
    match user {
        Some(u) => unread_count(state, u.id).await,
        None => 0,
    }
}

// ============= Home =============
pub async fn home(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<IndexTemplate> {
    let unread = unread_for(&state, current_user.as_ref()).await;
    let featured_games = GameRepo::featured(&state.db, 6, 0)
        .await
        .unwrap_or_default();
    let latest_games = GameRepo::list_published(&state.db, 12, 0, "latest").await?;
    let trending_games = GameRepo::list_published(&state.db, 12, 0, "trending").await?;
    let top_rated_games = GameRepo::list_published(&state.db, 12, 0, "top_rated").await?;
    let categories = CategoryRepo::list_with_counts(&state.db).await?;
    let popular_tags = TagRepo::popular(&state.db, 20).await.unwrap_or_default();
    let total_games = GameRepo::count_published(&state.db).await.unwrap_or(0);

    // JSON-LD schema.org/WebSite với SearchAction — giúp Google hiển thị
    // sitelinks searchbox ngay trên kết quả tìm kiếm (rich result).
    let json_ld = serde_json::json!({
        "@context": "https://schema.org",
        "@type": "WebSite",
        "name": "Kho Game",
        "url": state.config.base_url,
        "description": "Nền tảng chia sẻ game độc lập cho cộng đồng Việt Nam",
        "inLanguage": "vi-VN",
        "potentialAction": {
            "@type": "SearchAction",
            "target": {
                "@type": "EntryPoint",
                "urlTemplate": format!("{}/search?q={{search_term_string}}", state.config.base_url),
            },
            "query-input": "required name=search_term_string",
        }
    });
    let json_ld = format!(
        "<script type=\"application/ld+json\">\n{}\n</script>",
        serde_json::to_string_pretty(&json_ld).unwrap_or_default()
    );

    Ok(IndexTemplate {
        current_user,
        unread_notifications: unread,
        featured_games,
        latest_games,
        trending_games,
        top_rated_games,
        categories,
        popular_tags,
        total_games,
        json_ld,
    })
}

// ============= New game form =============
pub async fn new_game_form(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<NewGameTemplate> {
    let categories = CategoryRepo::list_all(&state.db).await?;
    let unread = unread_count(&state, user.id).await;
    Ok(NewGameTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        categories,
    })
}

// ============= Create game =============
pub async fn create_game(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Form(form): Form<GameForm>,
) -> AppResult<Redirect> {
    if form.title.trim().is_empty() {
        return Err(AppError::BadRequest("Tiêu đề không được để trống".into()));
    }
    // Giới hạn độ dài để chống lạm dụng (title được dùng làm slug + hiển thị)
    if form.title.chars().count() > 200 {
        return Err(AppError::BadRequest("Tiêu đề tối đa 200 ký tự".into()));
    }
    if form.content.trim().is_empty() {
        return Err(AppError::BadRequest("Nội dung không được để trống".into()));
    }
    // Validate cover_image: chỉ http/https (chống javascript: src dù
    // browser không execute JS cho <img src> thì vẫn chống tracking pixel
    // và scheme lạ).
    if !crate::utils::is_safe_url(&form.cover_image) {
        return Err(AppError::BadRequest(
            "Cover image URL phải là http:// hoặc https://".into(),
        ));
    }
    if form.cover_image.len() > 2048 {
        return Err(AppError::BadRequest(
            "Cover image URL quá dài (tối đa 2048 ký tự)".into(),
        ));
    }
    // Validate trailer_url: chỉ http/https (filter youtube_embed sẽ
    // trích ID YouTube an toàn, nhưng vẫn nên chặn scheme lạ sớm).
    if !crate::utils::is_safe_url(&form.trailer_url) {
        return Err(AppError::BadRequest(
            "Trailer URL phải là http:// hoặc https://".into(),
        ));
    }
    // Validate các link tải: chỉ cho phép http/https để chống XSS qua
    // javascript: scheme. HTMX có HX-Redirect sẽ làm window.location = url,
    // nếu url là javascript:alert(1) sẽ execute JS trong context user.
    for (label, url) in [
        ("Android", form.android_link.as_deref()),
        ("iOS", form.ios_link.as_deref()),
        ("Windows", form.windows_link.as_deref()),
        ("Linux", form.linux_link.as_deref()),
        ("macOS", form.macos_link.as_deref()),
    ] {
        if let Some(u) = url.filter(|s| !s.is_empty()) {
            let lower = u.to_ascii_lowercase();
            if !(lower.starts_with("http://") || lower.starts_with("https://")) {
                return Err(AppError::BadRequest(format!(
                    "Link tải {} phải là http:// hoặc https:// (đã chặn javascript: và các scheme nguy hiểm)",
                    label
                )));
            }
            if u.len() > 2048 {
                return Err(AppError::BadRequest(format!(
                    "Link tải {} quá dài (tối đa 2048 ký tự)",
                    label
                )));
            }
        }
    }
    if form
        .android_link
        .as_deref()
        .filter(|s| !s.is_empty())
        .is_none()
        && form.ios_link.as_deref().filter(|s| !s.is_empty()).is_none()
        && form
            .windows_link
            .as_deref()
            .filter(|s| !s.is_empty())
            .is_none()
        && form
            .linux_link
            .as_deref()
            .filter(|s| !s.is_empty())
            .is_none()
        && form
            .macos_link
            .as_deref()
            .filter(|s| !s.is_empty())
            .is_none()
    {
        return Err(AppError::BadRequest("Phải có ít nhất một link tải".into()));
    }

    // Sinh slug duy nhất: thử base, rồi base-2, base-3... tránh 500 do
    // trùng ràng buộc UNIQUE khi có nhiều game cùng tên
    let mut slug = slug::slugify(&form.title);
    if slug.is_empty() {
        slug = "game".into();
    }
    let mut suffix = 1;
    while GameRepo::slug_exists(&state.db, &slug)
        .await
        .unwrap_or(true)
    {
        suffix += 1;
        slug = format!("{}-{}", slug::slugify(&form.title), suffix);
        if suffix > 100 {
            slug = format!("{}-{}", slug, uuid::Uuid::new_v4().simple());
            break;
        }
    }

    let id = GameRepo::create(&state.db, user.id, &form, &slug).await?;
    tracing::info!("Game created: {} ({})", id, slug);

    Ok(Redirect::to(&format!("/games/{}", slug)))
}

// ============= Show game =============
pub async fn show_game(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<GameShowTemplate> {
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;

    let is_owner = current_user
        .as_ref()
        .map(|u| u.id == game.user_id)
        .unwrap_or(false);
    // Staff (admin/moderator) được xem game ẩn/nháp để kiểm duyệt báo cáo
    let is_staff = current_user
        .as_ref()
        .map(|u| u.role.is_staff())
        .unwrap_or(false);
    if !is_owner && !is_staff && !matches!(game.status, GameStatus::Published) {
        return Err(AppError::NotFound("Game không tồn tại".into()));
    }

    let mut game = game;
    if !is_owner {
        let _ = GameRepo::increment_view_count(&state.db, game.id).await;
        let _ = crate::repositories::StatsRepo::record_view(&state.db, game.id).await;
        game.view_count += 1;
    }

    let author = crate::repositories::UserRepo::find_by_id(&state.db, game.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Tác giả không tồn tại".into()))?;
    let links = GameRepo::get_links(&state.db, game.id).await?;
    let screenshots = GameRepo::get_screenshots(&state.db, game.id).await?;
    let tags = GameRepo::get_tags(&state.db, game.id).await?;
    let category = if let Some(cat_id) = game.category_id {
        CategoryRepo::find_by_id(&state.db, cat_id).await?
    } else {
        None
    };
    let comments = crate::repositories::CommentRepo::list_by_game(
        &state.db,
        game.id,
        current_user.as_ref().map(|u| u.id),
        50,
        0,
    )
    .await?;
    let related_games = GameRepo::related(&state.db, game.id, game.category_id, 6).await?;

    let (is_liked, is_bookmarked, is_following_author, user_rating) =
        if let Some(ref u) = current_user {
            let liked = InteractionRepo::is_liked(&state.db, game.id, u.id)
                .await
                .unwrap_or(false);
            let bm = InteractionRepo::is_bookmarked(&state.db, game.id, u.id)
                .await
                .unwrap_or(false);
            let following = if u.id != author.id {
                InteractionRepo::is_following(&state.db, u.id, author.id)
                    .await
                    .unwrap_or(false)
            } else {
                false
            };
            let rating = InteractionRepo::get_user_rating(&state.db, game.id, u.id)
                .await
                .unwrap_or(None);
            (liked, bm, following, rating)
        } else {
            (false, false, false, None)
        };

    let unread = unread_for(&state, current_user.as_ref()).await;

    // Structured data (JSON-LD) cho SEO: schema.org/VideoGame —
    // giúp Google hiển thị rich snippet (rating, lượt tải, v.v.) trên
    // kết quả tìm kiếm. Serialize trước bằng serde_json thay vì cố
    // render JSON trong askama (không có filter json_encode ở 0.16).
    let json_ld = build_game_json_ld(
        &state.config.base_url,
        &game,
        &author,
        &links,
        &tags,
        category.as_ref(),
    );

    Ok(GameShowTemplate {
        current_user,
        unread_notifications: unread,
        game,
        author,
        links,
        screenshots,
        tags,
        category,
        comments,
        related_games,
        is_liked,
        is_bookmarked,
        is_following_author,
        is_owner,
        user_rating,
        base_url: state.config.base_url.clone(),
        json_ld,
    })
}

/// Dựng JSON-LD schema.org/VideoGame để nhúng vào <head> của trang game.
/// Dùng serde_json::Value để tránh lỗi cú pháp JSON (escape không đúng).
/// Trả về tag <script type="application/ld+json">...</script> hoàn chỉnh.
fn build_game_json_ld(
    base_url: &str,
    game: &crate::models::game::Game,
    author: &crate::models::user::User,
    links: &[crate::models::game::GameLink],
    tags: &[String],
    category: Option<&crate::models::category::Category>,
) -> String {
    use serde_json::{json, Value};
    let mut root = json!({
        "@context": "https://schema.org",
        "@type": "VideoGame",
        "name": game.title,
        "url": format!("{}/games/{}", base_url, game.slug),
        "author": {
            "@type": "Person",
            "name": author.display_name,
            "url": format!("{}/u/{}", base_url, author.username),
        },
        "publisher": {
            "@type": "Organization",
            "name": "Kho Game",
            "url": base_url,
        },
        "operatingSystem": links.iter().map(|l| l.platform.label()).collect::<Vec<_>>(),
        "interactionStatistic": [
            json!({
                "@type": "InteractionCounter",
                "interactionType": "https://schema.org/WatchAction",
                "userInteractionCount": game.view_count,
            }),
            json!({
                "@type": "InteractionCounter",
                "interactionType": "https://schema.org/DownloadAction",
                "userInteractionCount": game.download_count,
            }),
            json!({
                "@type": "InteractionCounter",
                "interactionType": "https://schema.org/LikeAction",
                "userInteractionCount": game.like_count,
            }),
            json!({
                "@type": "InteractionCounter",
                "interactionType": "https://schema.org/CommentAction",
                "userInteractionCount": game.comment_count,
            }),
        ],
    });
    let obj = root.as_object_mut().unwrap();
    if !game.excerpt_or().is_empty() {
        obj.insert("description".into(), json!(game.excerpt_or()));
    }
    if let Some(url) = game.cover_image.as_deref().filter(|s| !s.is_empty()) {
        obj.insert("image".into(), json!(url));
    }
    if game.rating_count > 0 {
        obj.insert(
            "aggregateRating".into(),
            json!({
                "@type": "AggregateRating",
                "ratingValue": game.rating_avg_f64(),
                "ratingCount": game.rating_count,
                "bestRating": 5,
                "worstRating": 1,
            }),
        );
    }
    if let Some(d) = game.release_date {
        obj.insert(
            "datePublished".into(),
            json!(d.format("%Y-%m-%d").to_string()),
        );
    }
    if let Some(cat) = category {
        obj.insert("genre".into(), json!(cat.name));
    }
    if !tags.is_empty() {
        obj.insert(
            "keywords".into(),
            Value::Array(tags.iter().map(|t| json!(t)).collect()),
        );
    }
    let pretty = serde_json::to_string_pretty(&root).unwrap_or_else(|_| "{}".into());
    format!(
        "<script type=\"application/ld+json\">\n{}\n</script>",
        pretty
    )
}

// ============= Edit game =============
pub async fn edit_game_form(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
) -> AppResult<EditGameTemplate> {
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    if game.user_id != user.id && !user.role.is_staff() {
        return Err(AppError::Forbidden("Bạn không có quyền chỉnh sửa".into()));
    }
    let categories = CategoryRepo::list_all(&state.db).await?;
    let links = GameRepo::get_links(&state.db, game.id).await?;
    let screenshots = GameRepo::get_screenshots(&state.db, game.id).await?;
    let tags = GameRepo::get_tags(&state.db, game.id).await?;
    let unread = unread_count(&state, user.id).await;
    Ok(EditGameTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        categories,
        game,
        links,
        screenshots,
        tags,
    })
}

// ============= Update game =============
pub async fn update_game(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Form(form): Form<GameForm>,
) -> AppResult<Redirect> {
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    if game.user_id != user.id && !user.role.is_staff() {
        return Err(AppError::Forbidden("Bạn không có quyền chỉnh sửa".into()));
    }
    // Validate tương tự create_game: title không rỗng, không quá dài,
    // link tải chỉ http/https (chống javascript: XSS qua HX-Redirect).
    if form.title.trim().is_empty() {
        return Err(AppError::BadRequest("Tiêu đề không được để trống".into()));
    }
    if form.title.chars().count() > 200 {
        return Err(AppError::BadRequest("Tiêu đề tối đa 200 ký tự".into()));
    }
    if !crate::utils::is_safe_url(&form.cover_image) {
        return Err(AppError::BadRequest(
            "Cover image URL phải là http:// hoặc https://".into(),
        ));
    }
    if form.cover_image.len() > 2048 {
        return Err(AppError::BadRequest(
            "Cover image URL quá dài (tối đa 2048 ký tự)".into(),
        ));
    }
    if !crate::utils::is_safe_url(&form.trailer_url) {
        return Err(AppError::BadRequest(
            "Trailer URL phải là http:// hoặc https://".into(),
        ));
    }
    for (label, url) in [
        ("Android", form.android_link.as_deref()),
        ("iOS", form.ios_link.as_deref()),
        ("Windows", form.windows_link.as_deref()),
        ("Linux", form.linux_link.as_deref()),
        ("macOS", form.macos_link.as_deref()),
    ] {
        if let Some(u) = url.filter(|s| !s.is_empty()) {
            let lower = u.to_ascii_lowercase();
            if !(lower.starts_with("http://") || lower.starts_with("https://")) {
                return Err(AppError::BadRequest(format!(
                    "Link tải {} phải là http:// hoặc https://",
                    label
                )));
            }
            if u.len() > 2048 {
                return Err(AppError::BadRequest(format!(
                    "Link tải {} quá dài (tối đa 2048 ký tự)",
                    label
                )));
            }
        }
    }

    GameRepo::update(&state.db, game.id, &form).await?;
    Ok(Redirect::to(&format!("/games/{}", slug)))
}

// ============= Delete game =============
pub async fn delete_game(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
) -> AppResult<Redirect> {
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    if game.user_id != user.id && !user.role.is_staff() {
        return Err(AppError::Forbidden("Bạn không có quyền xóa".into()));
    }
    GameRepo::delete(&state.db, game.id).await?;
    Ok(Redirect::to("/"))
}

// ============= Download (hidden link redirect) =============
#[derive(Deserialize)]
pub struct DownloadForm {
    pub platform: String,
}

pub async fn download_game(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Form(form): Form<DownloadForm>,
) -> AppResult<Response> {
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    let platform = Platform::from_str(&form.platform)
        .ok_or_else(|| AppError::BadRequest("Nền tảng không hợp lệ".into()))?;
    let url = GameRepo::get_link_for_platform(&state.db, game.id, &platform)
        .await?
        .ok_or_else(|| AppError::NotFound("Link tải không tồn tại".into()))?;

    let _ =
        InteractionRepo::record_download(&state.db, game.id, Some(user.id), &form.platform, None)
            .await;
    let _ = GameRepo::increment_download_count(&state.db, game.id).await;
    let _ = crate::repositories::StatsRepo::record_download(&state.db, game.id).await;

    Ok((
        StatusCode::OK,
        [("X-Redirect", url.as_str()), ("HX-Redirect", url.as_str())],
        "",
    )
        .into_response())
}

// ============= Report form (modal HTML) =============
pub async fn report_form(
    State(state): State<Arc<AppState>>,
    AuthUser(_user): AuthUser,
    Path(slug): Path<String>,
) -> AppResult<Html<String>> {
    let _ = state;
    let partial = ReportModalPartial { slug: &slug };
    Ok(Html(partial.render()?))
}

// ============= Submit report =============
#[derive(Deserialize)]
pub struct ReportForm {
    pub reason: String,
    pub description: Option<String>,
}

pub async fn submit_report(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Form(form): Form<ReportForm>,
) -> AppResult<Html<String>> {
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;

    let reason = ReportReason::from_str(&form.reason)
        .ok_or_else(|| AppError::BadRequest("Lý do không hợp lệ".into()))?;

    if ReportRepo::has_reported(&state.db, game.id, user.id).await? {
        return Ok(Html(
            "<div class='alert alert-warning'>Bạn đã báo cáo game này rồi.</div>".into(),
        ));
    }

    ReportRepo::create(
        &state.db,
        game.id,
        user.id,
        &reason,
        &form.description.unwrap_or_default(),
    )
    .await?;

    Ok(Html(
        "<div class='alert alert-success'>✓ Báo cáo đã được gửi. Cảm ơn bạn đã đóng góp!</div>"
            .into(),
    ))
}

// ============= List pages =============
#[derive(Deserialize, Default)]
pub struct ListQuery {
    pub sort: Option<String>,
    pub page: Option<i64>,
}

async fn build_list_template(
    state: &AppState,
    current_user: Option<crate::models::user::User>,
    title: &str,
    list_type: &str,
    default_sort: &str,
    q: ListQuery,
) -> AppResult<GameListTemplate> {
    let sort = q.sort.unwrap_or_else(|| default_sort.to_string());
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 24;
    let offset = (page - 1) * per_page;

    // Trang "featured" chỉ liệt kê game nổi bật, không phải toàn bộ game
    let (games, total) = if list_type == "featured" {
        (
            GameRepo::featured(&state.db, per_page, offset).await?,
            GameRepo::count_featured(&state.db).await.unwrap_or(0),
        )
    } else {
        (
            GameRepo::list_published(&state.db, per_page, offset, &sort).await?,
            GameRepo::count_published(&state.db).await.unwrap_or(0),
        )
    };
    let unread = unread_for(state, current_user.as_ref()).await;
    let base = if list_type == "all" {
        "/games".to_string()
    } else {
        format!("/games/{}", list_type)
    };
    Ok(GameListTemplate {
        current_user,
        unread_notifications: unread,
        title: title.into(),
        games,
        total,
        page,
        per_page,
        sort,
        list_type: list_type.into(),
        base_url: base,
        category: None,
        tag: None,
    })
}

/// Danh mục đầy đủ tại /games (trước đây chỉ có POST → GET trả 405)
pub async fn list_all(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(q): Query<ListQuery>,
) -> AppResult<GameListTemplate> {
    build_list_template(&state, current_user, "🎮 Tất cả game", "all", "latest", q).await
}

pub async fn list_latest(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(q): Query<ListQuery>,
) -> AppResult<GameListTemplate> {
    build_list_template(
        &state,
        current_user,
        "🎮 Game mới nhất",
        "latest",
        "latest",
        q,
    )
    .await
}

pub async fn list_trending(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(q): Query<ListQuery>,
) -> AppResult<GameListTemplate> {
    build_list_template(
        &state,
        current_user,
        "🔥 Đang thịnh hành",
        "trending",
        "trending",
        q,
    )
    .await
}

pub async fn list_top_rated(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(q): Query<ListQuery>,
) -> AppResult<GameListTemplate> {
    build_list_template(
        &state,
        current_user,
        "⭐ Đánh giá cao nhất",
        "top-rated",
        "top_rated",
        q,
    )
    .await
}

pub async fn list_downloads(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(q): Query<ListQuery>,
) -> AppResult<GameListTemplate> {
    build_list_template(
        &state,
        current_user,
        "⬇️ Tải nhiều nhất",
        "downloads",
        "downloads",
        q,
    )
    .await
}

pub async fn list_featured(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(q): Query<ListQuery>,
) -> AppResult<GameListTemplate> {
    build_list_template(
        &state,
        current_user,
        "⭐ Game nổi bật",
        "featured",
        "trending",
        q,
    )
    .await
}

// ============= Category / Tag listing =============
pub async fn list_by_category(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Path(cat_slug): Path<String>,
    Query(q): Query<ListQuery>,
) -> AppResult<GameListTemplate> {
    let category = CategoryRepo::find_by_slug(&state.db, &cat_slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Thể loại không tồn tại".into()))?;
    let sort = q.sort.unwrap_or_else(|| "latest".into());
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 24;
    let offset = (page - 1) * per_page;
    let games = GameRepo::by_category(&state.db, &cat_slug, per_page, offset).await?;
    // Đếm đúng tổng số game của thể loại (trước đây lấy games.len() →
    // pagination luôn báo 1 trang dù còn game ở trang sau)
    let total = GameRepo::count_by_category(&state.db, &cat_slug)
        .await
        .unwrap_or(0);
    let unread = unread_for(&state, current_user.as_ref()).await;
    Ok(GameListTemplate {
        current_user,
        unread_notifications: unread,
        title: format!("📁 {}", category.name),
        games,
        total,
        page,
        per_page,
        sort,
        list_type: "category".into(),
        base_url: format!("/c/{}", cat_slug),
        category: Some(category),
        tag: None,
    })
}

pub async fn list_by_tag(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Path(tag_slug): Path<String>,
    Query(q): Query<ListQuery>,
) -> AppResult<GameListTemplate> {
    let tag = TagRepo::find_by_slug(&state.db, &tag_slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Tag không tồn tại".into()))?;
    let sort = q.sort.unwrap_or_else(|| "latest".into());
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 24;
    let offset = (page - 1) * per_page;
    let games = GameRepo::by_tag(&state.db, &tag_slug, per_page, offset).await?;
    let total = GameRepo::count_by_tag(&state.db, &tag_slug)
        .await
        .unwrap_or(0);
    let unread = unread_for(&state, current_user.as_ref()).await;
    Ok(GameListTemplate {
        current_user,
        unread_notifications: unread,
        title: format!("#{}", tag.name),
        games,
        total,
        page,
        per_page,
        sort,
        list_type: "tag".into(),
        base_url: format!("/t/{}", tag_slug),
        category: None,
        tag: Some(tag),
    })
}

pub async fn list_categories(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<CategoriesPageTemplate> {
    let cats = CategoryRepo::list_with_counts(&state.db).await?;
    let unread = unread_for(&state, current_user.as_ref()).await;
    Ok(CategoriesPageTemplate {
        current_user,
        unread_notifications: unread,
        categories: cats,
    })
}

// ============= Search =============
#[derive(Deserialize, Default)]
pub struct SearchQuery {
    // Sửa: `q` là Option để không trả 400 khi user vào /search không có query string.
    // Trang /search trống sẽ hiển thị danh sách game mới nhất (như browse page).
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub page: Option<i64>,
}

pub async fn search(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(q): Query<SearchQuery>,
) -> AppResult<SearchTemplate> {
    let sort = q.sort.unwrap_or_else(|| "latest".into());
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 24;
    let offset = (page - 1) * per_page;
    let games = if q.q.trim().is_empty() && q.category.is_none() && q.platform.is_none() {
        GameRepo::list_published(&state.db, per_page, offset, &sort).await?
    } else {
        GameRepo::search(
            &state.db,
            &q.q,
            q.category.as_deref(),
            q.platform.as_deref(),
            &sort,
            per_page,
            offset,
        )
        .await?
    };
    // Đếm đúng số kết quả khớp bộ lọc (trước đây lấy tổng game đã đăng
    // → "N kết quả" và phân trang sai khi có từ khóa/bộ lọc)
    let total = if q.q.trim().is_empty() && q.category.is_none() && q.platform.is_none() {
        GameRepo::count_published(&state.db).await.unwrap_or(0)
    } else {
        GameRepo::count_search(
            &state.db,
            &q.q,
            q.category.as_deref(),
            q.platform.as_deref(),
        )
        .await
        .unwrap_or(0)
    };
    let categories = CategoryRepo::list_all(&state.db).await?;
    let unread = unread_for(&state, current_user.as_ref()).await;
    Ok(SearchTemplate {
        current_user,
        unread_notifications: unread,
        query: q.q,
        sort,
        platform: q.platform,
        category_slug: q.category,
        games,
        categories,
        total,
        page,
        per_page,
    })
}

// ============= Share tracking =============
#[derive(Deserialize)]
pub struct ShareForm {
    pub platform: String,
}

pub async fn share_game(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Path(slug): Path<String>,
    Form(form): Form<ShareForm>,
) -> AppResult<Html<String>> {
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    let user_id = current_user.as_ref().map(|u| u.id);
    // Chuẩn hoá platform về enum hợp lệ trong DB, giá trị lạ → "copy"
    // (trước đây chuỗi lạ gây lỗi cast enum và share bị nuốt im lặng)
    let valid_platforms = [
        "facebook", "twitter", "telegram", "whatsapp", "copy", "native",
    ];
    let platform = if valid_platforms.contains(&form.platform.as_str()) {
        form.platform.clone()
    } else {
        "copy".to_string()
    };
    let _ = InteractionRepo::record_share(&state.db, game.id, user_id, &platform).await;
    let _ = GameRepo::increment_share_count(&state.db, game.id).await;
    Ok(Html("<span></span>".into()))
}

// ============= Game của tôi =============
pub async fn my_games(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<MyGamesTemplate> {
    let games = GameRepo::all_by_user(&state.db, user.id).await?;
    let unread = unread_count(&state, user.id).await;
    Ok(MyGamesTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        games,
    })
}

// ============= Xuất bản game nháp =============
pub async fn publish_game(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
) -> AppResult<Html<String>> {
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    if game.user_id != user.id && !user.role.is_staff() {
        return Err(AppError::Forbidden("Bạn không có quyền".into()));
    }
    sqlx::query(
        "UPDATE games SET status = 'published', \
         published_at = COALESCE(published_at, NOW()) WHERE id = $1",
    )
    .bind(game.id)
    .execute(&state.db)
    .await?;
    Ok(Html(
        "<div class='alert alert-success'>Đã xuất bản game.</div>".into(),
    ))
}
