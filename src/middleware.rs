use crate::auth::{hash_token, SESSION_COOKIE};
use crate::error::AppError;
use crate::models::user::User;
use crate::repositories::{AiAgentRepo, SessionRepo, UserRepo};
use crate::state::AppState;
use axum::{
    extract::{ConnectInfo, FromRef, FromRequestParts, Request, State},
    http::{request::Parts, HeaderValue, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Response},
};
use axum_extra::extract::CookieJar;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use uuid::Uuid;

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
    // Cập nhật last_seen_at tối đa 1 lần/giờ/user (throttle in-memory):
    // trước đây chỉ cập nhật lúc đăng nhập → hồ sơ 'hoạt động lần cuối'
    // và sitemap xếp theo hoạt động đều dùng dữ liệu stale cả tháng.
    touch_last_seen(state, &user);
    Some(user)
}

/// Throttle map cho việc cập nhật last_seen_at: user_id → lần cập nhật
/// gần nhất. Tránh ghi DB mỗi request (CurrentUser extractor có thể chạy
/// nhiều lần cho cùng request qua các middleware khác nhau).
static LAST_SEEN_THROTTLE: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<Uuid, std::time::Instant>>,
> = std::sync::OnceLock::new();

/// Đánh dấu user đang hoạt động — ghi DB nếu đã hơn 1 giờ từ lần gần nhất.
/// Best-effort: lỗi DB không ảnh hưởng request.
fn touch_last_seen(state: &AppState, user: &User) {
    // Chỉ update khi DB cho thấy đã stale > 1h (tránh spam map với user
    // thường xuyên quay lại trong phiên ngắn).
    let stale = user
        .last_seen_at
        .map(|t| {
            chrono::Utc::now()
                .signed_duration_since(t)
                .num_hours()
                .abs()
                >= 1
        })
        .unwrap_or(true);
    if !stale {
        return;
    }
    let map = LAST_SEEN_THROTTLE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    // Khôi phục từ poison — throttle không đáng để panic cả server.
    let mut map = map.lock().unwrap_or_else(|e| e.into_inner());
    let now = std::time::Instant::now();
    match map.get(&user.id) {
        Some(last) if now.duration_since(*last) < std::time::Duration::from_secs(3600) => {
            // Đã ghi trong giờ này — bỏ qua
        }
        _ => {
            map.insert(user.id, now);
            // Dọn map khi phình (nhiều user độc đáo ghé thăm)
            if map.len() > 5000 {
                map.retain(|_, t| now.duration_since(*t) < std::time::Duration::from_secs(3600));
            }
            let db = state.db.clone();
            let uid = user.id;
            tokio::spawn(async move {
                let _ = UserRepo::update_last_seen(&db, uid).await;
            });
        }
    }
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
        "/ai", // AI Agent vẫn có thể báo cáo tiến trình trong lúc maintenance
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

    /// true nếu cho phép qua; false nếu vượt giới hạn.
    ///
    /// Dọn entry rỗng mỗi ~256 request để tránh rò rỉ bộ nhớ khi có nhiều
    /// IP khác nhau ghé thăm rồi rời đi (entry tồn tại với timestamp cũ
    /// không bao giờ bị xoá nếu chỉ đợi `map.len() > 10_000`).
    pub fn check(&self, key: &str, max_requests: usize, window_secs: u64) -> bool {
        // Khôi phục từ mutex poison thay vì propagate panic — rate limit
        // không nên bring down toàn bộ server.
        let mut map = self.hits.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let window = std::time::Duration::from_secs(window_secs);
        let entry = map.entry(key.to_string()).or_default();
        entry.retain(|t| now.duration_since(*t) < window);
        let allowed = entry.len() < max_requests;
        if allowed {
            entry.push(now);
        }
        // Dọn map định kỳ tránh rò rỉ bộ nhớ. Giảm threshold từ 10_000
        // xuống 4_000 để dọn thường hơn, và xoá cả entry rỗng (không
        // chỉ entry có timestamp cuối ngoài cửa sổ).
        if map.len() > 4_000 {
            map.retain(|_, v| !v.is_empty() && now.duration_since(*v.last().unwrap()) < window);
        }
        allowed
    }
}

#[cfg(test)]
mod rate_limiter_tests {
    use super::*;

    #[test]
    fn test_allows_under_limit() {
        let rl = RateLimiter::new();
        for i in 0..5 {
            assert!(rl.check("ip1", 5, 60), "request #{} phải được phép", i + 1);
        }
    }

    #[test]
    fn test_blocks_over_limit() {
        let rl = RateLimiter::new();
        for _ in 0..3 {
            assert!(rl.check("ip2", 3, 60));
        }
        // Request thứ 4 trong cùng cửa sổ → bị chặn
        assert!(!rl.check("ip2", 3, 60));
        // Vẫn bị chặn ở request sau đó
        assert!(!rl.check("ip2", 3, 60));
    }

