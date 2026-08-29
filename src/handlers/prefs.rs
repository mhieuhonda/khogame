//! v3.0.0 — Handlers tùy chọn thông báo (/settings/notifications).

use crate::error::{AppError, AppResult};
use crate::middleware::{AuthUser, CurrentUser};
use crate::repositories::PrefsRepo;
use crate::state::AppState;
use crate::templates::NotifPrefsTemplate;
use axum::extract::State;
use serde::Deserialize;
use std::sync::Arc;

/// GET /settings/notifications — trang tùy chọn (yêu cầu đăng nhập).
/// # Errors
/// Trả lỗi khi DB fail.
pub async fn page(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    axum::extract::Query(q): axum::extract::Query<PrefsQuery>,
) -> AppResult<NotifPrefsTemplate> {
    let Some(user) = current_user else {
        return Err(AppError::Unauthorized);
    };
    let prefs = PrefsRepo::get(&state.db, user.id).await?;
    let unread = crate::handlers::auth::unread_count(&state, user.id).await;
    Ok(NotifPrefsTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        prefs,
        saved: q.saved.is_some(),
    })
}

#[derive(Debug, Default, Deserialize)]
pub struct PrefsQuery {
    #[serde(default)]
    pub saved: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PrefsForm {
    #[serde(default)]
    pub inapp_follow: Option<String>,
    #[serde(default)]
    pub inapp_new_game: Option<String>,
    #[serde(default)]
    pub inapp_review: Option<String>,
    #[serde(default)]
    pub inapp_mention: Option<String>,
    #[serde(default)]
    pub weekly_digest: Option<String>,
}

/// POST /settings/notifications — lưu tùy chọn (checkbox HTML: có value
/// khi tick, vắng hoàn toàn khi bỏ tick → `Option<String>`).
/// # Errors
/// Trả lỗi khi DB fail.
pub async fn save(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    axum::extract::Form(form): axum::extract::Form<PrefsForm>,
) -> AppResult<axum::response::Redirect> {
    PrefsRepo::save(
        &state.db,
        user.id,
        form.inapp_follow.is_some(),
        form.inapp_new_game.is_some(),
        form.inapp_review.is_some(),
        form.inapp_mention.is_some(),
        form.weekly_digest.is_some(),
    )
    .await?;
    Ok(axum::response::Redirect::to(
        "/settings/notifications?saved=1",
    ))
}
