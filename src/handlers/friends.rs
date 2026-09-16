//! Handlers Kết bạn (v3.14.0).
//!
//! - GET /friends — trang bạn bè (danh sách + lời mời đến/đi).
//! - POST /friends/request/{username} — gửi lời mời (tự chấp nhận nếu
//!   đối phương đã mời mình trước).
//! - POST /friends/respond/{id} — chấp nhận/từ chối (form `accept=1`).
//! - POST /friends/cancel/{id} — hủy lời mời đã gửi.
//! - POST /friends/unfriend/{username} — hủy kết bạn.
//! - POST /friends/block/{username} + /friends/unblock/{username}.
//!
//! Tất cả POST dùng PRG (303 redirect về /friends) — đơn giản, robust,
//! không phụ thuộc JS. Rate-limit chống spam mời.
//!
//! Quy tắc:
//! - Không tự kết bạn, không kết bạn với tài khoản AI Agent (bot).
//! - Target bị ban → 404 (coi như không tồn tại).

use crate::error::{AppError, AppResult};
use crate::handlers::auth::unread_count;
use crate::middleware::AuthUser;
use crate::repositories::{FriendRepo, NotificationRepo, UserRepo};
use crate::state::AppState;
use crate::templates::FriendsTemplate;
use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

/// GET /friends — trang quản lý bạn bè.
pub async fn friends_page(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<FriendsTemplate> {
    let (friends_res, incoming_res, outgoing_res, unread_res) = tokio::join!(
        FriendRepo::list_friends(&state.db, user.id, None, 100, 0),
        FriendRepo::list_incoming(&state.db, user.id),
        FriendRepo::list_outgoing(&state.db, user.id),
        unread_count(&state, user.id),
    );
    Ok(FriendsTemplate {
        unread_notifications: unread_res,
        friends: friends_res?,
        incoming: incoming_res?,
        outgoing: outgoing_res?,
        current_user: Some(user),
    })
}

/// Resolve target user cho các action theo username (chặn self/AI/banned).
async fn resolve_target(
    state: &AppState,
    me: &crate::models::User,
    username: &str,
) -> AppResult<crate::models::User> {
    let target = UserRepo::find_by_username(&state.db, username)
        .await?
        .ok_or_else(|| AppError::NotFound("Người dùng không tồn tại".into()))?;
    if target.id == me.id {
        return Err(AppError::BadRequest(
            "Bạn không thể kết bạn với chính mình".into(),
        ));
    }
    if target.is_banned {
        return Err(AppError::NotFound("Người dùng không tồn tại".into()));
    }
    if target.is_ai_agent_user() {
        return Err(AppError::BadRequest(
            "Tài khoản AI Agent là bot — không thể kết bạn".into(),
        ));
    }
    Ok(target)
}

/// POST /friends/request/{username} — gửi lời mời.
/// Nếu đối phương ĐÃ mời mình (incoming pending) → chấp nhận luôn thay vì
/// tạo lời mời ngược (tránh 2 row chờ nhau).
pub async fn send_request(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(username): Path<String>,
) -> AppResult<Redirect> {
    // Rate-limit: tối đa 20 lời mời/giờ (chống spam mời hàng loạt).
    if !state
        .rate_limiter
        .check(&format!("friend-req:{}", user.id), 20, 3600)
    {
        return Err(AppError::BadRequest(
            "Bạn gửi quá nhiều lời mời — thử lại sau 1 giờ".into(),
        ));
    }
    let target = resolve_target(&state, &user, &username).await?;
    match FriendRepo::between(&state.db, user.id, target.id).await? {
        None => {
            FriendRepo::request(&state.db, user.id, target.id).await?;
            let db = state.db.clone();
            let (uid, aid, aname) = (target.id, user.id, user.display_name.clone());
            tokio::spawn(async move {
                let _ = NotificationRepo::create_friend_request(&db, uid, aid, &aname).await;
            });
        }
        Some(rel) => {
            if rel.status == "accepted" {
                return Err(AppError::BadRequest("Hai bạn đã là bạn bè".into()));
            }
            if rel.status == "blocked" {
                return Err(AppError::BadRequest("Không thể kết bạn lúc này".into()));
            }
            if rel.status == "pending" {
                if rel.requester_id == user.id {
                    return Err(AppError::BadRequest(
                        "Bạn đã gửi lời mời rồi — chờ đối phương phản hồi".into(),
                    ));
                }
                // Đối phương mời mình trước → chấp nhận luôn.
                FriendRepo::respond(&state.db, rel.id, user.id, true).await?;
            } else {
                // declined cũ → gửi lại bằng resend() (xóa mọi row của cặp
                // rồi tạo mới — giữ invariant 1 row/cặp, HIGH-1).
                FriendRepo::resend(&state.db, user.id, target.id).await?;
                let db = state.db.clone();
                let (uid, aid, aname) = (target.id, user.id, user.display_name.clone());
                tokio::spawn(async move {
                    let _ = NotificationRepo::create_friend_request(&db, uid, aid, &aname).await;
                });
            }
        }
    }
    Ok(Redirect::to("/friends"))
}

#[derive(Debug, Deserialize)]
pub struct RespondForm {
    /// `accept=1` → chấp nhận, thiếu/khác → từ chối.
    pub accept: Option<String>,
}

/// POST /friends/respond/{id} — chấp nhận/từ chối lời mời đến.
pub async fn respond(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Form(form): Form<RespondForm>,
) -> AppResult<Redirect> {
    let accept = form.accept.as_deref() == Some("1");
    if !FriendRepo::respond(&state.db, id, user.id, accept).await? {
        return Err(AppError::BadRequest(
            "Lời mời không tồn tại hoặc đã được xử lý".into(),
        ));
    }
    Ok(Redirect::to("/friends"))
}

/// POST /friends/cancel/{id} — hủy lời mời mình đã gửi.
pub async fn cancel(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Redirect> {
    if !FriendRepo::cancel(&state.db, id, user.id).await? {
        return Err(AppError::BadRequest(
            "Lời mời không tồn tại hoặc đã được xử lý".into(),
        ));
    }
    Ok(Redirect::to("/friends"))
}

/// POST /friends/unfriend/{username} — hủy kết bạn.
pub async fn unfriend(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(username): Path<String>,
) -> AppResult<Redirect> {
    let target = UserRepo::find_by_username(&state.db, &username)
        .await?
        .ok_or_else(|| AppError::NotFound("Người dùng không tồn tại".into()))?;
    if !FriendRepo::unfriend(&state.db, user.id, target.id).await? {
        return Err(AppError::BadRequest("Hai bạn chưa phải bạn bè".into()));
    }
    Ok(Redirect::to("/friends"))
}

/// POST /friends/block/{username} — chặn (xóa mọi quan hệ cũ).
pub async fn block(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(username): Path<String>,
) -> AppResult<Redirect> {
    let target = UserRepo::find_by_username(&state.db, &username)
        .await?
        .ok_or_else(|| AppError::NotFound("Người dùng không tồn tại".into()))?;
    if target.id == user.id {
        return Err(AppError::BadRequest("Bạn không thể chặn chính mình".into()));
    }
    FriendRepo::block(&state.db, user.id, target.id).await?;
    Ok(Redirect::to("/friends"))
}

/// POST /friends/unblock/{username} — bỏ chặn.
pub async fn unblock(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(username): Path<String>,
) -> AppResult<Redirect> {
    let target = UserRepo::find_by_username(&state.db, &username)
        .await?
        .ok_or_else(|| AppError::NotFound("Người dùng không tồn tại".into()))?;
    if !FriendRepo::unblock(&state.db, user.id, target.id).await? {
        return Err(AppError::BadRequest("Bạn chưa chặn người này".into()));
    }
    Ok(Redirect::to("/friends"))
}
