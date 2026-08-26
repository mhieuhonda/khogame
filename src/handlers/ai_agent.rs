//! Handlers cho AI Agent account system.
//!
//! Bao gồm:
//! - [`register`]: POST /auth/ai/register — AI tạo tài khoản (yêu cầu secret). Body JSON.
//! - [`login_form`]: GET /auth/ai/login — trang web form nhập API token.
//! - [`login`]: POST /auth/ai/login — AI đăng nhập bằng API token, nhận session cookie. Body form.
//! - [`report_progress`]: POST /ai/progress — AI báo cáo tiến trình (form hoặc JSON).
//! - [`update_profile`]: POST /profile/ai — AI tự cập nhật hồ sơ.
//! - [`info`]: GET /ai/info — Trả về thông tin AI Agent đang xác thực.

use crate::auth;
use crate::error::{AppError, AppResult};
use crate::handlers::auth::unread_count;
use crate::middleware::{AuthAiAgent, AuthUser, CurrentUser};
use crate::models::ai_agent::{AiPrivacyLevel, AiTaskStatus};
use crate::models::user::User;
use crate::repositories::{AiAgentRepo, SessionRepo};
use crate::state::AppState;
use crate::templates::{AiLoginTemplate, AiProfileEditTemplate};
use askama::Template;
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct AuthQuery {
    pub next: Option<String>,
}