    #[test]
    fn test_keys_are_independent() {
        let rl = RateLimiter::new();
        for _ in 0..2 {
            assert!(rl.check("ipA", 2, 60));
        }
        assert!(!rl.check("ipA", 2, 60));
        // IP khác không bị ảnh hưởng bởi IP A
        assert!(rl.check("ipB", 2, 60));
    }

    #[test]
    fn test_window_expiry_frees_quota() {
        let rl = RateLimiter::new();
        // Cửa sổ 0 giây: mọi timestamp đều "quá hạn" ngay lập tức
        // (duration_since >= window) → quota tự do lại.
        assert!(rl.check("ipC", 1, 0));
        assert!(rl.check("ipC", 1, 0));
        assert!(rl.check("ipC", 1, 0));
    }

    #[test]
    fn test_reject_then_recover_after_window() {
        let rl = RateLimiter::new();
        assert!(rl.check("ipD", 1, 60));
        assert!(!rl.check("ipD", 1, 60));
        // Sau khi "hết hạn" (dùng window 0) → cho phép lại
        assert!(rl.check("ipD", 1, 0));
    }
}

#[cfg(test)]
mod path_normalization_tests {
    use super::normalize_path_for_rate_limit as norm;

    #[test]
    fn slug_rotation_cannot_bypass_comment_limit() {
        // 2 slug khác nhau phải cho CÙNG key bucket — spammer xoay slug
        // không còn tạo bucket mới để vượt giới hạn 10 comment/phút.
        assert_eq!(
            norm("/games/game-a/comments"),
            norm("/games/game-zzz/comments")
        );
    }

    #[test]
    fn slug_rotation_cannot_bypass_download_limit() {
        assert_eq!(norm("/games/abc/download"), norm("/games/xyz/download"));
    }

    #[test]
    fn uuid_segments_normalized() {
        let id1 = "550e8400-e29b-41d4-a716-446655440000";
        let id2 = "123e4567-e89b-12d3-a456-426614174000";
        assert_eq!(
            norm(&format!("/comments/{}/like", id1)),
            norm(&format!("/comments/{}/like", id2))
        );
        assert_eq!(
            norm(&format!("/comments/{}/like", id1)),
            "/comments/{x}/like"
        );
    }

    #[test]
    fn static_endpoints_keep_own_bucket() {
        // Endpoint tĩnh khác nhau → bucket khác nhau (không gộp oan)
        assert_ne!(norm("/games/latest"), norm("/games/trending"));
        assert_ne!(norm("/api/suggest"), norm("/api/check-duplicate"));
        assert_ne!(norm("/auth/google"), norm("/auth/logout"));
    }

    #[test]
    fn usernames_and_category_slugs_normalized() {
        // /u/{username} — mọi user chung bucket follow theo IP
        assert_eq!(norm("/u/alice/follow"), norm("/u/bob/follow"));
        assert_eq!(norm("/u/alice/follow"), "/u/{x}/follow");
        // /c/{slug} và /t/{slug}
        assert_eq!(norm("/c/hanh-dong"), norm("/c/giai-tri"));
        assert_eq!(norm("/t/2d"), "/t/{x}");
    }

    #[test]
    fn root_and_admin_paths() {
        assert_eq!(norm("/"), "/");
        // /admin — segment tĩnh giữ nguyên
        assert_eq!(norm("/admin"), "/admin");
        // /admin/users/{uuid}/ban → UUID bị chuẩn hoá
        let r = norm("/admin/users/550e8400-e29b-41d4-a716-446655440000/ban");
        assert_eq!(r, "/admin/users/{x}/ban");
    }

