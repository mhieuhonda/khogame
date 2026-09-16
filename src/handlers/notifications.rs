use crate::error::AppResult;
use crate::handlers::auth::unread_count;
use crate::middleware::AuthUser;
use crate::repositories::NotificationRepo;
use crate::state::AppState;
use crate::templates::{NotificationItemPartial, NotificationsTemplate};
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::Html;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Deserialize, Default)]
pub struct NotificationsQuery {
    pub page: Option<i64>,
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn list(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Query(q): Query<NotificationsQuery>,
) -> AppResult<NotificationsTemplate> {
    let page = q.page.unwrap_or(1).clamp(1, 10_000);
    let per_page: i64 = 50;
    // FIX v2.8.1: saturating math — page ~4e17 làm (page-1)*per_page
    // tràn i64 → OFFSET âm → 500 (prod) / panic (debug).
    let offset = page.saturating_sub(1).saturating_mul(per_page);
    let notifications =
        NotificationRepo::list_for_user(&state.db, user.id, per_page, offset, false).await?;
    let total = NotificationRepo::count_for_user(&state.db, user.id)
        .await
        .unwrap_or(0);
    let unread = unread_count(&state, user.id).await;
    Ok(NotificationsTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        notifications,
        page,
        per_page,
        total,
    })
}

/// Đánh dấu 1 thông báo đã đọc và trả về đúng item đó
/// (trước đây trả về toàn bộ danh sách → swap outerHTML nhân đôi danh sách)
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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

// ============================================================
// v3.16.0 — Toast realtime huy hiệu/lên cấp
// ============================================================

/// GET /notifications/toasts — JSON tối đa 5 toast huy hiệu/lên cấp chưa
/// báo + đánh dấu announced atomic (không trùng, đa tab OK).
/// Client (app.js) poll 30s khi đã đăng nhập, render toast góc màn hình.
/// Không đánh dấu is_read (badge /notifications giữ nguyên).
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn toasts(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<axum::Json<Vec<crate::models::ToastItem>>> {
    // Rate-limit: poll 30s/tab = 2/phút — cap 20/phút chống tab spam.
    if !state
        .rate_limiter
        .check(&format!("toasts:{}", user.id), 20, 60)
    {
        return Ok(axum::Json(Vec::new()));
    }
    let items = NotificationRepo::take_toasts(&state.db, user.id).await?;
    Ok(axum::Json(items))
}