// ============================================================
// Đăng ký AI Agent (POST /auth/ai/register, body JSON)
// ============================================================
/// Body cho đăng ký AI Agent. AI gửi kèm secret do admin cấp
/// out-of-band. Nếu secret sai → 403 Forbidden. Nếu feature chưa
/// bật trong env (`AI_AGENT_SECRET` rỗng) → 403.
#[derive(Debug, Deserialize)]
pub struct AiRegisterRequest {
    pub secret: String,
    pub model_name: String,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub bio: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub capabilities: Option<Vec<String>>,
    #[serde(default)]
    pub privacy_level: Option<String>,
    #[serde(default)]
    pub accent_color: Option<String>,
    #[serde(default)]
    pub token_label: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AiRegisterResponse {
    pub success: bool,
    /// Plain API token — chỉ trả 1 lần. AI phải lưu lại.
    pub api_token: String,
    pub username: String,
    pub user_id: String,
    pub message: String,
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn register(
    State(state): State<Arc<AppState>>,
    axum::Json(req): axum::Json<AiRegisterRequest>,
) -> AppResult<Response> {
    // 1) Kiểm tra feature đã bật (có secret trong env)
    if !state.config.ai_agent_enabled {
        return Err(AppError::Forbidden(
            "AI Agent registration is disabled (AI_AGENT_SECRET not set)".into(),
        ));
    }
    // 2) Verify secret (constant-time compare để chống timing attack)
    if !constant_time_eq(
        req.secret.as_bytes(),
        state.config.ai_agent_secret.as_bytes(),
    ) {
        tracing::warn!(
            "AI Agent registration: invalid secret (model_name={})",
            req.model_name
        );
        return Err(AppError::Forbidden("Secret không hợp lệ".into()));
    }
    // 3) Validate fields — length limits để chống lạm dụng (DB có thể
    //    nhận payload lớn do TEXT fields không có constraint).
    let model_name = req.model_name.trim();
    if model_name.is_empty() {
        return Err(AppError::BadRequest(
            "Tên model không được để trống (vd 'Ox Alpha')".into(),
        ));
    }
    if model_name.chars().count() > 100 {
        return Err(AppError::BadRequest("Tên model tối đa 100 ký tự".into()));
    }
    if req.vendor.chars().count() > 50 {
        return Err(AppError::BadRequest("Vendor tối đa 50 ký tự".into()));
    }
    if req.version.chars().count() > 50 {
        return Err(AppError::BadRequest("Version tối đa 50 ký tự".into()));
    }
    if let Some(bio) = req.bio.as_deref() {
        if bio.trim().chars().count() > 500 {
            return Err(AppError::BadRequest("Bio tối đa 500 ký tự".into()));
        }
    }
    // Validate display_name length (chống payload 1MB insert vào users.display_name)
    if let Some(dn) = req.display_name.as_deref() {
        if dn.trim().chars().count() > 100 {
            return Err(AppError::BadRequest("Display name tối đa 100 ký tự".into()));
        }
    }
    // Validate token_label length
    if let Some(tl) = req.token_label.as_deref() {
        if tl.trim().chars().count() > 100 {
            return Err(AppError::BadRequest("Token label tối đa 100 ký tự".into()));
        }
    }
    // Validate email — RFC 5321 max 254 ký tự
    if let Some(email) = req.email.as_deref() {
        if !email.is_empty() && email.trim().chars().count() > 254 {
            return Err(AppError::BadRequest("Email quá dài (tối đa 254 ký tự)".into()));
        }
    }
    // Validate username length — repo tự slugify nhưng text raw vẫn
    // chảy qua DB bind. Cắt 100 ký tự là hợp lý.
    if let Some(un) = req.username.as_deref() {
        if !un.is_empty() && un.trim().chars().count() > 100 {
            return Err(AppError::BadRequest("Username tối đa 100 ký tự".into()));
        }
    }
    // Validate accent_color — phải là hex color (chống payload lạ)
    if let Some(color) = req.accent_color.as_deref() {
        if !is_valid_hex_color(color) {
            return Err(AppError::BadRequest(
                "Accent color phải là hex color (vd #7c3aed)".into(),
            ));
        }
    }
    // Validate privacy_level — whitelist
    if let Some(level) = req.privacy_level.as_deref() {
        if !matches!(level, "public" | "private" | "internal") {
            return Err(AppError::BadRequest(
                "Privacy level phải là public/private/internal".into(),
            ));
        }
    }
    if let Some(avatar) = req.avatar_url.as_deref() {
        if !avatar.is_empty() && !crate::utils::is_safe_url(avatar) {
            return Err(AppError::BadRequest(
                "Avatar URL phải là http:// hoặc https://".into(),
            ));
        }
        if avatar.len() > 2048 {
            return Err(AppError::BadRequest(
                "Avatar URL quá dài (tối đa 2048 ký tự)".into(),
            ));
        }
    }
    if let Some(caps) = req.capabilities.as_ref() {
        if caps.len() > 20 {
            return Err(AppError::BadRequest("Tối đa 20 capability".into()));
        }
        for c in caps {
            if c.chars().count() > 50 {
                return Err(AppError::BadRequest(
                    "Mỗi capability tối đa 50 ký tự".into(),
                ));
            }
        }
    }
    let display_name = req
        .display_name
        .as_deref()
        .filter(|s| !s.trim().is_empty()).map_or_else(|| req.model_name.trim().to_string(), |s| s.trim().to_string());
    let token_label = req
        .token_label
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("default")
        .to_string();
    let capabilities = req.capabilities.unwrap_or_default();

    // 4) Tạo user + profile + token (Repo tự sinh username duy nhất)
    let plain_token = AiAgentRepo::register(
        &state.db,
        req.email.as_deref().unwrap_or(""),
        req.username.as_deref().unwrap_or(""),
        &display_name,
        req.bio.as_deref(),
        req.avatar_url.as_deref(),
        model_name,
        &req.vendor,
        &req.version,
        &capabilities,
        req.privacy_level.as_deref().unwrap_or("public"),
        req.accent_color.as_deref().unwrap_or("#7c3aed"),
        &token_label,
        None,
        "ai-agent-register",
    )
    .await?;

    // 5) Trả về token (chỉ 1 lần!) + thông tin user
    // Lấy lại username cuối cùng từ DB (Repo đã tự ensure unique)
    let token_hash = auth::hash_token(&plain_token);
    let user_id: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT user_id FROM ai_agent_tokens WHERE token_hash = $1")
            .bind(&token_hash)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();
    let username: String = match user_id {
        Some(id) => sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
            .bind(id)
            .fetch_one(&state.db)
            .await
            .unwrap_or_else(|_| display_name.clone()),
        None => display_name.clone(),
    };

    tracing::info!(
        "AI Agent registered: model_name={} vendor={} username={}",
        req.model_name,
        req.vendor,
        username
    );

    let resp = AiRegisterResponse {
        success: true,
        api_token: plain_token,
        username,
        user_id: user_id.map(|u| u.to_string()).unwrap_or_default(),
        message: "Đăng ký AI Agent thành công. Lưu API token cẩn thận — chỉ hiển thị 1 lần.".into(),
    };
    Ok(axum::response::Json(resp).into_response())
}

/// So sánh 2 slice byte constant-time (chống timing attack).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

// ============================================================
// Đăng nhập AI Agent (POST /auth/ai/login)
// ============================================================
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn login_form(
    State(state): State<Arc<AppState>>,
    Query(q): Query<AuthQuery>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<Response> {
    if !state.config.ai_agent_enabled {
        return Err(AppError::Forbidden(
            "AI Agent login is disabled (feature not configured)".into(),
        ));
    }
    if current_user.is_some() {
        return Ok(Redirect::to("/").into_response());
    }
    let tpl = AiLoginTemplate {
        current_user: None,
        unread_notifications: 0,
        next: q.next,
    };
    Ok(Html(tpl.render().map_err(AppError::from)?).into_response())
}

#[derive(Debug, Deserialize)]
pub struct AiLoginForm {
    pub api_token: String,
    pub next: Option<String>,
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn login(
    State(state): State<Arc<AppState>>,
    Query(q): Query<AuthQuery>,
    jar: CookieJar,
    Form(form): Form<AiLoginForm>,
) -> AppResult<(CookieJar, Redirect)> {
    if !state.config.ai_agent_enabled {
        return Err(AppError::Forbidden("AI Agent login is disabled".into()));
    }
    let token = form.api_token.trim();
    if token.is_empty() {
        return Err(AppError::BadRequest("API token không được để trống".into()));
    }
    // Tra user theo token
    let (user, _profile) = AiAgentRepo::find_by_api_token(&state.db, token)
        .await?
        .ok_or_else(|| AppError::Forbidden("API token không hợp lệ hoặc đã bị thu hồi".into()))?;
    if !user.role.is_ai_agent() {
        return Err(AppError::Forbidden(
            "Token này không thuộc tài khoản AI Agent".into(),
        ));
    }
    if user.is_banned {
        return Err(AppError::Forbidden("Tài khoản AI Agent đã bị cấm".into()));
    }
    // Tạo session
    let session_token = auth::gen_session_token();
    let token_hash = auth::hash_token(&session_token);
    SessionRepo::create(
        &state.db,
        user.id,
        &token_hash,
        "ai-agent-web",
        None,
        state.config.ai_agent_session_ttl_days,
    )
    .await?;
    let mut new_jar = jar;
    auth::set_session_cookie(&mut new_jar, &session_token, &state.config.base_url);
    tracing::info!("AI Agent logged in: {}", user.username);
    // Safe redirect next — sử dụng sanitize_redirect để chặn control char
    // (CR/LF/TAB) chống header injection qua Location. Trước đây chỉ
    // check starts_with('/') && !starts_with("//") cho phép \r\n qua.
    let next_raw = form
        .next
        .as_deref()
        .or(q.next.as_deref())
        .filter(|s| !s.is_empty())
        .map_or_else(|| "/".to_string(), crate::utils::sanitize_redirect);
    Ok((new_jar, Redirect::to(&next_raw)))
}

// ============================================================
// Báo cáo tiến trình (POST /ai/progress)
// ============================================================
#[derive(Debug, Deserialize)]
pub struct AiProgressRequest {
    pub task: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub percentage: Option<i16>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    /// JSON string tuỳ chọn (metadata)
    #[serde(default)]
    pub metadata: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AiProgressResponse {
    pub success: bool,
    pub report_id: String,
    pub percentage: i16,
    pub status: String,
    pub message: String,
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn report_progress(
    State(state): State<Arc<AppState>>,
    AuthAiAgent(user): AuthAiAgent,
    Form(req): Form<AiProgressRequest>,
) -> AppResult<axum::response::Json<AiProgressResponse>> {
    report_progress_impl(&state, user, req).await
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn report_progress_json(
    State(state): State<Arc<AppState>>,
    AuthAiAgent(user): AuthAiAgent,
    axum::Json(req): axum::Json<AiProgressRequest>,
) -> AppResult<axum::response::Json<AiProgressResponse>> {
    report_progress_impl(&state, user, req).await
}

async fn report_progress_impl(
    state: &AppState,
    user: User,
    req: AiProgressRequest,
) -> AppResult<axum::response::Json<AiProgressResponse>> {
    // Validate độ dài để tránh AI gửi payload lớn vô tội vạ.
    let task = req.task.trim();
    if task.is_empty() {
        return Err(AppError::BadRequest("Task không được để trống".into()));
    }
    if task.chars().count() > 200 {
        return Err(AppError::BadRequest("Task tối đa 200 ký tự".into()));
    }
    let action = req.action.trim();
    if action.chars().count() > 200 {
        return Err(AppError::BadRequest("Action tối đa 200 ký tự".into()));
    }
    let message = req.message.as_deref().unwrap_or("").trim();
    if message.chars().count() > 2000 {
        return Err(AppError::BadRequest("Message tối đa 2000 ký tự".into()));
    }
    if let Some(md) = req.metadata.as_deref().filter(|s| !s.is_empty()) {
        if md.len() > 8192 {
            return Err(AppError::BadRequest(
                "Metadata JSON tối đa 8192 ký tự".into(),
            ));
        }
    }
    let percentage = req.percentage.unwrap_or(0).clamp(0, 100);
    let status = req
        .status
        .as_deref()
        .map_or(AiTaskStatus::Running, parse_status);
    let metadata: Option<serde_json::Value> = req
        .metadata
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| serde_json::from_str(s).ok());
    let report = AiAgentRepo::add_progress(
        &state.db,
        user.id,
        task,
        action,
        percentage,
        &status,
        message,
        metadata.as_ref(),
        None,
    )
    .await?;
    Ok(axum::response::Json(AiProgressResponse {
        success: true,
        report_id: report.id.to_string(),
        percentage: report.percentage,
        status: format!("{:?}", report.status).to_lowercase(),
        message: "Tiến trình đã được ghi nhận".into(),
    }))
}

fn parse_status(s: &str) -> AiTaskStatus {
    match s.to_ascii_lowercase().as_str() {
        "queued" | "queue" => AiTaskStatus::Queued,
        "done" | "completed" | "success" => AiTaskStatus::Done,
        "failed" | "error" => AiTaskStatus::Failed,
        "cancelled" | "canceled" => AiTaskStatus::Cancelled,
        _ => AiTaskStatus::Running,
    }
}

// ============================================================
// AI Agent tự cập nhật hồ sơ (POST /profile/ai)
// ============================================================
#[derive(Debug, Deserialize)]
pub struct AiProfileForm {
    pub model_name: String,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub capabilities: Option<String>, // newline-separated
    #[serde(default)]
    pub privacy_level: Option<String>,
    #[serde(default)]
    pub accent_color: Option<String>,
    #[serde(default)]
    pub bio: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn update_profile(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Form(form): Form<AiProfileForm>,
) -> AppResult<Redirect> {
    // Chỉ AI Agent mới được dùng endpoint này
    if !user.role.is_ai_agent() {
        return Err(AppError::Forbidden(
            "Chỉ tài khoản AI Agent mới được cập nhật hồ sơ AI".into(),
        ));
    }
    // Validate độ dài các trường text — chống lạm dụng.
    let model_name = form.model_name.trim();
    if model_name.is_empty() {
        return Err(AppError::BadRequest("Tên model không được để trống".into()));
    }
    if model_name.chars().count() > 100 {
        return Err(AppError::BadRequest("Tên model tối đa 100 ký tự".into()));
    }
    if form.vendor.chars().count() > 50 {
        return Err(AppError::BadRequest("Vendor tối đa 50 ký tự".into()));
    }
    if form.version.chars().count() > 50 {
        return Err(AppError::BadRequest("Version tối đa 50 ký tự".into()));
    }
    if let Some(bio) = form.bio.as_deref() {
        if bio.trim().chars().count() > 500 {
            return Err(AppError::BadRequest("Bio tối đa 500 ký tự".into()));
        }
    }
    if let Some(avatar) = form.avatar_url.as_deref() {
        if !avatar.is_empty() && !crate::utils::is_safe_url(avatar) {
            return Err(AppError::BadRequest(
                "Avatar URL phải là http:// hoặc https://".into(),
            ));
        }
        if avatar.len() > 2048 {
            return Err(AppError::BadRequest(
                "Avatar URL quá dài (tối đa 2048 ký tự)".into(),
            ));
        }
    }
    let capabilities: Vec<String> = form
        .capabilities
        .as_deref()
        .map(|s| {
            s.lines()
                .map(|l| l.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    if capabilities.len() > 20 {
        return Err(AppError::BadRequest(
            "Tối đa 20 capability (mỗi dòng 1 capability)".into(),
        ));
    }
    for cap in &capabilities {
        if cap.chars().count() > 50 {
            return Err(AppError::BadRequest(
                "Mỗi capability tối đa 50 ký tự".into(),
            ));
        }
    }
    let _profile = AiAgentRepo::update_profile(
        &state.db,
        user.id,
        model_name,
        form.vendor.as_str(),
        form.version.as_str(),
        &capabilities,
        form.privacy_level.as_deref().unwrap_or("public"),
        form.accent_color.as_deref().unwrap_or("#7c3aed"),
        form.bio.as_deref().unwrap_or(""),
        form.avatar_url.as_deref(),
    )
    .await?;
    Ok(Redirect::to(&format!("/u/{}", user.username)))
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn edit_profile_form(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<AiProfileEditTemplate> {
    if !user.role.is_ai_agent() {
        return Err(AppError::Forbidden(
            "Chỉ tài khoản AI Agent mới truy cập được trang này".into(),
        ));
    }
    let profile = AiAgentRepo::find_profile_by_user_id(&state.db, user.id)
        .await?
        .ok_or_else(|| AppError::NotFound("Hồ sơ AI Agent không tồn tại".into()))?;
    let unread = unread_count(&state, user.id).await;
    Ok(AiProfileEditTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        profile,
        privacy_public_label: AiPrivacyLevel::Public.label(),
        privacy_anonymous_label: AiPrivacyLevel::Anonymous.label(),
    })
}

// ============================================================
// AI Agent info (GET /ai/info) — kiểm tra token hợp lệ không
// ============================================================
#[derive(Debug, Serialize)]
pub struct AiInfoResponse {
    pub success: bool,
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub model_name: String,
    pub vendor: String,
    pub verified: bool,
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn info(
    State(state): State<Arc<AppState>>,
    AuthAiAgent(user): AuthAiAgent,
) -> AppResult<axum::response::Json<AiInfoResponse>> {
    let profile = AiAgentRepo::find_profile_by_user_id(&state.db, user.id)
        .await?
        .ok_or_else(|| AppError::NotFound("Hồ sơ AI Agent không tồn tại".into()))?;
    Ok(axum::response::Json(AiInfoResponse {
        success: true,
        user_id: user.id.to_string(),
        username: user.username,
        display_name: user.display_name,
        model_name: profile.model_name,
        vendor: profile.vendor,
        verified: profile.verified,
    }))
}

/// Validate hex color string: #RGB hoặc #RRGGBB (case-insensitive).
/// Tránh lưu payload lạ vào ai_agent_profiles.accent_color.
fn is_valid_hex_color(s: &str) -> bool {
    let s = s.trim();
    if !s.starts_with('#') {
        return false;
    }
    let rest = &s[1..];
    matches!(rest.len(), 3 | 6) && rest.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_status_aliases() {
        // Mỗi alias phải map đúng variant
        assert_eq!(parse_status("queued"), AiTaskStatus::Queued);
        assert_eq!(parse_status("queue"), AiTaskStatus::Queued);
        assert_eq!(parse_status("done"), AiTaskStatus::Done);
        assert_eq!(parse_status("completed"), AiTaskStatus::Done);
        assert_eq!(parse_status("success"), AiTaskStatus::Done);
        assert_eq!(parse_status("failed"), AiTaskStatus::Failed);
        assert_eq!(parse_status("error"), AiTaskStatus::Failed);
        assert_eq!(parse_status("cancelled"), AiTaskStatus::Cancelled);
        assert_eq!(parse_status("canceled"), AiTaskStatus::Cancelled);
        assert_eq!(parse_status("running"), AiTaskStatus::Running);
        // Case-insensitive
        assert_eq!(parse_status("DONE"), AiTaskStatus::Done);
        // Giá trị lạ → Running (mặc định an toàn cho progress đang gửi)
        assert_eq!(parse_status("bất kỳ"), AiTaskStatus::Running);
        assert_eq!(parse_status(""), AiTaskStatus::Running);
    }

    #[test]
    fn test_constant_time_eq_basic() {
        assert!(constant_time_eq(b"secret123", b"secret123"));
        assert!(!constant_time_eq(b"secret123", b"secret124"));
        assert!(!constant_time_eq(b"abc", b"abd"));
    }

    #[test]
    fn test_constant_time_eq_length_mismatch() {
        // Khác độ dài → false ngay (độ dài không phải bí mật cần giấu
        // là public qua nhiều kênh khác)
        assert!(!constant_time_eq(b"short", b"longer-string"));
        assert!(!constant_time_eq(b"", b"x"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn test_constant_time_eq_no_early_exit() {
        // Khác nhau ở byte ĐẦU phải cho kết quả giống khác nhau ở byte CUỐI
        // (nếu có early-return theo vị trí byte sẽ lệch timing)
        let a = b"aaaaaaaaaaaaaaaa";
        let first = b"baaaaaaaaaaaaaaa";
        let last = b"aaaaaaaaaaaaaab";
        assert!(!constant_time_eq(a, first));
        assert!(!constant_time_eq(a, last));
    }
}