    #[test]
    fn trailing_slash_normalized() {
        // Slash cuối không tạo bucket khác
        assert_eq!(norm("/games/foo/comments/"), norm("/games/foo/comments"));
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

/// Chuẩn hoá đường dẫn thành "endpoint bucket" cho rate limit: thay mọi
/// segment động (slug game, username, UUID, category slug...) bằng `{x}`.
///
/// TRƯỚC ĐÂY (bug): key = `ip + path đầy đủ` → `/games/a/comments` và
/// `/games/b/comments` là 2 bucket khác nhau. Spammer xoay slug (có sẵn
/// danh sách 10.000 game) thì mỗi bucket riêng 10 req/phút → giới hạn
/// 10 bình luận/phút thực tế là VÔ HẠN. Tương tự `/games/{slug}/download`.
///
/// Sau fix: mọi slug đổi thành `{x}` → `/games/{x}/comments` chung một
/// bucket theo IP. Segment tĩnh được giữ nguyên để các endpoint khác
/// (GET /games/latest, POST /games) vẫn có bucket riêng đúng nghĩa.
///
/// An toàn theo hướng fail-closed: segment lạ (không nằm trong whitelist
/// từ routes.rs) được coi là động và chuẩn hoá — nếu route mới quên thêm
/// keyword thì bucket gộp chung (hơi gắt) chứ không bao giờ nới lỏng.
pub fn normalize_path_for_rate_limit(path: &str) -> String {
    /// Các segment tĩnh của router (routes.rs). Thêm keyword khi thêm route.
    const STATIC_SEGMENTS: &[&str] = &[
        // resources & actions chung
        "games",
        "comments",
        "repos",
        "users",
        "categories",
        "tags",
        "notifications",
        "bookmarks",
        "search",
        "profile",
        "settings",
        "sessions",
        "audit",
        "export",
        // game actions
        "new",
        "edit",
        "delete",
        "publish",
        "download",
        "report-form",
        "report",
        "like",
        "bookmark",
        "rate",
        "share",
        "replies",
        "hide",
        "feature",
        "pin",
        "resolve",
        "refresh",
        "role",
        "ban",
        "save",
        "status",
        "revoke",
        // danh sách đặc biệt
        "latest",
        "trending",
        "top-rated",
        "downloads",
        "featured",
        "my-games",
        "mark-all-read",
        "read",
        "broadcast",
        "related",
        "stats",
        // auth
        "auth",
        "google",
        "callback",
        "logout",
        "logout-all",
        "login",
        "register",
        // tiền tố & trang tĩnh
        "api",
        "v1",
        "ai",
        "admin",
        "u",
        "c",
        "t",
        "my-games",
        "ai-agents",
        "ai-reports",
        "progress",
        "progress.json",
        "info",
        "terms",
        "privacy",
        "health",
        "maintenance",
        "check-duplicate",
        "suggest",
        "preferences",
        "theme",
        "announcement",
        "static",
        "feed",
        "follow",
    ];
    let mut out = String::with_capacity(path.len());
    for seg in path.split('/') {
        if seg.is_empty() {
            continue;
        }
        if !STATIC_SEGMENTS.contains(&seg) {
            out.push_str("/{x}");
        } else {
            out.push('/');
            out.push_str(seg);
        }
    }
    if out.is_empty() {
        out.push('/');
    }
    out
}

/// Middleware giới hạn tốc độ cho các endpoint nhạy cảm
pub async fn rate_limit(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    let path = request.uri().path().to_string();
    // Lấy IP thật của client qua ConnectInfo (được axum thêm vào request
    // extensions khi dùng into_make_service_with_connect_info). Nếu chạy sau
    // proxy (Traefik/Coolify), ưu tiên header X-Forwarded-For / X-Real-IP /
    // CF-Connecting-IP do proxy đặt. Nếu không có proxy, dùng IP TCP gốc.
    let connect_info = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .copied();
    let ip = client_ip_from_parts(request.headers(), connect_info.map(|ci| ci.0).as_ref());
    // Tăng giới hạn nghiêm ngặt cho các endpoint AI Agent & auth để
    // chống brute-force token hoặc spam progress.
    // - /auth/ai/register: 5 / 10 phút (rất hiếm, chỉ AI admin mới gọi)
    // - /auth/ai/login:    10 / 10 phút (chống brute-force token)
    // - /ai/progress:      120 / phút  (AI báo cáo thường xuyên)
    // - /auth/google:      10 / 10 phút (chống lạm dụng OAuth)
    // - /download:         20 / phút
    // - /comments:         10 / phút
    let (max, window) = if path.starts_with("/auth/ai/register") {
        (5, 600)
    } else if path.starts_with("/auth/ai/login") || path.starts_with("/auth/google") {
        (10, 600)
    } else if path.starts_with("/ai/") {
        (120, 60)
    } else if path.starts_with("/api/suggest") {
        // Autocomplete: debounce 250ms client-side → gõ liên tục 1 phút
        // có thể phát ~200 request. Ngưỡng mặc định 120/phút sẽ chặn giữa
        // chừng người dùng đang gõ. 240/phút vẫn chặn spam script tốt.
        (240, 60)
    } else if path.contains("/download") {
        (20, 60) // 20 download/phút
    } else if path.contains("/comments") {
        (10, 60) // 10 bình luận/phút
    } else {
        (120, 60)
    };
    if !state.rate_limiter.check(
        // Key theo path ĐÃ CHUẨN HOÁ: xoay slug/UUID không tạo bucket mới
        // (xem normalize_path_for_rate_limit). Giới hạn áp theo endpoint
        // bucket + IP, không theo URL đầy đủ.
        &format!("{}{}", ip, normalize_path_for_rate_limit(&path)),
        max,
        window,
    ) {
        tracing::warn!("Rate limit exceeded: {} {} ({}/{})", ip, path, max, window);
        // Retry-After (RFC 6585/9110): báo client chờ bao lâu trước khi
        // thử lại — HTMX app.js / curl đều đọc được, tránh spam 429 liên tục.
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            [
                (axum::http::header::RETRY_AFTER, window.to_string()),
                (axum::http::header::CACHE_CONTROL, "no-store".to_string()),
            ],
            "Too Many Requests - vui lòng thử lại sau.",
        )
            .into_response());
    }
    Ok(next.run(request).await)
}

