//! Handlers cho hệ thống góp ý người dùng (v3.4.0).
//!
//! Bao gồm:
//! - [`feedback_page`]: GET /feedback — form gửi góp ý + danh sách góp ý
//!   của user kèm phản hồi admin.
//! - [`submit_feedback`]: POST /feedback — tạo góp ý mới (rate-limit 10/ngày).
//! - [`admin_feedback_page`]: GET /admin/feedback — danh sách + lọc trạng thái.
//! - [`admin_feedback_update`]: POST /admin/feedback/{id}/status — đổi trạng
//!   thái + gửi phản hồi tới người gửi (notification).

use crate::error::{AppError, AppResult};
use crate::handlers::auth::unread_count;
use crate::middleware::AuthUser;
use crate::models::feedback::{FeedbackCategory, FeedbackStatus};
use crate::repositories::FeedbackRepo;
use crate::state::AppState;
use crate::templates::{AdminFeedbackTemplate, FeedbackTemplate};
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

/// Query params cho trang feedback (admin filter / user success flash).
#[derive(Debug, Deserialize, Default)]
pub struct FeedbackQuery {
    pub status: Option<String>,
    /// "1" sau khi submit thành công — hiện banner cảm ơn.
    pub sent: Option<String>,
}

// ============================================================
// USER-FACING: /feedback
// ============================================================

/// Trang góp ý — form + "góp ý của tôi".
///
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn feedback_page(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Query(q): Query<FeedbackQuery>,
) -> AppResult<FeedbackTemplate> {
    let just_sent = q.sent.as_deref() == Some("1");
    let (mine, unread) = tokio::join!(
        FeedbackRepo::list_by_user(&state.db, user.id, 20),
        unread_count(&state, user.id)
    );
    Ok(FeedbackTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        categories: FeedbackCategory::all().to_vec(),
        my_feedback: mine.unwrap_or_default(),
        just_sent,
    })
}

#[derive(Debug, Deserialize)]
pub struct FeedbackForm {
    pub category: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub page_url: String,
}

/// POST /feedback — tạo góp ý mới.
///
/// Rate-limit: tối đa 10 góp ý / 24 giờ / user (chống spam).
/// Validate: category whitelist, title 5-200 ký tự, body 10-5000 ký tự,
/// page_url tối đa 2048 ký tự (nếu có thì phải bắt đầu bằng "/" — cùng site).
///
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn submit_feedback(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Form(form): Form<FeedbackForm>,
) -> AppResult<Response> {
    // 1) Category whitelist
    let category = FeedbackCategory::from_str(&form.category).ok_or_else(|| {
        AppError::BadRequest("Loại góp ý không hợp lệ. Chọn lại từ danh sách.".into())
    })?;

    // 2) Title: 5-200 ký tự
    let title = form.title.trim();
    let title_len = title.chars().count();
    if title_len < 5 {
        return Err(AppError::BadRequest(
            "Tiêu đề tối thiểu 5 ký tự — mô tả ngắn vấn đề của bạn".into(),
        ));
    }
    if title_len > 200 {
        return Err(AppError::BadRequest(
            "Tiêu đề tối đa 200 ký tự — rút gọn để admin dễ đọc".into(),
        ));
    }

    // 3) Body: 10-5000 ký tự
    let body = form.body.trim();
    let body_len = body.chars().count();
    if body_len < 10 {
        return Err(AppError::BadRequest(
            "Nội dung tối thiểu 10 ký tự — mô tả chi tiết hơn để mình hỗ trợ".into(),
        ));
    }
    if body_len > 5000 {
        return Err(AppError::BadRequest(
            "Nội dung tối đa 5000 ký tự — tách thành nhiều góp ý nhỏ hơn".into(),
        ));
    }

    // 4) page_url: same-site only (chống nhúng URL ngoài làm phishing),
    //    tối đa 2048 ký tự.
    let page_url = form.page_url.trim();
    if !page_url.is_empty() {
        // Chặn cả "/\": Chrome/Edge coi \ như / → "/\evil.com" ≡
        // "//evil.com" (protocol-relative ra site ngoài) — cùng class bug
        // mà sanitize_redirect đã fix (audit v3.4.0).
        if !page_url.starts_with('/')
            || page_url.starts_with("//")
            || page_url.starts_with("/\\")
            || page_url.contains('\r')
            || page_url.contains('\n')
        {
            return Err(AppError::BadRequest(
                "URL trang chỉ nhận đường dẫn nội bộ (bắt đầu bằng /)".into(),
            ));
        }
        if page_url.len() > 2048 {
            return Err(AppError::BadRequest(
                "URL trang quá dài (tối đa 2048 ký tự)".into(),
            ));
        }
    }

    // 5) Rate-limit 10 góp ý / 24h
    let recent = FeedbackRepo::count_recent_by_user(&state.db, user.id).await?;
    if recent >= 10 {
        return Err(AppError::Conflict(
            "Bạn đã gửi 10 góp ý trong 24 giờ qua — thử lại sau nhé.".into(),
        ));
    }

    FeedbackRepo::create(&state.db, user.id, &category, title, body, page_url).await?;

    tracing::info!(
        user = %user.username,
        category = category.key(),
        "User submitted feedback"
    );

    // Redirect về trang feedback với flash "đã gửi"
    Ok(Redirect::to("/feedback?sent=1").into_response())
}

