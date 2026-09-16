//! Handlers Chat riêng + Nhóm chat (v3.14.0).
//!
//! - GET /messages — inbox (DM + nhóm, preview + unread, 1 query).
//! - GET /messages/dm/{username} — thread DM (không tự tạo hội thoại).
//! - POST /messages/dm/{username}/start — tạo/mở DM (phải là bạn bè).
//! - GET /messages/dm/{username}/box — partial tin nhắn (HTMX poll 3s).
//! - POST /messages/dm/{username}/send — gửi tin DM.
//! - POST /groups/create — tạo nhóm (tên + bạn bè).
//! - GET /messages/group/{id} + /box — thread nhóm.
//! - POST /messages/group/{id}/send — gửi tin nhóm.
//! - POST /messages/group/{id}/{leave,add,remove,rename,delete}.
//! - POST /dm/messages/{id}/delete — xóa tin (chính chủ hoặc staff).
//!
//! Realtime = HTMX poll partial 3s (riêng tư: không broadcast WS chung,
//! không leak nội dung cho client khác). Hiệu năng: thread LIMIT 30 +
//! index (conversation_id, created_at DESC); inbox 1 query; unread badge
//! 1 COUNT.
//!
//! Giới hạn ký tự: member thường 500 (như live chat), admin + member được
//! cấp `chat_unlimited` → 20000 (hard cap chống phình DB). Vượt → cắt
//! (giống live chat WS) để UX mượt.

use crate::error::{AppError, AppResult};
use crate::handlers::auth::unread_count;
use crate::middleware::AuthUser;
use crate::repositories::dm::THREAD_LIMIT;
use crate::repositories::{DmRepo, FriendRepo, NotificationRepo, UserRepo, MAX_GROUP_MEMBERS};
use crate::state::AppState;
use crate::templates::{DmBoxTemplate, DmMessageTemplate, DmThreadTemplate, InboxTemplate};
use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, Redirect};
use axum::Form;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

/// Giới hạn ký tự mặc định cho member thường (đồng nhất live chat).
pub const DM_DEFAULT_MAX: usize = 500;
/// Hard cap cho cả unlimited (chống phình DB/memory).
pub const DM_ABSOLUTE_MAX: usize = 20_000;

/// Giới hạn áp dụng cho user (admin + được cấp → absolute).
fn limit_for(user: &crate::models::User) -> usize {
    if user.can_chat_unlimited() {
        DM_ABSOLUTE_MAX
    } else {
        DM_DEFAULT_MAX
    }
}

/// Cắt content về limit (chars, unicode-safe) — giống live chat WS.
fn clamp_content(content: &str, limit: usize) -> String {
    let trimmed = content.trim();
    if trimmed.chars().count() > limit {
        trimmed.chars().take(limit).collect()
    } else {
        trimmed.to_string()
    }
}

/// Validate image_url đính kèm: chỉ nhận file do chính server upload
/// (`/uploads/...` — chặn URL ngoài gây XSS/phishing qua ảnh).
/// v3.16.0 FIX (F2): dùng chung `storage::is_upload_url` thay vì check
/// prefix thủ công — thêm guard `..`/CRLF (path traversal/header injection
/// qua thuộc tính src).
fn validate_image_url(url: Option<String>) -> AppResult<Option<String>> {
    match url {
        None => Ok(None),
        Some(u) => {
            let t = u.trim().to_string();
            if t.is_empty() {
                return Ok(None);
            }
            if !(crate::services::storage::is_upload_url(&t) && t.len() <= 300) {
                return Err(AppError::BadRequest("Ảnh đính kèm không hợp lệ".into()));
            }
            Ok(Some(t))
        }
    }
}

// ============================================================
// Badge tin chưa đọc (layout poll)
// ============================================================

