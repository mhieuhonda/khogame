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
use crate::utils::constant_time_eq;
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
    /// Thông báo lỗi login (v3.4.0 — render lại form sau khi POST fail).
    pub error: Option<String>,
    /// Username điền lại sau khi sai mật khẩu (không trả mật khẩu).
    pub username: Option<String>,
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
    headers: axum::http::HeaderMap,
    axum::Json(req): axum::Json<AiRegisterRequest>,
) -> AppResult<Response> {
    // 1) Kiểm tra feature đã bật (có secret trong env)
    if !state.config.ai_agent_enabled {
        return Err(AppError::Forbidden(
            "AI Agent registration is disabled (AI_AGENT_SECRET not set)".into(),
        ));
    }
    // 1b) Origin/Referer check — endpoint này được gọi bởi AI script, không
    // phải browser form. Nhưng nếu secret bị lộ, attacker có thể dùng cross-site
    // fetch để tạo AI Agent. CORS + Origin check chặn trừ domain lạ.
    // Cho phép curl (không Origin/Referer) nhưng từ chối domain khác.
    crate::middleware::verify_origin(&headers, &state.config.base_url)?;
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
    // Validate email — RFC 5321 max 254 ký tự + format cơ bản.
    // v3.5.1 FIX (audit 5-e F10): trước đây chỉ check length — AI secret
    // holder đăng ký agent với email NẠO BẤT KỲ (email nạn nhân) → mỗi lượt
    // follow là 1 email gửi tới địa chỉ đó qua SMTP của site (spam relay).
    // Format check chặn địa chỉ rác; trigger email chỉ dùng cho user thật.
    if let Some(email) = req.email.as_deref() {
        if !email.is_empty() {
            let e = email.trim();
            if e.chars().count() > 254 {
                return Err(AppError::BadRequest(
                    "Email quá dài (tối đa 254 ký tự)".into(),
                ));
            }
            if !is_valid_email(e) {
                return Err(AppError::BadRequest(
                    "Email không hợp lệ (vd: name@example.com)".into(),
                ));
            }
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
        // Đồng bộ với repo: chỉ chấp nhận "public" hoặc "anonymous"
        // (enum AiPrivacyLevel chỉ có 2 variant). Trước đây handler cho
        // qua "private"/"internal" rồi repo reject với message khác —
        // user nhận 400 khó hiểu.
        if !matches!(level, "public" | "anonymous") {
            return Err(AppError::BadRequest(
                "Privacy level phải là 'public' hoặc 'anonymous'".into(),
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
        .filter(|s| !s.trim().is_empty())
        .map_or_else(
            || req.model_name.trim().to_string(),
            |s| s.trim().to_string(),
        );
    let token_label = req
        .token_label
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("default")
        .to_string();
    let capabilities = req.capabilities.unwrap_or_default();

    // 4) Tạo user + profile + token (Repo tự sinh username duy nhất)
    // v3.4.2: token có TTL (AI_AGENT_TOKEN_TTL_DAYS, mặc định 365) — hết
    // vòng đời "sống mãi mãi", lộ token chỉ ảnh hưởng trong cửa sổ TTL.
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
        state.config.ai_agent_token_ttl_days,
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

// (v2.9.2) `constant_time_eq` chuyển sang `crate::utils` dùng chung cho
// OAuth state + AI token. Xem utils.rs — logic giữ nguyên.

// ============================================================
// Đăng nhập AI Agent (GET/POST /auth/ai/login)
// v3.4.0 — REWORK HOÀN TOÀN: Username + Mật khẩu do admin tạo
// (mật khẩu có thời hạn do admin đặt — xem migration 028).
// ============================================================
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn login_form(
    State(_state): State<Arc<AppState>>,
    Query(q): Query<AuthQuery>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<Response> {
    // v3.4.0 FIX — KHÔNG còn gate `ai_agent_enabled` (AI_AGENT_SECRET)
    // cho trang login: mật khẩu do admin tạo trực tiếp (migration 028),
    // không cần secret. Trước đây prod chưa set AI_AGENT_SECRET → trang
    // login trả 403 "disabled" dù tính năng mật khẩu hoàn toàn sẵn sàng.
    // (Endpoint /auth/ai/register cũ vẫn giữ gate secret.)
    // v3.4.0 FIX — KHÔNG còn redirect "/" khi user đã đăng nhập.
    // Trước đây admin (đang login) mở /auth/ai/login bị redirect về trang
    // chủ → KHÔNG BAO GIỜ thấy form đăng nhập AI (bug "admin không thể
    // đăng nhập tài khoản AI Agent vì không thấy phần đăng nhập").
    // Giờ: luôn render form; template hiển thị cảnh báo "phiên hiện tại
    // sẽ bị thay thế" nếu đang có user.
    let tpl = AiLoginTemplate {
        current_user,
        unread_notifications: 0,
        next: q.next,
        error: q.error.filter(|s| !s.is_empty()),
        last_username: q.username.filter(|s| !s.is_empty()),
    };
    Ok(Html(tpl.render().map_err(AppError::from)?).into_response())
}

#[derive(Debug, Deserialize)]
pub struct AiLoginForm {
    pub username: String,
    pub password: String,
    pub next: Option<String>,
}

/// Helper: redirect về form login kèm thông báo lỗi (giữ username + next).
fn login_error_redirect(form: &AiLoginForm, q: &AuthQuery, msg: &str) -> Response {
    let mut loc = format!("/auth/ai/login?error={}", crate::utils::urlencode(msg));
    if !form.username.trim().is_empty() {
        loc.push_str(&format!(
            "&username={}",
            crate::utils::urlencode(form.username.trim())
        ));
    }
    if let Some(next) = q
        .next
        .as_deref()
        .or(form.next.as_deref())
        .filter(|s| !s.is_empty())
    {
        loc.push_str(&format!("&next={}", crate::utils::urlencode(next)));
    }
    Redirect::to(&loc).into_response()
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn login(
    State(state): State<Arc<AppState>>,
    Query(q): Query<AuthQuery>,
    headers: axum::http::HeaderMap,
    jar: CookieJar,
    Form(form): Form<AiLoginForm>,
) -> AppResult<Response> {
    // v3.4.0 — KHÔNG gate ai_agent_enabled (xem login_form): đăng nhập
    // username + mật khẩu hoạt động độc lập với AI_AGENT_SECRET.
    // Origin/Referer check — chống login CSRF cross-site auto-submit.
    // Endpoint tạo session mới nên SameSite=Lax cookie không bảo vệ được.
    crate::middleware::verify_origin(&headers, &state.config.base_url)?;

    // v3.4.0 — đăng nhập bằng Username + Password (admin tạo, Argon2id,
    // có thời hạn). Sai → redirect về form với error (không render trực
    // tiếp để tránh re-submit khi refresh).
    match AiAgentRepo::verify_password_login(&state.db, &form.username, &form.password).await {
        Ok(user) => {
            // Tạo session web cho AI Agent
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
            // v3.6.0 SECURITY/UX FIX (audit — "admin mất phiên gốc không
            // đường về"): nếu người submit form đang giữ phiên STAFF hợp lệ
            // (admin/mod), mint impersonation ticket one-shot trước khi ghi
            // đè kg_session — trước đây phiên admin bị ghi đè TRẮN, admin
            // kẹt luôn trong tài khoản AI (đồng thời mất nút impersonate
            // trên hồ sơ vì is_self). Giờ: bấm Đăng xuất hoặc POST
            // /impersonate/stop sẽ khôi phục phiên staff như flow
            // impersonation ở /admin/ai-agents (TTL ticket 2h, audit log).
            let staff_user = crate::middleware::current_user_from_jar(&state, &new_jar).await;
            if let Some(staff) = staff_user.as_ref().filter(|u| u.role.is_staff()) {
                let ticket_id = uuid::Uuid::new_v4();
                let inserted = sqlx::query(
                    r#"INSERT INTO impersonation_tickets
                           (id, admin_user_id, target_user_id, expires_at)
                       VALUES ($1, $2, $3, NOW() + ($4 || ' hours')::INTERVAL)"#,
                )
                .bind(ticket_id)
                .bind(staff.id)
                .bind(user.id)
                .bind(crate::auth::IMPERSONATION_TTL_HOURS.to_string())
                .execute(&state.db)
                .await;
                match inserted {
                    Ok(_) => {
                        auth::set_impersonator_cookie(
                            &mut new_jar,
                            &ticket_id.to_string(),
                            &state.config.base_url,
                        );
                        tracing::warn!(
                            staff = %staff.username,
                            target = %user.username,
                            "AI password login bởi STAFF — mint impersonation ticket để khôi phục phiên staff"
                        );
                    }
                    Err(e) => {
                        // Không chặn login vì thiếu ticket — chỉ log (admin
                        // vẫn đăng nhập lại được bằng Google OAuth).
                        tracing::warn!("Tạo impersonation ticket cho staff fail: {e}");
                    }
                }
            }
            // Ghi đè cookie session hiện tại (nếu admin đang login bằng
            // tài khoản người → phiên AI thay thế — đúng kỳ vọng "đăng
            // nhập vào tài khoản AI").
            auth::set_session_cookie(&mut new_jar, &session_token, &state.config.base_url);
            tracing::info!("AI Agent logged in (username+password): {}", user.username);

            // Safe redirect next — sanitize_redirect chặn control char
            // (CR/LF/TAB) chống header injection qua Location.
            let next_raw = form
                .next
                .as_deref()
                .or(q.next.as_deref())
                .filter(|s| !s.is_empty())
                .map_or_else(|| "/".to_string(), crate::utils::sanitize_redirect);
            Ok((new_jar, Redirect::to(&next_raw)).into_response())
        }
        Err(AppError::Forbidden(msg)) => {
            tracing::warn!("AI Agent login failed: {msg}");
            Ok(login_error_redirect(&form, &q, &msg))
        }
        Err(e) => Err(e),
    }
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
        // Validate JSON hợp lệ — trước đây chỉ from_str().ok() silently
        // drop metadata nếu JSON lỗi, AI tưởng đã lưu nhưng thực ra không.
        if serde_json::from_str::<serde_json::Value>(md).is_err() {
            return Err(AppError::BadRequest("Metadata phải là JSON hợp lệ".into()));
        }
    }
    let percentage = req.percentage.unwrap_or(0).clamp(0, 100);
    let status = req
        .status
        .as_deref()
        .map_or(AiTaskStatus::Running, parse_status);
    // Parse metadata lại (đã validate phía trên) — trả None nếu rỗng.
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
    // Validate accent_color — phải là hex color (#RGB hoặc #RRGGBB).
    // Cùng chuẩn với register: chặn payload lạ vào DB (có thể vỡ CSS
    // render hoặc lách CSP qua value cũ) và giữ giá trị mặc định khi rỗng.
    let accent_color = form.accent_color.as_deref().unwrap_or("#7c3aed");
    if !is_valid_hex_color(accent_color) {
        return Err(AppError::BadRequest(
            "Accent color phải là hex color (vd #7c3aed)".into(),
        ));
    }
    // Validate privacy_level — whitelist giá trị hợp lệ. Trước đây update
    // không kiểm tra (chỉ register có check) → AI Agent có thể set giá trị
    // lạ, vỡ cast sang privacy_level enum khi repo bind.
    let privacy_level = form.privacy_level.as_deref().unwrap_or("public");
    if !matches!(privacy_level, "public" | "anonymous") {
        return Err(AppError::BadRequest(
            "Privacy level phải là 'public' hoặc 'anonymous'".into(),
        ));
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
        privacy_level,
        accent_color,
        form.bio.as_deref().unwrap_or(""),
        form.avatar_url.as_deref(),
    )
    .await?;
    // v3.6.2 — AI Agent có namespace hồ sơ riêng /ai/{username}
    Ok(Redirect::to(&format!("/ai/{}", user.username)))
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
    /// v3.5.0 — đầy đủ khai báo tham số + tham số kích hoạt của chính agent
    /// (kể cả param riêng tư — agent có quyền thấy toàn bộ của mình):
    /// mỗi phần tử `{key, value, group, description, is_public}`.
    #[serde(default)]
    pub params: Vec<AiInfoParam>,
}

/// View 1 tham số trong /ai/info (v3.5.0).
#[derive(Debug, Serialize)]
pub struct AiInfoParam {
    pub key: String,
    pub value: String,
    /// "spec" | "activation"
    pub group: String,
    pub description: String,
    pub is_public: bool,
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
    // v3.5.0 — agent thấy TOÀN BỘ tham số của mình (kể cả riêng tư).
    let params: Vec<AiInfoParam> = AiAgentRepo::list_params(&state.db, user.id, false)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|p| AiInfoParam {
            key: p.param_key,
            value: p.param_value,
            group: p.param_group,
            description: p.description,
            is_public: p.is_public,
        })
        .collect();
    Ok(axum::response::Json(AiInfoResponse {
        success: true,
        user_id: user.id.to_string(),
        username: user.username,
        display_name: user.display_name,
        model_name: profile.model_name,
        vendor: profile.vendor,
        verified: profile.verified,
        params,
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

/// v3.5.1 — Validate email format tối giản nhưng chặt (audit 5-e F10):
/// local-part@domain, local-part 1-64 ký tự [a-zA-Z0-9._%+-], domain
/// label [a-zA-Z0-9-] + TLD ≥2 chữ cái, không khoảng trắng/ký tự điều
/// khiển. Không dùng regex crate (tránh dependency mới) — pure parser.
/// Mục đích: chặn địa chỉ rác dùng site làm spam relay, KHÔNG phải
/// validate RFC 5322 đầy đủ.
fn is_valid_email(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    // Local part: 1..=64, charset an toàn
    if local.is_empty() || local.len() > 64 {
        return false;
    }
    if !local
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '%' | '+' | '-'))
    {
        return false;
    }
    // Domain: ít nhất 1 label + TLD, tổng ≤ 190 (254 - local - @)
    if domain.is_empty() || domain.len() > 190 || !domain.contains('.') {
        return false;
    }
    if domain
        .chars()
        .any(|c| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'))
    {
        return false;
    }
    // TLD cuối ≥ 2 chữ cái, không label rỗng (a..b) hoặc bắt đầu/kết thúc '-'
    domain
        .split('.')
        .all(|label| !label.is_empty() && !label.starts_with('-') && !label.ends_with('-'))
        && domain
            .rsplit('.')
            .next()
            .is_some_and(|tld| tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()))
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

    /// v3.5.1 — email format validation (audit 5-e F10).
    #[test]
    fn test_is_valid_email() {
        // Hợp lệ
        assert!(is_valid_email("name@example.com"));
        assert!(is_valid_email("first.last+tag@sub.domain.io"));
        assert!(is_valid_email("a_b%test@my-domain.vn"));
        // Rác / spam relay vector
        assert!(!is_valid_email("no-at-sign"));
        assert!(!is_valid_email("@nodomain.com"));
        assert!(!is_valid_email("user@"));
        assert!(!is_valid_email("user@localhost"));
        assert!(!is_valid_email("user name@example.com"));
        assert!(!is_valid_email("user@exa mple.com"));
        assert!(!is_valid_email("user@example.c"));
        assert!(!is_valid_email("user@-bad-.com"));
        assert!(!is_valid_email("user@exa..mple.com"));
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