// ============================================================
// ADMIN: /admin/feedback
// ============================================================

/// Trang admin — danh sách feedback theo trạng thái.
///
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn admin_feedback_page(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Query(q): Query<FeedbackQuery>,
) -> AppResult<AdminFeedbackTemplate> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    // v3.4.0 (audit): góp ý BẢO MẬT chỉ ADMIN được xem — moderator thấy
    // mọi danh mục khác (đúng doc model + migration + template).
    let is_admin = user.role.is_admin();
    let status = q
        .status
        .as_deref()
        .and_then(FeedbackStatus::from_str)
        .filter(|_| {
            !q.status
                .as_deref()
                .unwrap_or("")
                .eq_ignore_ascii_case("all")
        });
    let active_key = status.map_or_else(String::new, |s| s.key().to_string());
    let (items, counts, unread) = tokio::join!(
        FeedbackRepo::list_for_admin(&state.db, status, 100, is_admin),
        FeedbackRepo::counts_by_status(&state.db, is_admin),
        unread_count(&state, user.id)
    );
    let security_hidden_note = if is_admin { None } else { Some(true) };
    Ok(AdminFeedbackTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        items: items.unwrap_or_default(),
        counts: counts.unwrap_or_default(),
        active_status: status,
        active_key,
        statuses: FeedbackStatus::all().to_vec(),
        is_admin,
        security_hidden: security_hidden_note,
    })
}

#[derive(Debug, Deserialize)]
pub struct FeedbackStatusForm {
    pub status: String,
    #[serde(default)]
    pub admin_response: String,
}

/// POST /admin/feedback/{id}/status — đổi trạng thái + phản hồi người gửi.
///
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn admin_feedback_update(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Form(form): Form<FeedbackStatusForm>,
) -> AppResult<Response> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let status = FeedbackStatus::from_str(&form.status)
        .ok_or_else(|| AppError::BadRequest("Trạng thái không hợp lệ".into()))?;
    let response = form.admin_response.trim();
    if response.chars().count() > 2000 {
        return Err(AppError::BadRequest("Phản hồi tối đa 2000 ký tự".into()));
    }
    // v3.4.0 (audit): feedback BẢO MẬT chỉ admin được cập nhật trạng thái
    // (moderator không thấy nội dung bảo mật — tránh lộ lỗ hổng cho
    // tài khoản staff bị xâm phạm).
    let existing = FeedbackRepo::find_by_id(&state.db, id).await?;
    if let Some(f) = &existing {
        if f.category.is_security() && !user.role.is_admin() {
            return Err(AppError::Forbidden(
                "Góp ý bảo mật chỉ quản trị viên được xử lý".into(),
            ));
        }
    }
    // Chỉ gửi notification khi trạng thái THAY ĐỔI thực sự (trước đây
    // set lại Resolved nhiều lần = spam notification trùng — audit).
    let status_changed = existing.as_ref().is_none_or(|f| f.status != status);
    let target_user = FeedbackRepo::update_status(&state.db, id, user.id, status, response).await?;
    let Some(target_user) = target_user else {
        return Err(AppError::NotFound(
            "Góp ý không tồn tại (đã bị xóa?)".into(),
        ));
    };

    // Thông báo cho người gửi khi admin xử lý xong — INSERT trực tiếp
    // cùng pattern ReportRepo. CHỈ gửi khi trạng thái thay đổi thật.
    if status_changed && matches!(status, FeedbackStatus::Resolved | FeedbackStatus::Dismissed) {
        let title = if matches!(status, FeedbackStatus::Resolved) {
            "✅ Góp ý của bạn đã được xử lý"
        } else {
            "ℹ️ Góp ý của bạn đã được xem xét"
        };
        let content: String = if response.is_empty() {
            "Cảm ơn bạn đã góp ý cho Louis Space!".to_string()
        } else {
            response.chars().take(180).collect()
        };
        let _ = sqlx::query(
            r"INSERT INTO notifications (user_id, type, title, content, link)
              VALUES ($1, 'feedback_status'::notification_type, $2, $3, '/feedback')",
        )
        .bind(target_user)
        .bind(title)
        .bind(content)
        .execute(&state.db)
        .await;
    }

    crate::services::audit::audit(
        &state,
        user.id,
        "feedback.update_status",
        "user_feedback",
        &id.to_string(),
        &format!(
            "{} chuyển góp ý sang trạng thái {} (phản hồi: {} ký tự)",
            user.username,
            status.label(),
            response.chars().count()
        ),
    )
    .await;

    // Redirect về filter đang active
    Ok(Redirect::to(&format!(
        "/admin/feedback?status={}",
        status_label_key(status)
    ))
    .into_response())
}

/// Key form của trạng thái (lowercase snake) dùng trong redirect.
fn status_label_key(s: FeedbackStatus) -> &'static str {
    match s {
        FeedbackStatus::Pending => "pending",
        FeedbackStatus::Reviewing => "reviewing",
        FeedbackStatus::Resolved => "resolved",
        FeedbackStatus::Dismissed => "dismissed",
    }
}
