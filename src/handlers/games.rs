use crate::error::{AppError, AppResult};
use crate::handlers::auth::unread_count;
use crate::middleware::{AuthUser, CurrentUser};
use crate::models::game::{GameForm, GameStatus, Platform};
use crate::models::report::ReportReason;
use crate::repositories::{CategoryRepo, GameRepo, InteractionRepo, ReportRepo, TagRepo};
use crate::state::AppState;
use crate::templates::*;
use crate::utils;
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
    let featured_games = GameRepo::featured(&state.db, 6).await.unwrap_or_default();
    let latest_games = GameRepo::list_published(&state.db, 12, 0, "latest").await?;
    let trending_games = GameRepo::list_published(&state.db, 12, 0, "trending").await?;
    let top_rated_games = GameRepo::list_published(&state.db, 12, 0, "top_rated").await?;
    let categories = CategoryRepo::list_with_counts(&state.db).await?;
    let popular_tags = TagRepo::popular(&state.db, 20).await.unwrap_or_default();
    let total_games = GameRepo::count_published(&state.db).await.unwrap_or(0);

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
    Form(mut form): Form<GameForm>,
) -> AppResult<Redirect> {
    if form.title.trim().is_empty() {
        return Err(AppError::BadRequest("Tiêu đề không được để trống".into()));
    }
    if form.content.trim().is_empty() {
        return Err(AppError::BadRequest("Nội dung không được để trống".into()));
    }
    if form.android_link.as_deref().filter(|s| !s.is_empty()).is_none()
        && form.ios_link.as_deref().filter(|s| !s.is_empty()).is_none()
        && form.windows_link.as_deref().filter(|s| !s.is_empty()).is_none()
        && form.linux_link.as_deref().filter(|s| !s.is_empty()).is_none()
        && form.macos_link.as_deref().filter(|s| !s.is_empty()).is_none()
    {
        return Err(AppError::BadRequest("Phải có ít nhất một link tải".into()));
    }

    let slug_base = slug::slugify(&form.title);
    let count = GameRepo::count_slug(&state.db, &slug_base).await.unwrap_or(0);
    let slug = utils::make_unique_slug(&form.title, count);

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

    let is_owner = current_user.as_ref().map(|u| u.id == game.user_id).unwrap_or(false);
    if !is_owner && !matches!(game.status, GameStatus::Published) {
        return Err(AppError::NotFound("Game không tồn tại".into()));
    }

    if !is_owner {
        let _ = GameRepo::increment_view_count(&state.db, game.id).await;
        let _ = crate::repositories::StatsRepo::record_view(&state.db, game.id).await;
    }
    let game = GameRepo::find_by_slug(&state.db, &slug).await?.unwrap();

    let author = crate::repositories::UserRepo::find_by_id(&state.db, game.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Tác giả không tồn tại".into()))?;
    let links = GameRepo::get_links(&state.db, game.id).await?;
    let screenshots = GameRepo::get_screenshots(&state.db, game.id).await?;
    let tags = GameRepo::get_tags(&state.db, game.id).await?;
    let category = if let Some(cat_id) = game.category_id {
        CategoryRepo::list_all(&state.db)
            .await?
            .into_iter()
            .find(|c| c.id == cat_id)
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

    let (is_liked, is_bookmarked, is_following_author, user_rating) = if let Some(ref u) = current_user
    {
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
    })
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
    Form(mut form): Form<GameForm>,
) -> AppResult<Redirect> {
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    if game.user_id != user.id && !user.role.is_staff() {
        return Err(AppError::Forbidden("Bạn không có quyền chỉnh sửa".into()));
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

    let _ = InteractionRepo::record_download(
        &state.db,
        game.id,
        Some(user.id),
        &form.platform,
        None,
    )
    .await;
    let _ = GameRepo::increment_download_count(&state.db, game.id).await;
    let _ = crate::repositories::StatsRepo::record_download(&state.db, game.id).await;

    Ok((
        StatusCode::OK,
        [
            ("X-Redirect", url.as_str()),
            ("HX-Redirect", url.as_str()),
        ],
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
        "<div class='alert alert-success'>✓ Báo cáo đã được gửi. Cảm ơn bạn đã đóng góp!</div>".into(),
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
    let games = GameRepo::list_published(&state.db, per_page, offset, &sort).await?;
    let total = GameRepo::count_published(&state.db).await.unwrap_or(0);
    let unread = unread_for(state, current_user.as_ref()).await;
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
        category: None,
        tag: None,
    })
}

pub async fn list_latest(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(q): Query<ListQuery>,
) -> AppResult<GameListTemplate> {
    build_list_template(&state, current_user, "🎮 Game mới nhất", "latest", "latest", q).await
}

pub async fn list_trending(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(q): Query<ListQuery>,
) -> AppResult<GameListTemplate> {
    build_list_template(&state, current_user, "🔥 Đang thịnh hành", "trending", "trending", q).await
}

pub async fn list_top_rated(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(q): Query<ListQuery>,
) -> AppResult<GameListTemplate> {
    build_list_template(&state, current_user, "⭐ Đánh giá cao nhất", "top-rated", "top_rated", q).await
}

pub async fn list_downloads(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(q): Query<ListQuery>,
) -> AppResult<GameListTemplate> {
    build_list_template(&state, current_user, "⬇️ Tải nhiều nhất", "downloads", "downloads", q).await
}

pub async fn list_featured(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(q): Query<ListQuery>,
) -> AppResult<GameListTemplate> {
    build_list_template(&state, current_user, "⭐ Game nổi bật", "featured", "trending", q).await
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
    let total = games.len() as i64;
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
    let total = games.len() as i64;
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
        category: None,
        tag: Some(tag),
    })
}

pub async fn list_categories(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<Html<String>> {
    let cats = CategoryRepo::list_with_counts(&state.db).await?;
    let unread = unread_for(&state, current_user.as_ref()).await;
    let mut html = format!(
        r#"<!DOCTYPE html><html lang="vi" data-theme="dark"><head><meta charset="UTF-8"><title>Thể loại - Kho Game</title><link rel="stylesheet" href="/static/css/style.css"></head><body>
        <header class="site-header"><div class="container header-inner"><a href="/" class="logo"><span>Kho Game</span></a></div></header>
        <main class="site-main"><div class="container"><h1>📁 Tất cả thể loại</h1><p>Tổng cộng {} thể loại</p><div class="category-grid">"#,
        cats.len()
    );
    for c in cats {
        html.push_str(&format!(
            r#"<a href="/c/{}" class="category-card"><div class="category-icon">{}</div><div class="category-info"><h3>{}</h3><p>{} game</p></div></a>"#,
            c.slug,
            c.name.chars().next().unwrap_or('G'),
            c.name,
            c.games_count
        ));
    }
    html.push_str("</div></div></main></body></html>");
    let _ = unread;
    Ok(Html(html))
}

// ============= Search =============
#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub category: Option<String>,
    pub platform: Option<String>,
    pub sort: Option<String>,
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
    let games = if q.q.trim().is_empty() {
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
    let total = GameRepo::count_published(&state.db).await.unwrap_or(0);
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
    let game = GameRepo::find_by_slug(&state.db, &slug).await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    let user_id = current_user.as_ref().map(|u| u.id);
    let _ = InteractionRepo::record_share(&state.db, game.id, user_id, &form.platform).await;
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
