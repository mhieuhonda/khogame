use crate::error::AppResult;
use crate::handlers::auth::unread_count;
use crate::middleware::AuthUser;
use crate::repositories::NotificationRepo;
use crate::state::AppState;
use crate::templates::{NotificationItemPartial, NotificationsTemplate};
use askama::Template;
use axum::extract::{Path, State};
use axum::response::Html;
use std::sync::Arc;
use uuid::Uuid;

pub async fn list(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<NotificationsTemplate> {
    let notifications = NotificationRepo::list_for_user(&state.db, user.id, 50, 0, false).await?;
    let unread = unread_count(&state, user.id).await;
    Ok(NotificationsTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        notifications,
    })
}

pub async fn mark_read(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Html<String>> {
    NotificationRepo::mark_read(&state.db, id, user.id).await?;
    let n = NotificationRepo::list_for_user(&state.db, user.id, 50, 0, false).await?;
    let mut html = String::new();
    for n in &n {
        let partial = NotificationItemPartial { notification: n };
        html.push_str(&partial.render()?);
    }
    Ok(Html(html))
}

pub async fn mark_all_read(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<Html<String>> {
    NotificationRepo::mark_all_read(&state.db, user.id).await?;
    let n = NotificationRepo::list_for_user(&state.db, user.id, 50, 0, false).await?;
    let mut html = String::new();
    for n in &n {
        let partial = NotificationItemPartial { notification: n };
        html.push_str(&partial.render()?);
    }
    Ok(Html(html))
}