// ============================================================
// Security headers middleware (tăng bảo mật toàn site)
//
// Áp dụng cho mọi response. Đặc biệt:
// - X-Frame-Options: DENY — chống clickjacking (không cho nhúng iframe)
// - X-Content-Type-Options: nosniff — chống MIME sniffing
// - Referrer-Policy: strict-origin-when-cross-origin — rò rỉ referer tối thiểu
// - Permissions-Policy: tắt FCM, microphone, camera, geolocation (chỉ site cơ bản)
// - Strict-Transport-Security: bắt buộc HTTPS (chỉ có hiệu lực khi đã ở HTTPS)
// - Cross-Origin-Opener-Policy: same-origin — cô lập context browsing
// ============================================================
pub async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    // Chống clickjacking
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    // Chống MIME sniffing
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    // Referrer policy
    headers.insert(
        "referrer-policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    // Permissions policy (tắt các API nhạy cảm)
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static(
            "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()",
        ),
    );
    // Cross-origin isolation (chống side-channel / spectre-like)
    headers.insert(
        "cross-origin-opener-policy",
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("same-origin"),
    );
    // HSTS: 1 năm, includeSubDomains, preload (chỉ phát huy tác dụng khi ở HTTPS)
    headers.insert(
        "strict-transport-security",
        HeaderValue::from_static("max-age=31536000; includeSubDomains; preload"),
    );
    // COEP:.require-corp sẽ chặn ảnh flickr/imgur v.v. → dùng unsafe-inline CSP
    // cởi hơn cho nội dung động. Đặt CSP chung:
    //   default-src 'self' (chỉ cho phép nội dung từ chính site)
    //   script-src 'self' 'unsafe-inline' (htmx + app.js inline được)
    //   style-src 'self' 'unsafe-inline' (style nội tuyến)
    //   img-src 'self' https: data: (avatar từ Google + placeholder)
    //   font-src 'self' https://fonts.gstatic.com (Google Fonts)
    //   connect-src 'self' (htmx không gọi cross-origin)
    //   frame-ancestors 'none' (chống nhúng iframe)
    //   base-uri 'self'
    //   form-action 'self' (form chỉ submit về chính site)
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; img-src 'self' https: data:; font-src 'self' https://fonts.gstatic.com; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'; object-src 'none'"
        ),
    );
    response
}

// ============================================================
// AI Agent auth middleware: chỉ cho phép AI Agent (role=ai_agent)
// qua. Dùng cho các endpoint /ai/* (report progress, v.v.).
//
// Ưu tiên kiểm tra Authorization: Bearer <token> trước (lấy từ header).
// Nếu không có, fallback sang session cookie (AI đã đăng nhập qua web).
// ============================================================
pub async fn require_ai_agent(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // 1) Thử Authorization: Bearer <token>
    let auth_header = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let user_opt = if let Some(h) = auth_header.as_deref() {
        if let Some(token) = h
            .strip_prefix("Bearer ")
            .or_else(|| h.strip_prefix("bearer "))
        {
            let token = token.trim();
            if token.is_empty() {
                None
            } else {
                AiAgentRepo::find_by_api_token(&state.db, token)
                    .await
                    .ok()
                    .flatten()
                    .map(|(u, _)| u)
            }
        } else {
            None
        }
    } else {
        // 2) Fallback: session cookie
        let jar = CookieJar::from_headers(request.headers());
        current_user_from_jar(&state, &jar)
            .await
            .filter(|u| u.role.is_ai_agent())
    };

    let user = user_opt.ok_or(StatusCode::UNAUTHORIZED)?;
    if !user.role.is_ai_agent() {
        return Err(StatusCode::FORBIDDEN);
    }
    if user.is_banned {
        return Err(StatusCode::FORBIDDEN);
    }
    // Lưu user vào request extensions để handler lấy ra qua AuthAiAgent extractor
    let mut request = request;
    request.extensions_mut().insert(user);
    Ok(next.run(request).await)
}

/// Extractor lấy AI Agent user (đã được middleware `require_ai_agent` xác thực)
#[derive(Clone)]
pub struct AuthAiAgent(pub User);

impl<S> FromRequestParts<S> for AuthAiAgent
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<User>()
            .filter(|u| u.role.is_ai_agent())
            .map(|u| AuthAiAgent(u.clone()))
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}
