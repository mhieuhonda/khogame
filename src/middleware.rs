use crate::auth::{hash_token, SESSION_COOKIE};
use crate::error::AppError;
use crate::models::user::User;
use crate::repositories::{SessionRepo, SettingsRepo, UserRepo};
use crate::state::AppState;
use axum::{
    extract::{ConnectInfo, FromRef, FromRequestParts, Request, State},
    http::{request::Parts, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Response},
};
use axum_extra::extract::CookieJar;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

impl FromRef<Arc<AppState>> for AppState {
    fn from_ref(state: &Arc<AppState>) -> Self {
        (**state).clone()
    }
}

/// Extracts the current user from the request, if any.
pub async fn current_user_from_jar(state: &AppState, jar: &CookieJar) -> Option<User> {
    let token = jar.get(SESSION_COOKIE)?.value().to_string();
    let token_hash = hash_token(&token);
    let user_id = SessionRepo::find_user_by_token(&state.db, &token_hash)
        .await
        .ok()??;
    let user = UserRepo::find_by_id(&state.db, user_id).await.ok()??;
    if user.is_banned {
        return None;
    }
    Some(user)
}

/// Optional current user extractor - returns Option<User>
#[derive(Clone)]
pub struct CurrentUser(pub Option<User>);

impl<S> FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
    Arc<AppState>: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state: Arc<AppState> = Arc::from_ref(state);
        let jar = CookieJar::from_request_parts(parts, state)
            .await
            .map_err(|_| AppError::Internal(anyhow::anyhow!("Failed to extract cookies")))?;
        let user = current_user_from_jar(&app_state, &jar).await;
        Ok(CurrentUser(user))
    }
}

/// Required current user extractor - errors if not authenticated
#[derive(Clone)]
pub struct AuthUser(pub User);

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    Arc<AppState>: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = CurrentUser::from_request_parts(parts, state)
            .await?
            .0
            .ok_or(AppError::Unauthorized)?;
        Ok(AuthUser(user))
    }
}

/// Admin-only middleware
pub async fn require_admin(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract cookies from request headers
    let jar = CookieJar::from_headers(request.headers());
    let user = current_user_from_jar(&state, &jar)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if !user.role.is_staff() {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(next.run(request).await)
}

// ============================================================
// Maintenance mode: chặn người thường, admin vẫn qua được
// ============================================================
pub async fn maintenance_guard(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    let bypass_prefixes = [
        "/admin",
        "/login",
        "/auth",
        "/static",
        "/api/v1/health",
        "/health",
        "/maintenance",
        "/api/announcement",
        "/api/preferences",
    ];
    if bypass_prefixes.iter().any(|p| path.starts_with(p)) {
        return next.run(request).await;
    }
    let on = state.maintenance_enabled().await;
    if !on {
        return next.run(request).await;
    }
    // Staff bypass
    let jar = CookieJar::from_headers(request.headers());
    if let Some(user) = current_user_from_jar(&state, &jar).await {
        if user.role.is_staff() {
            return next.run(request).await;
        }
    }
    Html(
        r#"<!DOCTYPE html>
<html lang="vi" data-theme="dark">
<head><meta charset="UTF-8"><title>Bảo trì - Kho Game</title>
<link rel="stylesheet" href="/static/css/style.css"></head>
<body>
<main class="site-main"><div class="container" style="text-align:center;padding:80px 16px">
<div style="font-size:72px">🛠️</div>
<h1>Hệ thống đang bảo trì</h1>
<p>Kho Game tạm thời ngừng phục vụ để nâng cấp. Vui lòng quay lại sau ít phút.</p>
<p><a class="btn btn-primary" href="/">Thử lại</a></p>
</div></main>
</body></html>"#,
    )
    .into_response()
}

// ============================================================
// Rate limiter đơn giản (token bucket theo IP)
// ============================================================
use std::time::Instant;

#[derive(Default)]
pub struct RateLimiter {
    /// key -> danh sách timestamp các request trong cửa sổ
    hits: std::sync::Mutex<HashMap<String, Vec<Instant>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// true nếu cho phép qua; false nếu vượt giới hạn
    pub fn check(&self, key: &str, max_requests: usize, window_secs: u64) -> bool {
        let mut map = self.hits.lock().unwrap();
        let now = Instant::now();
        let window = std::time::Duration::from_secs(window_secs);
        let entry = map.entry(key.to_string()).or_default();
        entry.retain(|t| now.duration_since(*t) < window);
        if entry.len() >= max_requests {
            false
        } else {
            entry.push(now);
            // Dọn map định kỳ tránh rò rỉ bộ nhớ
            if map.len() > 10_000 {
                map.retain(|_, v| now.duration_since(v[v.len() - 1]) < window);
            }
            true
        }
    }
}

/// Lấy IP client từ headers proxy phổ biến (Coolify/Traefik) hoặc ConnectInfo
pub fn client_ip_from_parts(
    headers: &axum::http::HeaderMap,
    connect_info: Option<&SocketAddr>,
) -> String {
    for h in ["x-forwarded-for", "x-real-ip", "cf-connecting-ip"] {
        if let Some(v) = headers.get(h).and_then(|v| v.to_str().ok()) {
            return v.split(',').next().unwrap_or("unknown").trim().to_string();
        }
    }
    connect_info
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| "unknown".into())
}

/// Middleware giới hạn tốc độ cho các endpoint nhạy cảm
pub async fn rate_limit(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = request.uri().path().to_string();
    let ip = client_ip_from_parts(request.headers(), None);
    let (max, window) = if path.contains("/download") {
        (20, 60) // 20 download/phút
    } else if path.contains("/comments") {
        (10, 60) // 10 bình luận/phút
    } else {
        (120, 60)
    };
    if !state
        .rate_limiter
        .check(&format!("{}:{}", ip, path), max, window)
    {
        tracing::warn!("Rate limit exceeded: {} {}", ip, path);
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    Ok(next.run(request).await)
}

#[allow(dead_code)]
pub async fn get_client_ip(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> String {
    addr.ip().to_string()
}

#[allow(dead_code)]
pub async fn seed_admin_email(state: &AppState) {
    let email = &state.config.admin_email;
    if email.is_empty() {
        return;
    }
    match UserRepo::ensure_admin_by_email(&state.db, email).await {
        Ok(true) => tracing::info!("Seeded admin role for {}", email),
        Ok(false) => {}
        Err(e) => tracing::warn!("Failed seeding admin: {}", e),
    }
    let _ = SettingsRepo::set(&state.db, "admin_email", email, None).await;
}
