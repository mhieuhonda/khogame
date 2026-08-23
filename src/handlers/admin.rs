use crate::error::{AppError, AppResult};
use crate::handlers::auth::unread_count;
use crate::middleware::AuthUser;
use crate::models::report::{ReportStatus};
use crate::repositories::{GameRepo, ReportRepo};
use crate::state::AppState;
use crate::templates::{AdminReportsTemplate, AdminTemplate};
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::Html;
use axum::Form;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

pub async fn dashboard(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<AdminTemplate> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let total_games = GameRepo::count_published(&state.db).await.unwrap_or(0);
    let total_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
    let total_downloads: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
    let pending_reports = ReportRepo::count_pending(&state.db).await.unwrap_or(0);
    let recent_reports = ReportRepo::list(&state.db, Some("pending"), 10, 0).await.unwrap_or_default();
    let recent_games = GameRepo::list_published(&state.db, 10, 0, "latest").await.unwrap_or_default();
    let unread = unread_count(&state, user.id).await;
    Ok(AdminTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        total_games,
        total_users,
        total_downloads,
        pending_reports,
        recent_reports,
        recent_games,
    })
}

#[derive(Deserialize)]
pub struct ReportsQuery {
    pub status: Option<String>,
}

pub async fn reports(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Query(q): Query<ReportsQuery>,
) -> AppResult<AdminReportsTemplate> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let reports = ReportRepo::list(&state.db, q.status.as_deref(), 50, 0).await?;
    let unread = unread_count(&state, user.id).await;
    Ok(AdminReportsTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        reports,
        status_filter: q.status,
    })
}

#[derive(Deserialize)]
pub struct ResolveForm {
    pub status: String,
    pub resolution: Option<String>,
}

pub async fn resolve_report(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Form(form): Form<ResolveForm>,
) -> AppResult<Html<String>> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let status = match form.status.as_str() {
        "reviewing" => ReportStatus::Reviewing,
        "resolved" => ReportStatus::Resolved,
        "dismissed" => ReportStatus::Dismissed,
        _ => return Err(AppError::BadRequest("Trạng thái không hợp lệ".into())),
    };
    ReportRepo::resolve(&state.db, id, user.id, &form.status, &form.resolution.unwrap_or_default()).await?;
    let _ = status;
    // Return updated report row HTML
    let reports = ReportRepo::list(&state.db, None, 50, 0).await?;
    let r = reports.iter().find(|r| r.id == id);
    let html = if let Some(r) = r {
        format!(
            r#"<div class="report-info"><a href="/games/{}" class="report-game-title">{}</a><div class="report-meta"><span class="report-reason">{}</span><span class="report-reporter">bởi {}</span><span class="report-time">{}</span></div><span class="status-badge" style="color: {}">{}</span></div>"#,
            r.game_slug,
            r.game_title,
            r.reason.label(),
            r.reporter_name,
            crate::utils::time_ago(r.created_at),
            r.status.color(),
            r.status.label()
        )
    } else {
        "".to_string()
    };
    Ok(Html(html))
}

#[derive(Deserialize)]
pub struct HideGameForm {
    pub hide: Option<String>,
}

pub async fn hide_game(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Form(form): Form<HideGameForm>,
) -> AppResult<Html<String>> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let status = if form.hide.is_some() { "hidden" } else { "published" };
    GameRepo::set_status(&state.db, id, status).await?;
    Ok(Html(format!("<div class='alert alert-success'>Đã {} game.</div>",
        if status == "hidden" { "ẩn" } else { "hiện" })))
}

pub async fn feature_game(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Html<String>> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    // Toggle featured
    let game = GameRepo::find_by_id(&state.db, id).await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    GameRepo::set_featured(&state.db, id, !game.is_featured).await?;
    Ok(Html(format!("<div class='alert alert-success'>Đã {} nổi bật.</div>",
        if !game.is_featured { "đặt làm" } else { "bỏ" })))
}

pub async fn pin_comment(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Html<String>> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let pinned = crate::repositories::CommentRepo::toggle_pin(&state.db, id).await?;
    let comment = crate::repositories::CommentRepo::find_by_id(&state.db, id).await?
        .ok_or_else(|| AppError::NotFound("Bình luận không tồn tại".into()))?;
    let partial = crate::templates::CommentItemPartial {
        comment: &comment,
        game_slug: "",
        current_user: Some(&user),
    };
    let _ = pinned;
    Ok(Html(partial.render()?))
}
