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

/// Đánh dấu 1 thông báo đã đọc và trả về đúng item đó
/// (trước đây trả về toàn bộ danh sách → swap outerHTML nhân đôi danh sách)
pub async fn mark_read(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Html<String>> {
    NotificationRepo::mark_read(&state.db, id, user.id).await?;
    // Fetch đúng 1 item đã cập nhật (trước đây load 200 dòng rồi find —
    // query nặng không cần thiết mỗi lần click một notification).
    match NotificationRepo::find_for_user(&state.db, id, user.id).await? {
        Some(n) => {
            let partial = NotificationItemPartial { notification: &n };
            Ok(Html(partial.render()?))
        }
        None => Ok(Html(String::new())),
    }
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
