use crate::error::{AppError, AppResult};
use crate::handlers::auth::unread_count;
use crate::middleware::{AuthUser, CurrentUser};
use crate::repositories::{GameRepo, InteractionRepo, UserRepo};
use crate::state::AppState;
use crate::templates::*;
use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use serde::Deserialize;
use std::sync::Arc;

// ============= View profile =============
pub async fn show_profile(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Path(username): Path<String>,
) -> AppResult<ProfileTemplate> {
    let user = UserRepo::find_by_username(&state.db, &username)
        .await?
        .ok_or_else(|| AppError::NotFound("Người dùng không tồn tại".into()))?;
    let stats = UserRepo::stats(&state.db, user.id).await?;
    let games = GameRepo::by_user(&state.db, user.id, 24, 0).await?;
    let is_self = current_user.as_ref().map(|u| u.id == user.id).unwrap_or(false);
    let is_following = if let Some(ref cu) = current_user {
        if !is_self {
            InteractionRepo::is_following(&state.db, cu.id, user.id)
                .await
                .unwrap_or(false)
        } else {
            false
        }
    } else {
        false
    };
    let preferences = UserRepo::get_preferences(&state.db, user.id).await.unwrap_or_default();
    let unread = match &current_user {
        Some(u) => unread_count(&state, u.id).await,
        None => 0,
    };
    Ok(ProfileTemplate {
        current_user,
        unread_notifications: unread,
        user,
        stats,
        games,
        is_following,
        is_self,
        preferences,
    })
}

// ============= My profile redirect =============
pub async fn my_profile(AuthUser(user): AuthUser) -> Redirect {
    Redirect::to(&format!("/u/{}", user.username))
}

// ============= Edit profile form =============
pub async fn edit_profile_form(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<EditProfileTemplate> {
    let preferences = UserRepo::get_preferences(&state.db, user.id).await.unwrap_or_default();
    let unread = unread_count(&state, user.id).await;
    Ok(EditProfileTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        preferences,
    })
}

// ============= Update profile =============
#[derive(Deserialize)]
pub struct ProfileForm {
    pub display_name: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub theme: Option<String>,
    pub language: Option<String>,
    pub email_notifications: Option<String>,
    pub show_online: Option<String>,
}

pub async fn update_profile(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Form(form): Form<ProfileForm>,
) -> AppResult<Redirect> {
    let display_name = form.display_name.trim();
    if display_name.is_empty() {
        return Err(AppError::BadRequest("Tên hiển thị không được để trống".into()));
    }
    let bio = form.bio.unwrap_or_default();
    let avatar_url = form.avatar_url.as_deref().filter(|s| !s.is_empty());

    UserRepo::update_profile(&state.db, user.id, display_name, &bio, avatar_url).await?;

    // Update preferences
    let theme = form.theme.unwrap_or_else(|| "dark".into());
    let language = form.language.unwrap_or_else(|| "vi".into());
    let email_notif = form.email_notifications.is_some();
    let show_online = form.show_online.is_some();
    UserRepo::update_preferences(&state.db, user.id, &theme, email_notif, show_online, &language)
        .await?;

    Ok(Redirect::to(&format!("/u/{}", user.username)))
}

// ============= Bookmarks page =============
pub async fn bookmarks_page(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<BookmarksTemplate> {
    let games = InteractionRepo::bookmarks_for_user(&state.db, user.id, 50, 0).await?;
    let unread = unread_count(&state, user.id).await;
    Ok(BookmarksTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        games,
    })
}