/// GET /messages/unread-badge — partial số tin chưa đọc cho menu layout
/// (HTMX poll, không cần thêm field vào mọi template).
pub async fn unread_badge(
    State(state): State<Arc<AppState>>,
    crate::middleware::CurrentUser(user): crate::middleware::CurrentUser,
) -> Html<String> {
    let n = match user {
        Some(u) => DmRepo::total_unread(&state.db, u.id).await.unwrap_or(0),
        None => 0,
    };
    if n > 0 {
        Html(format!(r#"<span class="menu-badge">{n}</span>"#))
    } else {
        Html(String::new())
    }
}

// ============================================================
// Inbox
// ============================================================

/// GET /messages — hộp thư (DM + nhóm).
pub async fn inbox_page(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<InboxTemplate> {
    let (items_res, total_res, incoming_res, unread_res) = tokio::join!(
        DmRepo::inbox(&state.db, user.id),
        DmRepo::total_unread(&state.db, user.id),
        FriendRepo::count_incoming(&state.db, user.id),
        unread_count(&state, user.id),
    );
    Ok(InboxTemplate {
        unread_notifications: unread_res,
        items: items_res?,
        total_unread: total_res?,
        incoming_requests: incoming_res?,
        current_user: Some(user),
    })
}

// ============================================================
// DM 1-1
// ============================================================

/// Resolve đối phương + kiểm tra quyền GỬI DM.
/// Chính sách v3.16.0 (MED-4/5, IDOR-3/4):
/// - Lịch sử cũ được ĐỌC tự do khi còn membership (xem `resolve_dm_peer`);
///   riêng GỬI tin mới yêu cầu bạn bè.
/// - Block chặn TẤT CẢ mọi role (kể cả admin — block là ranh giới tuyệt
///   đối, không có DM cưỡng bức một chiều).
/// - Chỉ ADMIN được bypass yêu cầu bạn bè (mod/user thường không).
async fn resolve_dm_target(
    state: &AppState,
    me: &crate::models::User,
    username: &str,
) -> AppResult<crate::models::User> {
    let other = UserRepo::find_by_username(&state.db, username)
        .await?
        .ok_or_else(|| AppError::NotFound("Người dùng không tồn tại".into()))?;
    if other.id == me.id {
        return Err(AppError::BadRequest(
            "Dùng Ghi chú cá nhân? Tính năng tự chat với mình chưa hỗ trợ".into(),
        ));
    }
    if other.is_banned {
        return Err(AppError::NotFound("Người dùng không tồn tại".into()));
    }
    // Block hai chiều chặn tất cả (kể cả admin).
    if let Some(rel) = FriendRepo::between(&state.db, me.id, other.id).await? {
        if rel.status == "blocked" {
            return Err(AppError::Forbidden("Không thể nhắn tin lúc này".into()));
        }
    }
    if !me.role.is_admin() {
        if other.is_ai_agent_user() {
            return Err(AppError::BadRequest(
                "Tài khoản AI Agent là bot — không thể nhắn riêng".into(),
            ));
        }
        if !FriendRepo::are_friends(&state.db, me.id, other.id).await? {
            return Err(AppError::Forbidden(
                "Chỉ nhắn riêng được với bạn bè — hãy kết bạn trước".into(),
            ));
        }
    }
    Ok(other)
}

/// Resolve đối phương để ĐỌC lịch sử (thread/box): chỉ cần từng là
/// member — unfriend/block sau này không xóa quyền xem lại tin cũ của
/// chính mình (MED-4: hết inbox ma + thread 403 oan).
async fn resolve_dm_peer(
    state: &AppState,
    me: &crate::models::User,
    username: &str,
) -> AppResult<crate::models::User> {
    let other = UserRepo::find_by_username(&state.db, username)
        .await?
        .ok_or_else(|| AppError::NotFound("Người dùng không tồn tại".into()))?;
    if other.id == me.id {
        return Err(AppError::BadRequest(
            "Dùng Ghi chú cá nhân? Tính năng tự chat với mình chưa hỗ trợ".into(),
        ));
    }
    if other.is_banned {
        return Err(AppError::NotFound("Người dùng không tồn tại".into()));
    }
    Ok(other)
}

/// GET /messages/dm/{username} — thread DM (chưa có hội thoại → màn hình
/// "bắt đầu trò chuyện", không tự tạo row rác).
pub async fn dm_thread_page(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(username): Path<String>,
) -> AppResult<DmThreadTemplate> {
    // Đọc lịch sử: chỉ cần từng là member (MED-4).
    let other = resolve_dm_peer(&state, &user, &username).await?;
    let conv = DmRepo::find_dm(&state.db, user.id, other.id).await?;
    let limit = limit_for(&user);
    let unread = unread_count(&state, user.id).await;
    match conv {
        None => Ok(DmThreadTemplate::not_started(
            Some(user),
            unread,
            other,
            limit,
        )),
        Some(c) => {
            // Chắc chắn mình còn là member (rời DM cũ rồi vào lại).
            if DmRepo::member_role(&state.db, c.id, user.id)
                .await?
                .is_none()
            {
                return Ok(DmThreadTemplate::not_started(
                    Some(user),
                    unread,
                    other,
                    limit,
                ));
            }
            let mut messages = DmRepo::thread(&state.db, c.id, THREAD_LIMIT).await?;
            messages.reverse();
            // v3.16.0 (LOW-10): chốt read theo mốc đã fetch, không NOW().
            let upto = messages.iter().map(|m| m.created_at).max();
            DmRepo::mark_read(&state.db, c.id, user.id, upto).await?;
            Ok(DmThreadTemplate::dm(
                Some(user),
                unread,
                c,
                other,
                messages,
                limit,
            ))
        }
    }
}

/// POST /messages/dm/{username}/start — tạo (hoặc mở lại) DM rồi redirect.
pub async fn dm_start(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(username): Path<String>,
) -> AppResult<Redirect> {
    let other = resolve_dm_target(&state, &user, &username).await?;
    // Audit khi admin bypass friendship (mở DM với người lạ): hành động
    // cưỡng bức tiềm năng phải để lại dấu vết (IDOR-3).
    if user.role.is_admin() && !FriendRepo::are_friends(&state.db, user.id, other.id).await? {
        crate::services::audit::audit(
            &state,
            user.id,
            "dm.force_start",
            "user",
            &other.id.to_string(),
            &format!(
                "admin {} mở DM với người lạ {}",
                user.username, other.username
            ),
        )
        .await;
    }
    DmRepo::get_or_create_dm(&state.db, user.id, other.id).await?;
    Ok(Redirect::to(&format!("/messages/dm/{}", other.username)))
}

/// GET /messages/dm/{username}/box — partial tin nhắn (HTMX poll `every 3s`).
/// Không mark_read ở đây (tránh UPDATE mỗi poll khi tab chạy nền) —
/// read được đánh dấu khi mở trang thread.
pub async fn dm_box(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(username): Path<String>,
) -> AppResult<Html<String>> {
    // Poll đọc: membership-only (MED-4).
    let other = resolve_dm_peer(&state, &user, &username).await?;
    let conv = DmRepo::find_dm(&state.db, user.id, other.id).await?;
    let messages = match conv {
        Some(c)
            if DmRepo::member_role(&state.db, c.id, user.id)
                .await?
                .is_some() =>
        {
            let mut m = DmRepo::thread(&state.db, c.id, THREAD_LIMIT).await?;
            m.reverse();
            m
        }
        _ => Vec::new(),
    };
    Ok(Html(
        DmBoxTemplate {
            messages,
            me: user.id,
        }
        .render()?,
    ))
}

#[derive(Debug, Deserialize)]
pub struct SendForm {
    pub content: Option<String>,
    pub image_url: Option<String>,
}

/// POST /messages/dm/{username}/send — gửi tin DM (HTMX → append partial).
pub async fn dm_send(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(username): Path<String>,
    Form(form): Form<SendForm>,
) -> AppResult<Html<String>> {
    // Rate-limit: 30 tin/phút (đồng nhất live chat công cộng).
    if !state.rate_limiter.check(&format!("dm:{}", user.id), 30, 60) {
        return Err(AppError::BadRequest(
            "Bạn nhắn quá nhanh — nghỉ vài giây rồi gửi tiếp".into(),
        ));
    }
    let other = resolve_dm_target(&state, &user, &username).await?;
    let conv = DmRepo::get_or_create_dm(&state.db, user.id, other.id).await?;
    let limit = limit_for(&user);
    let content = clamp_content(&form.content.unwrap_or_default(), limit);
    let image_url = validate_image_url(form.image_url)?;
    if content.is_empty() && image_url.is_none() {
        return Err(AppError::BadRequest("Tin nhắn trống".into()));
    }
    let msg = DmRepo::send(&state.db, conv.id, user.id, &content, image_url.as_deref()).await?;
    // Báo cho đối phương (best-effort, 1 query).
    let db = state.db.clone();
    let (conv_id, sender, name, link) = (
        conv.id,
        user.id,
        user.display_name.clone(),
        format!("/messages/dm/{}", other.username),
    );
    tokio::spawn(async move {
        let title = format!("{name} đã nhắn tin cho bạn");
        let _ = NotificationRepo::create_dm_batch(&db, conv_id, sender, &title, &link).await;
    });
    Ok(Html(
        DmMessageTemplate {
            message: msg,
            me: user.id,
        }
        .render()?,
    ))
}

// ============================================================
// Nhóm chat
// ============================================================

#[derive(Debug, Deserialize)]
pub struct GroupCreateForm {
    pub name: String,
    /// Username bạn bè, phân tách phẩy (từ form tạo nhóm).
    pub members: Option<String>,
}

/// POST /groups/create — tạo nhóm (tên 2-100 ký tự, ≥1 bạn).
pub async fn group_create(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Form(form): Form<GroupCreateForm>,
) -> AppResult<Redirect> {
    let name = form.name.trim();
    if name.chars().count() < 2 || name.chars().count() > 100 {
        return Err(AppError::BadRequest("Tên nhóm 2–100 ký tự".into()));
    }
    // Parse danh sách username → resolve + chỉ giữ BẠN BÈ (chống add người lạ).
    // v3.16.0 FIX (HIGH-2): giới hạn đúng MAX_GROUP_MEMBERS (50 cả nhóm —
    // creator chiếm 1 slot nên others ≤ 49). Trước đây take(60) cho nhóm
    // 61 người, vượt cap mà group_add lại chặn → luật không nhất quán.
    let mut member_ids = Vec::new();
    if let Some(raw) = form.members.as_deref() {
        for uname in raw
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .take(MAX_GROUP_MEMBERS as usize)
        {
            if member_ids.len() >= (MAX_GROUP_MEMBERS as usize) - 1 {
                break;
            }
            if let Some(u) = UserRepo::find_by_username(&state.db, uname).await? {
                if u.id != user.id
                    && !u.is_banned
                    && !u.is_ai_agent_user()
                    && FriendRepo::are_friends(&state.db, user.id, u.id).await?
                    && !member_ids.contains(&u.id)
                {
                    member_ids.push(u.id);
                }
            }
        }
    }
    if member_ids.is_empty() {
        return Err(AppError::BadRequest(
            "Nhóm cần ít nhất 1 bạn bè tham gia".into(),
        ));
    }
    let conv = DmRepo::create_group(&state.db, user.id, name, &member_ids).await?;
    Ok(Redirect::to(&format!("/messages/group/{}", conv.id)))
}

/// Lấy hội thoại nhóm + check membership (None nếu không phải nhóm).
async fn resolve_group(
    state: &AppState,
    user: &crate::models::User,
    id: Uuid,
) -> AppResult<(crate::models::Conversation, String)> {
    let conv = DmRepo::find_conversation(&state.db, id)
        .await?
        .filter(|c| c.is_group())
        .ok_or_else(|| AppError::NotFound("Nhóm không tồn tại".into()))?;
    let role = DmRepo::member_role(&state.db, id, user.id)
        .await?
        .ok_or_else(|| AppError::Forbidden("Bạn không ở trong nhóm này".into()))?;
    Ok((conv, role))
}

/// GET /messages/group/{id} — thread nhóm.
pub async fn group_thread_page(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<DmThreadTemplate> {
    let (conv, role) = resolve_group(&state, &user, id).await?;
    let (messages, members, unread) = tokio::join!(
        DmRepo::thread(&state.db, conv.id, THREAD_LIMIT),
        DmRepo::members(&state.db, conv.id),
        unread_count(&state, user.id),
    );
    let mut messages = messages?;
    messages.reverse();
    // v3.16.0 (LOW-10): chốt read theo mốc đã fetch, không NOW().
    let upto = messages.iter().map(|m| m.created_at).max();
    DmRepo::mark_read(&state.db, conv.id, user.id, upto).await?;
    let limit = limit_for(&user);
    Ok(DmThreadTemplate::group(
        Some(user),
        unread,
        conv,
        members?,
        messages,
        limit,
        role,
    ))
}

/// GET /messages/group/{id}/box — partial poll nhóm.
pub async fn group_box(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Html<String>> {
    let (conv, _) = resolve_group(&state, &user, id).await?;
    let mut messages = DmRepo::thread(&state.db, conv.id, THREAD_LIMIT).await?;
    messages.reverse();
    Ok(Html(
        DmBoxTemplate {
            messages,
            me: user.id,
        }
        .render()?,
    ))
}

/// POST /messages/group/{id}/send — gửi tin nhóm (HTMX → append partial).
pub async fn group_send(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Form(form): Form<SendForm>,
) -> AppResult<Html<String>> {
    if !state.rate_limiter.check(&format!("dm:{}", user.id), 30, 60) {
        return Err(AppError::BadRequest(
            "Bạn nhắn quá nhanh — nghỉ vài giây rồi gửi tiếp".into(),
        ));
    }
    let (conv, _) = resolve_group(&state, &user, id).await?;
    let limit = limit_for(&user);
    let content = clamp_content(&form.content.unwrap_or_default(), limit);
    let image_url = validate_image_url(form.image_url)?;
    if content.is_empty() && image_url.is_none() {
        return Err(AppError::BadRequest("Tin nhắn trống".into()));
    }
    let msg = DmRepo::send(&state.db, conv.id, user.id, &content, image_url.as_deref()).await?;
    let db = state.db.clone();
    let (conv_id, sender, name, link) = (
        conv.id,
        user.id,
        user.display_name.clone(),
        format!("/messages/group/{}", conv.id),
    );
    tokio::spawn(async move {
        let title = format!("{name} đã nhắn trong nhóm");
        let _ = NotificationRepo::create_dm_batch(&db, conv_id, sender, &title, &link).await;
    });
    Ok(Html(
        DmMessageTemplate {
            message: msg,
            me: user.id,
        }
        .render()?,
    ))
}

#[derive(Debug, Deserialize)]
pub struct GroupMembersForm {
    /// Username bạn bè, phân tách phẩy.
    pub usernames: String,
}

/// POST /messages/group/{id}/add — thêm bạn vào nhóm (owner/admin).
pub async fn group_add(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Form(form): Form<GroupMembersForm>,
) -> AppResult<Redirect> {
    let (conv, role) = resolve_group(&state, &user, id).await?;
    if role != "owner" && role != "admin" {
        return Err(AppError::Forbidden(
            "Chỉ trưởng/phó nhóm mới thêm thành viên".into(),
        ));
    }
    let current = DmRepo::member_count(&state.db, conv.id).await?;
    let mut ids = Vec::new();
    for uname in form
        .usernames
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .take(20)
    {
        if current + ids.len() as i64 >= MAX_GROUP_MEMBERS {
            break;
        }
        if let Some(u) = UserRepo::find_by_username(&state.db, uname).await? {
            // v3.16.0 FIX (MED-5/IDOR-4): người được mời PHẢI là bạn bè của
            // người mời — không có bypass staff (kể cả admin): nhét người lạ
            // vào nhóm kín = lộ nội dung nhóm + ép họ vào cuộc trò chuyện.
            if u.id != user.id
                && !u.is_banned
                && !u.is_ai_agent_user()
                && FriendRepo::are_friends(&state.db, user.id, u.id).await?
            {
                ids.push(u.id);
            }
        }
    }
    if ids.is_empty() {
        return Err(AppError::BadRequest(
            "Không thêm được ai (phải là bạn bè, chưa trong nhóm, nhóm chưa đầy)".into(),
        ));
    }
    DmRepo::add_members(&state.db, conv.id, &ids).await?;
    Ok(Redirect::to(&format!("/messages/group/{}", conv.id)))
}

#[derive(Debug, Deserialize)]
pub struct GroupRemoveForm {
    pub user_id: Uuid,
}

/// POST /messages/group/{id}/remove — xóa thành viên (owner xóa được
/// admin/member; admin chỉ xóa được member).
pub async fn group_remove(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Form(form): Form<GroupRemoveForm>,
) -> AppResult<Redirect> {
    let (conv, role) = resolve_group(&state, &user, id).await?;
    let target_role = DmRepo::member_role(&state.db, conv.id, form.user_id).await?;
    let allowed = matches!(
        (role.as_str(), target_role.as_deref()),
        ("owner", Some("member" | "admin")) | ("admin", Some("member"))
    );
    if !allowed {
        return Err(AppError::Forbidden(
            "Bạn không có quyền xóa thành viên này".into(),
        ));
    }
    DmRepo::remove_member(&state.db, conv.id, form.user_id).await?;
    Ok(Redirect::to(&format!("/messages/group/{}", conv.id)))
}

/// POST /messages/group/{id}/leave — rời nhóm. Owner rời khi còn người
/// khác → từ chối (phải xóa nhóm); owner cuối cùng rời → xóa luôn nhóm.
pub async fn group_leave(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Redirect> {
    let (conv, role) = resolve_group(&state, &user, id).await?;
    let count = DmRepo::member_count(&state.db, conv.id).await?;
    if role == "owner" && count > 1 {
        return Err(AppError::BadRequest(
            "Trưởng nhóm không thể rời khi còn thành viên — hãy xóa nhóm".into(),
        ));
    }
    DmRepo::leave(&state.db, conv.id, user.id).await?;
    if role == "owner" {
        DmRepo::delete_group(&state.db, conv.id).await?;
    }
    Ok(Redirect::to("/messages"))
}

#[derive(Debug, Deserialize)]
pub struct GroupRenameForm {
    pub name: String,
}

/// POST /messages/group/{id}/rename — đổi tên nhóm (owner/admin).
pub async fn group_rename(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Form(form): Form<GroupRenameForm>,
) -> AppResult<Redirect> {
    let (conv, role) = resolve_group(&state, &user, id).await?;
    if role != "owner" && role != "admin" {
        return Err(AppError::Forbidden(
            "Chỉ trưởng/phó nhóm mới đổi tên".into(),
        ));
    }
    let name = form.name.trim();
    if name.chars().count() < 2 || name.chars().count() > 100 {
        return Err(AppError::BadRequest("Tên nhóm 2–100 ký tự".into()));
    }
    DmRepo::rename_group(&state.db, conv.id, name).await?;
    Ok(Redirect::to(&format!("/messages/group/{}", conv.id)))
}

/// POST /messages/group/{id}/delete — xóa nhóm (chỉ owner).
pub async fn group_delete(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Redirect> {
    let (conv, role) = resolve_group(&state, &user, id).await?;
    if role != "owner" {
        return Err(AppError::Forbidden("Chỉ trưởng nhóm mới xóa nhóm".into()));
    }
    DmRepo::delete_group(&state.db, conv.id).await?;
    Ok(Redirect::to("/messages"))
}

// ============================================================
// Xóa tin nhắn riêng
// ============================================================

/// POST /dm/messages/{id}/delete — xóa tin của chính mình (staff xóa mọi tin,
/// kể cả nhóm mình không tham gia — kiểm duyệt report nhóm kín, MED-7).
pub async fn delete_message(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Redirect> {
    // Tìm tin để biết conversation (check membership) — 1 query PK.
    let row: Option<(Uuid, Uuid)> = sqlx::query_as(
        r"SELECT conversation_id, sender_id FROM dm_messages WHERE id = $1 AND is_deleted = FALSE",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let (conv_id, sender_id) =
        row.ok_or_else(|| AppError::NotFound("Tin nhắn không tồn tại".into()))?;
    // v3.16.0 FIX (MED-7): staff kiểm duyệt bypass membership (nhận report
    // nhóm kín vẫn xử lý được); member thường vẫn phải ở trong hội thoại.
    let is_staff = user.role.is_staff();
    if !is_staff
        && DmRepo::member_role(&state.db, conv_id, user.id)
            .await?
            .is_none()
    {
        return Err(AppError::Forbidden(
            "Bạn không ở trong hội thoại này".into(),
        ));
    }
    let scope = if is_staff { None } else { Some(user.id) };
    if !DmRepo::soft_delete(&state.db, id, scope).await? {
        return Err(AppError::Forbidden(
            "Bạn chỉ xóa được tin của chính mình".into(),
        ));
    }
    // Quay lại thread (DM cần username đối phương — lấy nhanh).
    // Staff kiểm duyệt từ NGOÀI hội thoại → về inbox (vào thread sẽ 403).
    let own_member = DmRepo::member_role(&state.db, conv_id, user.id)
        .await?
        .is_some();
    if (sender_id == user.id || user.role.is_staff()) && own_member {
        let conv = DmRepo::find_conversation(&state.db, conv_id).await?;
        if let Some(c) = conv {
            if c.is_group() {
                return Ok(Redirect::to(&format!("/messages/group/{conv_id}")));
            }
            if let Some(other) = DmRepo::dm_other(&state.db, conv_id, user.id).await? {
                return Ok(Redirect::to(&format!("/messages/dm/{}", other.username)));
            }
        }
    }
    Ok(Redirect::to("/messages"))
}
