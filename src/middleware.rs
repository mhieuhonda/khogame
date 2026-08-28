use crate::auth::{hash_token, SESSION_COOKIE};
use crate::error::AppError;
use crate::models::user::User;
use crate::repositories::{AiAgentRepo, SessionRepo, UserRepo};
use crate::state::AppState;
use askama::Template as _;
use axum::{
    extract::{ConnectInfo, FromRef, FromRequestParts, Request, State},
    http::{request::Parts, HeaderName, HeaderValue, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Response},
};
use axum_extra::extract::CookieJar;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

impl FromRef<Arc<Self>> for AppState {
    fn from_ref(state: &Arc<Self>) -> Self {
        (**state).clone()
    }
}

/// Cache session user ngắn hạn (v2.1.0 PERF): `token_hash` → `(User, khi cache)`.
///
/// Mỗi request của user đã đăng nhập tốn 2 query DB (session→user_id,
/// user_id→user) chỉ để xác thực — trước đây lặp lại cho MỌI request
/// (trang + HTMX partials + API). Cache 10 giây cắt 2 query này khỏi
/// hot path: các request liên tiếp trong 10s (điển hình: 1 trang web
/// bắn 5-15 request song song) dùng chung 1 lần lookup.
///
/// Đánh đổi: thay đổi quyền/ban có thể trễ tối đa 10s. ĐỀU bị invalidation
/// chủ động phủ kín: logout, logout-all, admin revoke session, admin set
/// role/ban (gọi `invalidate_session_cache_for_user`).
/// Kiểu map cache session — tách alias để tránh clippy::type_complexity.
type SessionCacheMap = std::collections::HashMap<String, (Arc<User>, std::time::Instant)>;

static SESSION_CACHE: std::sync::OnceLock<std::sync::Mutex<SessionCacheMap>> =
    std::sync::OnceLock::new();

/// TTL cache session — đủ dài để phục vụ burst request của 1 page load,
/// đủ ngắn để thay đổi quyền lan truyền gần như tức thời.
const SESSION_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(10);

/// Xoá 1 session khỏi cache (key = token hash). Gọi khi logout /
/// revoke — user bị đá ra NGAY LẬP TỨC, không đợi TTL.
pub fn invalidate_session_cache(token_hash: &str) {
    if let Some(map) = SESSION_CACHE.get() {
        let mut map = map
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.remove(token_hash);
    }
}

/// Xoá MỌI session của 1 user khỏi cache (logout-all, admin đổi role/ban).
pub fn invalidate_session_cache_for_user(user_id: Uuid) {
    if let Some(map) = SESSION_CACHE.get() {
        let mut map = map
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.retain(|_, (u, _)| u.id != user_id);
    }
}

/// Extracts the current user from the request, if any.
///
/// DB path: session token (hash) → user_id → user. Cache path: hit trong
/// 10s thì trả thẳng không đụng DB (xem SESSION_CACHE).
pub async fn current_user_from_jar(state: &AppState, jar: &CookieJar) -> Option<User> {
    let token = jar.get(SESSION_COOKIE)?.value().to_string();
    let token_hash = hash_token(&token);

    // === Fast path: cache hit (không chạm DB) ===
    if let Some(map) = SESSION_CACHE.get() {
        let map = map
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((user, cached_at)) = map.get(&token_hash) {
            if cached_at.elapsed() < SESSION_CACHE_TTL {
                if user.is_banned {
                    return None;
                }
                let user = (**user).clone();
                drop(map);
                touch_last_seen(state, &user);
                return Some(user);
            }
        }
    }

    // === Slow path: query DB như cũ ===
    let user_id = SessionRepo::find_user_by_token(&state.db, &token_hash)
        .await
        .ok()??;
    let user = UserRepo::find_by_id(&state.db, user_id).await.ok()??;
    if user.is_banned {
        return None;
    }
    // Lưu vào cache cho các request kế tiếp trong cửa sổ TTL.
    // Cleanup khi map phình (nhiều user ghé trong 10s) — retain entry còn hạn.
    if let Some(map) = SESSION_CACHE.get() {
        let mut map = map
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if map.len() > 5_000 {
            map.retain(|_, (_, at)| at.elapsed() < SESSION_CACHE_TTL);
        }
        map.insert(
            token_hash,
            (Arc::new(user.clone()), std::time::Instant::now()),
        );
    }
    // Cập nhật last_seen_at tối đa 1 lần/giờ/user (throttle in-memory):
    // trước đây chỉ cập nhật lúc đăng nhập → hồ sơ 'hoạt động lần cuối'
    // và sitemap xếp theo hoạt động đều dùng dữ liệu stale cả tháng.
    touch_last_seen(state, &user);
    Some(user)
}

/// Throttle map cho việc cập nhật `last_seen_at`: `user_id` → lần cập nhật
/// gần nhất. Tránh ghi DB mỗi request (`CurrentUser` extractor có thể chạy
/// nhiều lần cho cùng request qua các middleware khác nhau).
static LAST_SEEN_THROTTLE: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<Uuid, std::time::Instant>>,
> = std::sync::OnceLock::new();

/// Đánh dấu user đang hoạt động — ghi DB nếu đã hơn 1 giờ từ lần gần nhất.
/// Best-effort: lỗi DB không ảnh hưởng request.
fn touch_last_seen(state: &AppState, user: &User) {
    // Chỉ update khi DB cho thấy đã stale > 1h (tránh spam map với user
    // thường xuyên quay lại trong phiên ngắn).
    let stale = user.last_seen_at.is_none_or(|t| {
        chrono::Utc::now()
            .signed_duration_since(t)
            .num_hours()
            .abs()
            >= 1
    });
    if !stale {
        return;
    }
    let map = LAST_SEEN_THROTTLE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    // Khôi phục từ poison — throttle không đáng để panic cả server.
    let mut map = map
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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

/// Optional current user extractor - returns `Option<User>`
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
        Ok(Self(user))
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
        Ok(Self(user))
    }
}

/// Admin-only middleware
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
    let bypass_prefixes: [&str; 10] = [
        "/admin",
        "/login",
        "/auth",
        "/static",
        "/api/v1/health",
        "/health",
        "/maintenance",
        "/api/announcement",
        "/api/preferences",
        "/ai/",
    ];
    if bypass_prefixes
        .iter()
        .any(|p| path == p.trim_end_matches('/') || path.starts_with(p))
    {
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
<html lang="vi">
<head><meta charset="UTF-8"><title>Bảo trì - Louis Space</title>
<link rel="stylesheet" href="/static/css/style.css">
<script>(function(){try{var t=localStorage.getItem('ls-theme')||localStorage.getItem('kg-theme');if(t!=='dark'&&t!=='light'){t=window.matchMedia&&window.matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light';}document.documentElement.setAttribute('data-theme',t);}catch(e){document.documentElement.setAttribute('data-theme','light');}})();</script>
</head>
<body>
<main class="site-main"><div class="container" style="text-align:center;padding:80px 16px">
<div style="font-size:72px">🛠️</div>
<h1>Hệ thống đang bảo trì</h1>
<p>Louis Space tạm thời ngừng phục vụ để nâng cấp. Vui lòng quay lại sau ít phút.</p>
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
    #[must_use]
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
        let mut map = self
            .hits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = Instant::now();
        let window = std::time::Duration::from_secs(window_secs);
        let entry = map.entry(key.to_string()).or_default();
        entry.retain(|t| now.duration_since(*t) < window);
        // Cap vec size: nếu 1 IP gửi 10_000 req trong cửa sổ (trước khi
        // bị block lần đầu), vec có thể phình to. Truncate giữ đủ phần tử
        // để đo đạt ngưỡng, bỏ phần dư thừa (sẽ bị keep vì == max_requests).
        if entry.len() > max_requests * 2 {
            let drop_count = entry.len().saturating_sub(max_requests);
            // Cắt phần tử CŨ NHẤT để vec chỉ còn chứa các timestamp gần
            // đây — đúng cho việc đếm số request còn trong window.
            entry.drain(0..drop_count);
        }
        let allowed = entry.len() < max_requests;
        if allowed {
            entry.push(now);
        }
        // Dọn map định kỳ tránh rò rỉ bộ nhớ. Giảm threshold từ 10_000
        // xuống 4_000 để dọn thường hơn, và xoá cả entry rỗng (không
        // chỉ entry có timestamp cuối ngoài cửa sổ).
        if map.len() > 4_000 {
            map.retain(|_, v| {
                if v.is_empty() {
                    return false;
                }
                match v.last() {
                    Some(last) => now.duration_since(*last) < window,
                    None => false,
                }
            });
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
            norm(&format!("/comments/{id1}/like")),
            norm(&format!("/comments/{id2}/like"))
        );
        assert_eq!(norm(&format!("/comments/{id1}/like")), "/comments/{x}/like");
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

/// Lấy IP client từ headers proxy phổ biến (Coolify/Traefik) hoặc `ConnectInfo`.
///
/// `trust_proxy`: chỉ tin headers khi app chạy sau reverse proxy kiểm soát
/// được (Traefik/Coolify/CDN). Khi expose trực tiếp, header X-Forwarded-For
/// do CLIENT tự gắn là dữ liệu attacker điều khiển — dùng sẽ bị giả IP
/// để lách rate-limit. Tắt qua env `TRUST_PROXY_HEADERS=false`.
///
/// `hops`: số proxy TIN CẬY giữa client và app (env `TRUSTED_PROXY_HOPS`,
/// mặc định 1). X-Forwarded-For có dạng `client, proxy1, proxy2...` — mỗi
/// proxy append IP của hop TRƯỚC vào cuối chuỗi. Phần tử cuối cùng do
/// proxy gần app nhất thêm là IP của hop trước nó (không phải client nếu
/// có ≥2 proxy). Real client IP = phần tử thứ `hops` kể từ PHẢI sang trái:
/// - 1 proxy (Traefik trực tiếp): lấy phần tử cuối.
/// - 2 proxy (CDN/Cloudflare → Traefik): lấy phần tử KẾ TRƯỚC cuối — lấy
///   cuối sẽ ra IP edge của CDN, mọi user cùng một IP (bug observed trên
///   prod: toàn bộ session hiện cùng IP proxy).
#[must_use]
pub fn client_ip_from_parts(
    headers: &axum::http::HeaderMap,
    connect_info: Option<&SocketAddr>,
    trust_proxy: bool,
    hops: u8,
) -> String {
    let hops = hops.max(1) as usize;
    if trust_proxy {
        // hops=1 mới tin X-Real-IP / CF-Connecting-IP: các header này chỉ
        // mang 1 giá trị do proxy gần nhất ghi (IP của peer của nó). Khi
        // có ≥2 hop, giá trị đó là IP của proxy trung gian chứ không phải
        // client → bỏ qua để rơi vào nhánh XFF parse đúng hop bên dưới.
        if hops == 1 {
            for h in ["x-real-ip", "cf-connecting-ip"] {
                if let Some(v) = headers.get(h).and_then(|v| v.to_str().ok()) {
                    let ip = v.trim();
                    if is_valid_ip_string(ip) {
                        return ip.to_string();
                    }
                }
            }
        }
        // X-Forwarded-For: `client, proxy1, ...` — bỏ `hops` phần tử bên
        // phải (do các trusted proxy append), phần tử hợp lệ kế tiếp chính
        // là client. Walk từ phải sang trái, skip `hops-1` entry đầu tiên
        // gặp được, rồi lấy entry hợp lệ tiếp theo.
        if let Some(v) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            let valids: Vec<&str> = v
                .split(',')
                .map(|p| p.trim())
                .filter(|p| is_valid_ip_string(p))
                .collect();
            if !valids.is_empty() {
                // valids[len-1] = hop gần app nhất; client = valids[len-hops].
                // Nếu chuỗi ngắn hơn expected (proxy chỉ append 1 phần)
                // thì lấy phần tử ĐẦU — vẫn tốt hơn "unknown" và an toàn
                // vì phần tử đầu của XFF do proxy ngoài cùng ghi.
                let idx = valids.len().saturating_sub(hops);
                return valids[idx].to_string();
            }
        }
    }
    connect_info.map_or_else(|| "unknown".into(), |a| a.ip().to_string())
}

/// IP có phải private/loopback/link-local (RFC 1918/4193...) không?
///
/// Khi app chạy sau proxy KHÔNG truyền IP thật (TCP forwarding không
/// PROXY protocol — vd nginx stream giữa 2 VPS), mọi request tới app đều
/// mang IP private của tunnel/proxy: IP này KHÔNG phân biệt được user.
/// Rate limiter dựa vào IP sẽ gộp toàn bộ user vào 1 bucket chung
/// (một user spam = chặn cả site) — cần fallback key theo session cookie.
#[must_use]
pub fn is_private_ip(ip: &str) -> bool {
    match ip.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(v4)) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                // Docker network 172.17-31.x có overlap với RFC1918
                // 172.16/12 — is_private() đã cover. 169.254/16 link-local.
                || v4.is_unspecified()
        }
        Ok(std::net::IpAddr::V6(v6)) => {
            v6.is_loopback() || v6.is_unspecified() || matches!(v6.segments()[0] & 0xfe00, 0xfc00)
        }
        Err(_) => ip == "unknown", // không parse được → coi như không định danh được
    }
}

/// Kiểm tra chuỗi là IPv4/IPv6 hợp lệ, đồng thời ngầm giới hạn độ dài
/// (IPv4 tối đa 15 ký tự, IPv6 tối đa 45 ký tự). Tránh lưu 1MB header text
/// vào TEXT column news.author_ip / sessions.ip.
fn is_valid_ip_string(s: &str) -> bool {
    s.parse::<std::net::IpAddr>().is_ok()
}

/// Verify `Origin` (hoặc fallback `Referer`) khớp với `base_url` host.
///
/// Dùng cho POST endpoints không yêu cầu session hiện tại (vd `/auth/ai/login`,
/// `/auth/ai/register`) — SameSite=Lax cookie không bảo vệ được vì endpoint
/// không cần cookie hiện tại của nạn nhân. Cross-site form auto-submit sẽ bị
/// từ chối vì Origin không khớp.
///
/// Trả về `Ok(())` nếu Origin/Referer hợp lệ hoặc rỗng (không có header).
/// Trả về `Err(AppError::Forbidden)` nếu Origin/Referer có nhưng không khớp
/// base_url — fail-closed.
pub fn verify_origin(
    headers: &axum::http::HeaderMap,
    base_url: &str,
) -> crate::error::AppResult<()> {
    let base_host = base_url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("");
    if base_host.is_empty() {
        // Không xác định được host gốc → bỏ qua check (không fail-closed
        // để không block dev trên localhost).
        return Ok(());
    }
    // Origin hoặc Referer — Referer là fallback vì browser luôn gửi cho POST.
    // Origin có dạng `https://host[:port]` (không có path), Referer có path.
    let origin = headers
        .get(axum::http::header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string());
    let referer = headers
        .get(axum::http::header::REFERER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string());
    let check = |url: &str| -> bool {
        // Lấy host từ URL (sau scheme://) — so sánh case-insensitive với base_host.
        let host_part = url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap_or("");
        // Hỗ trợ cả `host:port` (dev localhost:3000) bằng cách bỏ phần port.
        let host_only = host_part.split(':').next().unwrap_or("");
        host_only.eq_ignore_ascii_case(base_host.split(':').next().unwrap_or(""))
    };
    match (origin.as_deref(), referer.as_deref()) {
        (Some(o), _) if !o.is_empty() && check(o) => Ok(()),
        (Some(_), Some(r)) if !r.is_empty() && check(r) => Ok(()),
        (None, Some(r)) if !r.is_empty() && check(r) => Ok(()),
        (Some(_), _) | (_, Some(_)) => {
            // Có Origin/Referer nhưng không khớp → từ chối.
            Err(AppError::Forbidden(
                "Yêu cầu không đến từ domain hợp lệ".into(),
            ))
        }
        (None, None) => {
            // Không có cả Origin lẫn Referer — request kỳ lạ (curl, legacy
            // browser). Cho phép qua để không phá tương thích curl test,
            // nhưng log để admin quan sát.
            tracing::debug!("POST không có Origin/Referer — cho phép qua (curl/legacy client)");
            Ok(())
        }
    }
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
#[must_use]
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
        "news-check-duplicate",
        "suggest",
        "preferences",
        "theme",
        "announcement",
        "static",
        "feed",
        "follow",
        // news module
        "news",
        "my-news",
        "pending",
        "all",
        "news.rss",
        "news-suggest",
    ];
    let mut out = String::with_capacity(path.len());
    for seg in path.split('/') {
        if seg.is_empty() {
            continue;
        }
        if STATIC_SEGMENTS.contains(&seg) {
            out.push('/');
            out.push_str(seg);
        } else {
            out.push_str("/{x}");
        }
    }
    if out.is_empty() {
        out.push('/');
    }
    out
}

/// Wrapper response 429 cho middleware `rate_limit` — Box bên trong để
/// Result<Response, _> nhỏ (clippy `result_large_err`), đồng thời impl
/// `IntoResponse` để dùng được làm Err-type của `axum::middleware::from_fn`.
pub struct RateLimited(Box<Response>);

impl IntoResponse for RateLimited {
    fn into_response(self) -> Response {
        *self.0
    }
}

/// Middleware giới hạn tốc độ cho các endpoint nhạy cảm
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn rate_limit(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, RateLimited> {
    let path = request.uri().path().to_string();
    // Lấy IP thật của client qua ConnectInfo (được axum thêm vào request
    // extensions khi dùng into_make_service_with_connect_info). Nếu chạy sau
    // proxy (Traefik/Coolify), ưu tiên header X-Forwarded-For / X-Real-IP /
    // CF-Connecting-IP do proxy đặt. Nếu không có proxy, dùng IP TCP gốc.
    let connect_info = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .copied();
    let ip = client_ip_from_parts(
        request.headers(),
        connect_info.map(|ci| ci.0).as_ref(),
        state.config.trust_proxy_headers,
        state.config.trusted_proxy_hops,
    );

    // === Fix shared-bucket khi proxy giấu IP thật ===
    // Prod hiện chạy: client → nginx stream (TCP, KHÔNG PROXY protocol)
    // → Traefik → app. IP client bị mất ở hop TCP — app thấy IP private
    // của tunnel cho MỌI user → toàn site chia chung rate-limit bucket
    // (một user spam = 429 cả site; một trang 50 reply lazy-load có thể
    // đốt 50/240 slot global). Khi IP là private/unknown, key bucket
    // theo định danh per-browser thay vì IP:
    //   1) Có session cookie (đã login) → hash cookie (ổn định/user).
    //   2) Có anon cookie (đã ghé trước đó) → dùng nguyên giá trị.
    //   3) Chưa có gì → sinh anon id mới, set vào response.
    // Khi hạ tầng truyền IP thật (PROXY protocol / CDN), IP là public →
    // key theo IP như cũ — hành vi cũ được bảo toàn.
    let mut set_anon_cookie: Option<String> = None;
    let bucket_identity = if is_private_ip(&ip) {
        warn_shared_ip_once(&ip);
        if let Some(token) = session_cookie_value(request.headers()) {
            format!("s:{}", &hash_token(&token)[..16])
        } else if let Some(anon) = anon_cookie_value(request.headers()) {
            format!("a:{anon}")
        } else {
            // Chưa có cookie nào → sinh anon id, set vào response;
            // bucket cho request NÀY đã dùng id mới (không đợi request
            // sau) để 1 browser spam liên tục vẫn bị giới hạn đúng.
            let anon = Uuid::new_v4().to_string();
            set_anon_cookie = Some(anon.clone());
            format!("a:{anon}")
        }
    } else {
        ip.clone()
    };

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
    } else if path.ends_with("/download") {
        // Chỉ match URL kết thúc bằng /download (POST /games/{slug}/download).
        // Trước đây dùng contains("/download") → match cả
        // /admin/games/{id}/delete?reason=/download — gán bucket
        // 20/phút cho admin action, không ảnh hưởng security nhưng
        // tạo bucket sai (admin bị giới hạn download thay vì admin).
        (20, 60) // 20 download/phút
    } else if path.ends_with("/replies") {
        // GET /comments/{id}/replies — lazy-load HTMX revealed. Một trang
        // game có 50 top-level comment sẽ bắn 50 GET đồng thời. Nếu gộp
        // vào bucket 10/phút của "comments" thì 40 request bị 429 + 40
        // toast "thao tác quá nhanh". Tách bucket riêng 240/phút cho read-only.
        (240, 60)
    } else if path.ends_with("/comments") || path.contains("/comments/") {
        // Match /games/{slug}/comments (POST create) và
        // /comments/{id}/like, /comments/{id}/edit, /comments/{id} (DELETE).
        // Tất cả là write action — giới hạn 10/phút.
        (10, 60) // 10 bình luận/phút
    } else {
        (120, 60)
    };
    if !state.rate_limiter.check(
        // Key theo path ĐÃ CHUẨN HOÁ: xoay slug/UUID không tạo bucket mới
        // (xem normalize_path_for_rate_limit). Giới hạn áp theo endpoint
        // bucket + identity, không theo URL đầy đủ.
        &format!(
            "{}{}",
            bucket_identity,
            normalize_path_for_rate_limit(&path)
        ),
        max,
        window,
    ) {
        tracing::warn!("Rate limit exceeded: {} {} ({}/{})", ip, path, max, window);
        // Retry-After (RFC 6585/9110): báo client chờ bao lâu trước khi
        // thử lại — HTMX app.js / curl đều đọc được, tránh spam 429 liên tục.
        let mut too_many = (
            StatusCode::TOO_MANY_REQUESTS,
            [
                (axum::http::header::RETRY_AFTER, window.to_string()),
                (axum::http::header::CACHE_CONTROL, "no-store".to_string()),
            ],
            "Too Many Requests - vui lòng thử lại sau.",
        )
            .into_response();
        // Set anon cookie ngay cả trên 429 — nếu không, request spam
        // đầu tiên không có cookie → response không set → request tiếp
        // theo lại được bucket mới → vô hiệu hoá hoàn toàn rate limit
        // với bot không cookie khi app sau proxy shared-IP.
        if let Some(anon) = &set_anon_cookie {
            let cookie =
                format!("{ANON_COOKIE}={anon}; Path=/; Max-Age=31536000; HttpOnly; SameSite=Lax");
            if let Ok(v) = HeaderValue::from_str(&cookie) {
                too_many
                    .headers_mut()
                    .append(axum::http::header::SET_COOKIE, v);
            }
        }
        return Err(RateLimited(too_many.into()));
    }
    let mut response = next.run(request).await;
    // Set anon id cookie cho browser chưa login (chỉ khi IP là proxy
    // private — xem comment bucket_identity bên trên). Cookie là UUID
    // ngẫu nhiên thuần chức năng (rate limit), không PII, HttpOnly +
    // SameSite=Lax, 1 năm — đủ dài để không reset khi user quay lại.
    if let Some(anon) = set_anon_cookie {
        let cookie =
            format!("{ANON_COOKIE}={anon}; Path=/; Max-Age=31536000; HttpOnly; SameSite=Lax");
        if let Ok(v) = HeaderValue::from_str(&cookie) {
            response
                .headers_mut()
                .append(axum::http::header::SET_COOKIE, v);
        }
    }
    Ok(response)
}

/// Đọc giá trị cookie theo tên từ header Cookie — không qua CookieJar
/// (rate_limit middleware cần giá trị thô để hash, không cần parse jar
/// đầy đủ; tránh clone toàn bộ jar mỗi request).
fn cookie_value(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    let raw = headers
        .get(axum::http::header::COOKIE)?
        .to_str()
        .ok()?
        .to_string();
    raw.split(';').find_map(|part| {
        let part = part.trim();
        part.strip_prefix(name)
            .and_then(|rest| rest.strip_prefix('='))
            .filter(|v| !v.is_empty())
            .map(std::borrow::ToOwned::to_owned)
    })
}

fn session_cookie_value(headers: &axum::http::HeaderMap) -> Option<String> {
    cookie_value(headers, SESSION_COOKIE)
}

/// Cookie anon cho visitor chưa login — chỉ được đọc/set khi rate-limit
/// cần fallback identity (IP private). Không PII, chỉ là UUID ngẫu nhiên.
const ANON_COOKIE: &str = "ls_anon";

fn anon_cookie_value(headers: &axum::http::HeaderMap) -> Option<String> {
    cookie_value(headers, ANON_COOKIE)
}

/// Log 1 lần duy nhất khi phát hiện mọi request dùng chung 1 IP private —
/// giúp operator đọc log hiểu tại sao admin hiện cùng IP cho mọi user và
/// rate-limit phải fallback theo cookie (xem docs/real-ip.md để bật
/// PROXY protocol lấy lại IP thật).
static SHARED_IP_WARNED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
fn warn_shared_ip_once(ip: &str) {
    if SHARED_IP_WARNED.set(()).is_ok() {
        tracing::warn!(
            "client IP = {ip} (private) cho MỌI request — app đang chạy sau proxy \
             KHÔNG truyền IP thật (nginx stream/L4 forwarding không PROXY protocol). \
             Admin sẽ thấy cùng 1 IP cho toàn bộ user; rate-limit đã tự fallback \
             key theo session/anon cookie. Để lấy lại IP thật: bật PROXY protocol \
             ở proxy ngoài + TRUSTED_PROXY_HOPS tương ứng (xem docs/real-ip.md)."
        );
    }
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
    //   connect-src 'self' ws: wss: (htmx + WebSocket cho Live Chat)
    //   frame-src https://www.youtube-nocookie.com https://www.youtube.com
    //     (trailer iframe — KHÔNG có frame-src thì fallback default-src 'self'
    //     chặn luôn YouTube embed, trailer không bao giờ load được)
    //   frame-ancestors 'none' (chống nhúng iframe)
    //   base-uri 'self'
    //   form-action 'self' (form chỉ submit về chính site)
    //   + manifest-src 'self', worker-src 'self' (v2.1.0 — chặn tighter
    //     fallback default-src cho manifest/worker future)
    //   LƯU Ý: KHÔNG dùng upgrade-insecure-requests — directive này upgrade
    //   cả subresource http://localhost trong môi trường dev (localhost là
    //   "potentially trustworthy" nên browser vẫn áp dụng) → CSS/JS bị request
    //   qua https://localhost → chết toàn bộ dev environment.
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; img-src 'self' https: data:; font-src 'self' https://fonts.gstatic.com; connect-src 'self' ws: wss:; frame-src https://www.youtube-nocookie.com https://www.youtube.com; frame-ancestors 'none'; base-uri 'self'; form-action 'self'; object-src 'none'; manifest-src 'self'; worker-src 'self'"
        ),
    );
    response
}

// ============================================================
// v2.3.0 PERF — ETag + Cache-Control cho HTML responses
//
// Mục tiêu: giảm băng thông + tăng tốc độ page view lần 2 bằng:
//   1) ETag (hash nội dung HTML) — browser gửi If-None-Match → server
//      trả 304 Not Modified (body rỗng, chỉ header) khi content không
//      đổi. Tiết kiệm ~50-200KB mỗi page view.
//   2) Cache-Control: public, max-age=60, stale-while-revalidate=600
//      cho anonymous (cookie session thiếu) — browser cache 1 phút,
//      SWR 10 phút. Vận dụng cache browser cho các page phổ biến (homepage).
//      cho user đã login: Cache-Control: private, no-cache (không cache
//      shared proxy, mỗi request revalidate) — tránh leak thông tin
//      user A sang user B.
//   3) Link: </static/css/style.css?v=...>; rel=preload; as=style —
//      HTTP/2 Early Hints (103) cho preload. Browser fetch CSS ngay
//      trước khi nhận HTML, FCP nhanh hơn rõ.
//
// KHÔNG áp dụng cho:
//   - API responses (Accept: application/json, hoặc path /api/*)
//   - HTMX partials (header HX-Request: true)
//   - Redirects (3xx) hoặc errors (4xx/5xx)
//   - Responses đã có ETag (vd: static file ETag do tower-http ServeDir)
//   - Non-GET (POST/PUT/PATCH/DELETE)
//   - Streaming/chunked responses (Content-Encoding không rõ)
//
// Layer ordering: NẰM TRONG security_headers (inner hơn) để mọi
// response — kể cả 304 Not Modified — đều được gắn CSP/HSTS/X-Frame.
// Nằm NGOÀI rate_limit + maintenance_guard + error_page_mw + origin_check
// để các layer đó short-circuit (429/503/403) không bị tốn công hash body.
// ============================================================
pub async fn cache_control_html(request: Request, next: Next) -> Response {
    // Capture request metadata TRƯỚC khi next.run() consume.
    let method = request.method().clone();
    let is_get = method == axum::http::Method::GET || method == axum::http::Method::HEAD;
    let path = request.uri().path().to_string();
    // v2.5.1 — thêm /manifest: handler set Content-Type
    // application/manifest+json + Cache-Control max-age=86400 riêng —
    // KHÔNG được xử lý như HTML page (bug v2.3.0: /manifest.json bị ép
    // Content-Type text/html + cache 60s).
    let is_api = path.starts_with("/api/")
        || path.starts_with("/chat/")
        || path.starts_with("/ai/")
        || path.starts_with("/opensearch")
        || path.starts_with("/rss")
        || path.starts_with("/sitemap")
        || path.starts_with("/robots")
        || path.starts_with("/health")
        || path.starts_with("/manifest")
        || path.starts_with("/.well-known/");
    let is_static = path.starts_with("/static/") || path.starts_with("/uploads/");
    let is_htmx = request
        .headers()
        .get("hx-request")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("true"));
    let accept_html = request
        .headers()
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|a| a.contains("text/html"));
    let if_none_match = request
        .headers()
        .get(axum::http::header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    // Cookie session — nếu có cookie ls_session → user đã login → dùng
    // Cache-Control private, không cache browser (revalidate mỗi request).
    let has_session_cookie = request
        .headers()
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|cookies| cookies.contains("ls_session="));

    let response = next.run(request).await;

    // Bỏ qua nếu: non-GET, API/static/htmx, không phải HTML, hoặc status
    // không phải 2xx (không cache lỗi).
    if !is_get || is_api || is_static || is_htmx || !accept_html {
        return response;
    }
    if !response.status().is_success() {
        return response;
    }

    // Snapshot headers trước khi consume body — để rebuild response giữ
    // nguyên CSP/HSTS/X-Frame-Options/etc. từ security_headers.
    let original_headers = response.headers().clone();
    let original_status = response.status();

    // Đọc body ra để hash — phải collect hết vì cần hash toàn bộ.
    // Trade-off: tốn memory tạm cho 1 page (~50-200KB), đáng để có ETag.
    // v2.4 — Lower limit từ 16MB xuống 4MB: bài tin dài 50K chars + markdown
    // syntax highlight + 6 post-process pass ~ 1-2MB max. 4MB đủ an toàn
    // cho mọi page, giảm memory pressure khi concurrent (vd 100 users *
    // 16MB = 1.6GB tạm). Vượt 4MB → skip ETag, vẫn response bình thường.
    let body = match axum::body::to_bytes(response.into_body(), 4 * 1024 * 1024).await {
        Ok(bytes) => bytes,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    // Compute ETag (weak) từ body content + length. DefaultHasher đủ cho
    // cache matching (không phải mục đích chống tamper — Content-Length
    // cũng được verify bởi browser).
    let etag = compute_etag(&body);

    // Nếu client gửi If-None-Match khớp ETag → trả 304 (body rỗng)
    if let Some(inm) = if_none_match {
        if inm == etag || inm == format!("\"{etag}\"") || inm == format!("W/\"{etag}\"") {
            let mut not_modified = Response::new(axum::body::Body::empty());
            *not_modified.status_mut() = StatusCode::NOT_MODIFIED;
            let headers = not_modified.headers_mut();
            // Copy toàn bộ headers gốc (CSP/HSTS/etc.) — bảo đảm 304 cũng có
            // security headers đầy đủ.
            for (name, value) in &original_headers {
                headers.insert(name.clone(), value.clone());
            }
            headers.insert(axum::http::header::ETAG, etag_value(&etag));
            set_html_cache_headers(headers, has_session_cookie);
            if let Ok(v) = HeaderValue::from_str("Cookie, Accept, Accept-Encoding") {
                headers.insert(axum::http::header::VARY, v);
            }
            return not_modified;
        }
    }

    // Build response mới với body cũ + headers gốc + ETag + Cache-Control
    let mut final_response = Response::new(axum::body::Body::from(body));
    *final_response.status_mut() = original_status;
    let headers = final_response.headers_mut();
    // Copy headers gốc — ghi đè Content-Type/Content-Length để axum tự set lại
    // khi body mới (đã có đúng length trong Body::from(Bytes)).
    for (name, value) in &original_headers {
        // Skip Content-Length — sẽ được axum tự compute lại cho body mới.
        // Skip Content-Encoding — CompressionLayer (outer) sẽ nén lại nếu cần.
        if name == axum::http::header::CONTENT_LENGTH
            || name == axum::http::header::CONTENT_ENCODING
        {
            continue;
        }
        headers.insert(name.clone(), value.clone());
    }
    // v2.5.1 — FIX: KHÔNG ép Content-Type: text/html nữa. Copy loop phía
    // trên đã khôi phục Content-Type GỐC của response (askama → text/html;
    // manifest/JSON endpoint → đúng MIME riêng). Trước đây insert cứng
    // text/html đè mất MIME gốc (bug từ v2.3.0: /manifest.json nhận về
    // text/html thay vì application/manifest+json).
    headers.insert(axum::http::header::ETAG, etag_value(&etag));
    set_html_cache_headers(headers, has_session_cookie);
    if let Ok(v) = HeaderValue::from_str("Cookie, Accept, Accept-Encoding") {
        headers.insert(axum::http::header::VARY, v);
    }
    // Link header cho HTTP/2 Early Hints — preload critical assets.
    // Browser cache first visit có thể dùng hint này fetch song song CSS/JS
    // trước khi parse HTML đến thẻ <link>/<script> tương ứng.
    if let Ok(link_val) = HeaderValue::from_str(
        "</static/css/style.css?v=2.8.0>; rel=preload; as=style, \
         </static/js/htmx.min.js?v=2.8.0>; rel=preload; as=script, \
         </static/js/app.js?v=2.8.0>; rel=preload; as=script, \
         </static/fonts/inter-var-latin.woff2>; rel=preload; as=font; crossorigin",
    ) {
        headers.insert(axum::http::header::LINK, link_val);
    }
    let _ = HeaderName::from_static; // silence unused import warning
    final_response
}

/// Compute ETag (weak) từ body content. DefaultHasher đủ nhanh và đủ mạnh
/// cho mục đích cache matching — không cần kháng va chạm vì browser verify
/// thêm bằng Content-Length.
fn compute_etag(bytes: &[u8]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;
    let mut hasher = DefaultHasher::new();
    hasher.write(bytes);
    hasher.write_usize(bytes.len());
    format!("{:016x}", hasher.finish())
}

fn etag_value(etag: &str) -> HeaderValue {
    // ETag weak prefix W/ — cho phép browser revalidate semantic equivalent
    // (vd: compression khác → cùng ETag weak, vẫn 304).
    let s = format!("W/\"{etag}\"");
    HeaderValue::from_str(&s).unwrap_or_else(|_| HeaderValue::from_static("W/\"0\""))
}

fn set_html_cache_headers(headers: &mut axum::http::HeaderMap, has_session: bool) {
    if has_session {
        // User đã login — Cache-Control private (không cache proxy),
        // no-cache (revalidate mỗi request qua ETag). Vẫn hưởng lợi ETag
        // (304 Not Modified).
        headers.insert(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("private, no-cache, must-revalidate"),
        );
    } else {
        // Anonymous — cache browser 1 phút + SWR 10 phút. Browser gửi
        // If-None-Match → 304 không body nếu ETag khớp.
        headers.insert(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=60, stale-while-revalidate=600"),
        );
    }
}

// ============================================================
// Error page middleware (v2.1.0)
//
// FIX bug "trang lỗi hiện HTML thuần": AppError::into_response render
// partials/error.html (fragment KHÔNG có <html>/CSS) cho MỌI request.
// Khi browser điều hướng trực tiếp tới URL lỗi (bấm link hỏng, vào trang
// bị cấm, OAuth callback fail...) người dùng thấy chữ trơn không giao diện.
//
// Middleware này đọc marker `ErrorPageInfo` từ response extension:
//   - Request HTMX (header HX-Request: true) → giữ nguyên partial
//     (HTMX swap fragment vào DOM hiện tại — đúng thiết kế).
//   - Request browser (Accept chứa text/html) → render lại body bằng
//     templates/error.html — trang lỗi standalone có đầy đủ stylesheet,
//     sync theme, nút về trang chủ. Giữ nguyên status + headers gốc.
//   - Request khác (curl, API client với Accept: application/json...)
//     → giữ nguyên như cũ (không đoán mò content-type).
// ============================================================
pub async fn error_page_mw(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    // Capture thông tin cần từ request TRƯỚC khi next.run() consume nó.
    let is_htmx = request
        .headers()
        .get("hx-request")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("true"));
    let accepts_html = request
        .headers()
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|a| a.contains("text/html"));
    let jar = CookieJar::from_headers(request.headers());

    let response = next.run(request).await;

    // Chỉ can thiệp khi response là lỗi từ AppError (có marker) VÀ
    // request đến từ browser navigation (không phải HTMX, chấp nhận HTML).
    if is_htmx || !accepts_html {
        return response;
    }
    let Some(info) = response
        .extensions()
        .get::<crate::error::ErrorPageInfo>()
        .cloned()
    else {
        return response;
    };

    // Lấy user hiện hành (best-effort — lỗi DB thì render trang lỗi
    // không có user, vẫn đẹp hơn fragment trơn).
    // v2.6.0 — Wrap với timeout 2s: nếu error gốc là DB pool exhaustion
    // (5xx), current_user_from_jar sẽ lại cố query DB → có thể treo
    // thêm 10s (acquire_timeout) trước khi trả. Timeout 2s cắt ngang
    // → render trang lỗi với current_user=None nhanh chóng thay vì
    // cộng dồn latency vào response lỗi.
    let current_user =
        tokio::time::timeout(Duration::from_secs(2), current_user_from_jar(&state, &jar))
            .await
            .ok()
            .flatten();

    // Render trang lỗi đầy đủ giao diện.
    let full_page = crate::templates::ErrorTemplate {
        status: info.status,
        message: info.message.clone(),
        current_user,
        request_id: info.request_id.clone(),
    }
    .render()
    .unwrap_or_default();

    // Giữ nguyên status + headers của response gốc (security headers,
    // x-request-id, retry-after...), chỉ thay body.
    let (parts, _old_body) = response.into_parts();
    let mut new_resp = axum::response::Html(full_page).into_response();
    // Copy status
    if let Ok(st) = StatusCode::from_u16(info.status) {
        *new_resp.status_mut() = st;
    }
    // Copy headers gốc đè lên headers mới (trừ Content-Length sẽ được
    // axum tính lại — into_parts đã set Content-Length của body cũ).
    let new_headers = new_resp.headers_mut();
    let keys: Vec<axum::http::HeaderName> = parts.headers.keys().cloned().collect();
    for key in keys {
        // FIX v2.4.1 (bug "trang lỗi hiện HTML thuần"): KHÔNG copy
        // Content-Type + Content-Encoding từ response cũ. Response cũ do
        // AppError sinh ra (trước fix) có thể mang Content-Type:
        // text/plain — nếu đè lên `Html(full_page)` mới thì browser nhận
        // text/plain và hiển thị THẺ HTML thô. Html<> đã tự set đúng
        // `text/html; charset=utf-8`. Content-Encoding cũng bỏ: body mới
        // chưa nén (CompressionLayer ở outer sẽ nén lại toàn bộ).
        if key == axum::http::header::CONTENT_LENGTH
            || key == axum::http::header::CONTENT_TYPE
            || key == axum::http::header::CONTENT_ENCODING
        {
            continue;
        }
        let values: Vec<HeaderValue> = parts.headers.get_all(&key).iter().cloned().collect();
        new_headers.remove(&key);
        for v in values {
            new_headers.append(&key, v);
        }
    }
    // Extensions của response cũ KHÔNG copy được type-erased sang response
    // mới — nhưng không sao: các layer bên ngoài (security_headers,
    // PropagateRequestId, Compression) đã/không cần đọc extension này;
    // marker ErrorPageInfo chỉ cần cho middleware này (đã tiêu thụ).
    let _ = parts.extensions;
    new_resp
}

// ============================================================
// Origin check middleware (v2.1.0 — CSRF defense-in-depth)
//
// SameSite=Lax cookie đã chặn phần lớn CSRF, nhưng defense-in-depth
// yêu cầu thêm xác thực Origin cho mọi request đổi-trạng-thái
// (POST/PUT/PATCH/DELETE): cross-site form auto-submit sẽ có Origin
// của site tấn công → mismatch → 403 ngay tại middleware, handler
// không bao giờ được gọi.
//
// Quy tắc (OWASP Origin Verification):
//   - Origin có          → host phải khớp Host header HOẶC base_url host.
//   - Origin vắng, Referer có → Referer host phải khớp (tương tự).
//   - Cả hai vắng        → cho qua (curl, AI agent Bearer token, client
//     không phải browser — không thể là browser hiện đại vì mọi browser
//     đều gửi Origin cho cross-origin POST & form POST).
//   - Origin: null (sandboxed iframe / data: URI) → chặn (vector tấn công).
//
// So sánh với CHÍNH Host header của request (không chỉ base_url) để
// không vỡ khi site phục vụ nhiều domain (apex + www) hoặc BASE_URL
// cấu hình lệch. Host spoof bị reverse proxy (Traefik) chặn sẵn — chỉ
// route request tới host đã đăng ký.
// ============================================================
pub async fn origin_check(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let method = request.method().clone();
    let is_unsafe = matches!(
        method,
        axum::http::Method::POST
            | axum::http::Method::PUT
            | axum::http::Method::PATCH
            | axum::http::Method::DELETE
    );
    if !is_unsafe {
        return Ok(next.run(request).await);
    }

    let host = request
        .headers()
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(|h| h.split(':').next().unwrap_or("").to_lowercase())
        .unwrap_or_default();
    let origin = request
        .headers()
        .get(axum::http::header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string());
    let referer = request
        .headers()
        .get(axum::http::header::REFERER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string());

    if let Some(err) = check_origin_headers(
        origin.as_deref(),
        referer.as_deref(),
        &host,
        &state.config.base_url,
    ) {
        tracing::warn!(
            "CSRF Origin check FAIL: method={} host={host} origin={origin:?} referer={referer:?}",
            method
        );
        return Err(err);
    }

    Ok(next.run(request).await)
}

/// So sánh Origin/Referer với Host + base_url. Trả `Some(AppError)` nếu
/// từ chối, `None` nếu cho qua.
fn check_origin_headers(
    origin: Option<&str>,
    referer: Option<&str>,
    host: &str,
    base_url: &str,
) -> Option<AppError> {
    let base_host = base_url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_lowercase();

    /// Lấy hostname từ Origin/Referer value: bỏ scheme, port, path.
    fn hostname_of(url: &str) -> &str {
        url.trim()
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap_or("")
            .split(':')
            .next()
            .unwrap_or("")
    }

    let host_match = |url: &str| -> bool {
        let h = hostname_of(url).to_lowercase();
        if h.is_empty() {
            return false;
        }
        // Khớp Host của request NÀY (đa domain vẫn ổn) hoặc base_url.
        (!host.is_empty() && h == host) || (!base_host.is_empty() && h == base_host)
    };

    match (
        origin.filter(|o| !o.is_empty()),
        referer.filter(|r| !r.is_empty()),
    ) {
        (Some(o), _) => {
            // Origin: null → luôn chặn (sandboxed iframe / data: URI attack).
            if o.eq_ignore_ascii_case("null") {
                return Some(AppError::Forbidden(
                    "Yêu cầu không hợp lệ (origin null)".into(),
                ));
            }
            if host_match(o) {
                None
            } else {
                Some(AppError::Forbidden(
                    "Yêu cầu không đến từ domain hợp lệ".into(),
                ))
            }
        }
        // Origin vắng (client không-browser hoặc browser rất cũ) → xét Referer.
        (None, Some(r)) => {
            if host_match(r) {
                None
            } else {
                Some(AppError::Forbidden(
                    "Yêu cầu không đến từ domain hợp lệ".into(),
                ))
            }
        }
        // Không có cả hai → client không phải browser (curl, AI agent) →
        // cho qua; rate limit + auth vẫn bảo vệ endpoint.
        (None, None) => None,
    }
}

// ============================================================
// AI Agent auth middleware: chỉ cho phép AI Agent (role=ai_agent)
// qua. Dùng cho các endpoint /ai/* (report progress, v.v.).
//
// Ưu tiên kiểm tra Authorization: Bearer <token> trước (lấy từ header).
// Nếu không có, fallback sang session cookie (AI đã đăng nhập qua web).
// ============================================================
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
        .map(std::string::ToString::to_string);

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
            .map(|u| Self(u.clone()))
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}

#[cfg(test)]
mod origin_check_tests {
    use super::check_origin_headers;

    const HOST: &str = "louis.vangioitutien.com";
    const BASE: &str = "https://louis.vangioitutien.com";

    #[test]
    fn same_origin_passes() {
        // Form POST từ chính site: Origin khớp Host → cho qua.
        assert!(
            check_origin_headers(Some("https://louis.vangioitutien.com"), None, HOST, BASE)
                .is_none()
        );
    }

    #[test]
    fn cross_site_origin_blocked() {
        // Cross-site form auto-submit: Origin của attacker → 403.
        assert!(check_origin_headers(Some("https://evil.example"), None, HOST, BASE).is_some());
    }

    #[test]
    fn origin_null_blocked() {
        // Sandbox iframe / data: URI → Origin: null → chặn.
        assert!(check_origin_headers(Some("null"), None, HOST, BASE).is_some());
    }

    #[test]
    fn referer_fallback_passes() {
        // Browser cũ không gửi Origin nhưng gửi Referer khớp host → qua.
        assert!(check_origin_headers(
            None,
            Some("https://louis.vangioitutien.com/games/abc"),
            HOST,
            BASE
        )
        .is_none());
    }

    #[test]
    fn referer_fallback_blocked() {
        assert!(
            check_origin_headers(None, Some("https://evil.example/phish"), HOST, BASE).is_some()
        );
    }

    #[test]
    fn no_headers_passes_for_non_browser() {
        // curl / AI agent không gửi Origin lẫn Referer → cho qua
        // (rate limit + auth vẫn bảo vệ).
        assert!(check_origin_headers(None, None, HOST, BASE).is_none());
    }

    #[test]
    fn port_suffix_matched() {
        // Dev localhost:3000 — Origin có port, Host có port → strip port
        // rồi so hostname (cả 2 đều "localhost").
        assert!(check_origin_headers(
            Some("http://localhost:3000"),
            None,
            "localhost:3000",
            "http://localhost:3000"
        )
        .is_none());
    }

    #[test]
    fn base_url_match_passes_when_host_differs() {
        // Host header thiếu/lệch nhưng Origin khớp BASE_URL → vẫn qua
        // (đề phòng proxy không forward Host).
        assert!(
            check_origin_headers(Some("https://louis.vangioitutien.com"), None, "", BASE).is_none()
        );
    }

    #[test]
    fn empty_origin_ignored_falls_to_referer() {
        // Origin rỗng ("" — header lỗi) → coi như vắng, xét Referer.
        assert!(check_origin_headers(None, Some("https://evil.example"), HOST, BASE).is_some());
    }

    #[test]
    fn subdomain_not_accepted() {
        // Subdomain lạ KHÔNG được tự động chấp nhận (evil.louis... khác host).
        assert!(check_origin_headers(
            Some("https://evil-louis.vangioitutien.com"),
            None,
            HOST,
            BASE
        )
        .is_some());
    }
}

#[cfg(test)]
mod verify_origin_tests {
    use super::verify_origin;
    use axum::http::HeaderMap;

    fn hm_origin(o: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        if !o.is_empty() {
            h.insert(axum::http::header::ORIGIN, o.parse().unwrap());
        }
        h
    }

    fn hm_origin_referer(o: &str, r: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        if !o.is_empty() {
            h.insert(axum::http::header::ORIGIN, o.parse().unwrap());
        }
        if !r.is_empty() {
            h.insert(axum::http::header::REFERER, r.parse().unwrap());
        }
        h
    }

    #[test]
    fn test_origin_match_https() {
        let h = hm_origin("https://louis.vangioitutien.com");
        assert!(verify_origin(&h, "https://louis.vangioitutien.com").is_ok());
    }

    #[test]
    fn test_origin_match_localhost_with_port() {
        let h = hm_origin("http://localhost:3000");
        assert!(verify_origin(&h, "http://localhost:3000").is_ok());
    }

    #[test]
    fn test_origin_mismatch_rejected() {
        let h = hm_origin("https://evil.com");
        assert!(verify_origin(&h, "https://louis.vangioitutien.com").is_err());
    }

    #[test]
    fn test_origin_subdomain_rejected() {
        // subdomain khác phải bị từ chối — không phải subdomain hợp lệ.
        let h = hm_origin("https://evil.louis.vangioitutien.com");
        assert!(verify_origin(&h, "https://louis.vangioitutien.com").is_err());
    }

    #[test]
    fn test_no_origin_no_referer_allowed_for_curl() {
        // curl/legacy client không có header → cho phép qua (không fail-closed
        // để không phá tương thích dev/test với curl).
        let h = HeaderMap::new();
        assert!(verify_origin(&h, "https://louis.vangioitutien.com").is_ok());
    }

    #[test]
    fn test_referer_fallback_when_origin_empty() {
        let h = hm_origin_referer("", "https://louis.vangioitutien.com/auth/ai/login");
        assert!(verify_origin(&h, "https://louis.vangioitutien.com").is_ok());
    }

    #[test]
    fn test_referer_mismatch_rejected_when_origin_empty() {
        let h = hm_origin_referer("", "https://evil.com/path");
        assert!(verify_origin(&h, "https://louis.vangioitutien.com").is_err());
    }

    #[test]
    fn test_origin_match_referer_mismatch_uses_origin() {
        // Origin đúng → OK dù Referer sai (Origin là chuẩn RFC 6454)
        let h = hm_origin_referer("https://louis.vangioitutien.com", "https://evil.com/path");
        assert!(verify_origin(&h, "https://louis.vangioitutien.com").is_ok());
    }
}

#[cfg(test)]
mod client_ip_tests {
    use super::{client_ip_from_parts, is_private_ip};
    use axum::http::HeaderMap;
    use std::net::SocketAddr;

    fn hm_xff(v: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", v.parse().unwrap());
        h
    }

    fn addr(ip: &str) -> SocketAddr {
        format!("{ip}:12345").parse().unwrap()
    }

    #[test]
    fn single_proxy_takes_rightmost() {
        // 1 hop: Traefik set XFF = IP client
        let h = hm_xff("203.0.113.10");
        assert_eq!(
            client_ip_from_parts(&h, Some(&addr("10.0.0.5")), true, 1),
            "203.0.113.10"
        );
    }

    #[test]
    fn single_proxy_client_spoofed_prefix_still_safe() {
        // Client tự gắn XFF giả → trusted proxy append IP thật vào cuối →
        // lấy cuối (hops=1) là IP thật, phần tử giả bị bỏ qua.
        let h = hm_xff("1.2.3.4, 203.0.113.10");
        assert_eq!(
            client_ip_from_parts(&h, Some(&addr("10.0.0.5")), true, 1),
            "203.0.113.10"
        );
    }

    #[test]
    fn two_hops_takes_second_from_right() {
        // CDN → Traefik → app: XFF = "client, cdn_edge". Phần tử cuối là
        // IP edge của CDN (ai cũng giống nhau) — hops=2 lấy client.
        let h = hm_xff("203.0.113.10, 104.16.1.1");
        assert_eq!(
            client_ip_from_parts(&h, Some(&addr("10.0.0.5")), true, 2),
            "203.0.113.10"
        );
    }

    #[test]
    fn two_hops_with_spoofed_prefix() {
        // Attacker gắn "1.2.3.4" → CDN append client → Traefik append CDN:
        // "1.2.3.4, client, cdn" — hops=2 vẫn lấy đúng client.
        let h = hm_xff("1.2.3.4, 203.0.113.10, 104.16.1.1");
        assert_eq!(
            client_ip_from_parts(&h, Some(&addr("10.0.0.5")), true, 2),
            "203.0.113.10"
        );
    }

    #[test]
    fn xff_shorter_than_hops_falls_back_to_leftmost() {
        // hops=2 nhưng XFF chỉ có 1 phần tử → lấy phần tử đầu (do proxy
        // ngoài cùng ghi) thay vì "unknown".
        let h = hm_xff("203.0.113.10");
        assert_eq!(
            client_ip_from_parts(&h, Some(&addr("10.0.0.5")), true, 2),
            "203.0.113.10"
        );
    }

    #[test]
    fn hops_1_trusts_x_real_ip() {
        let mut h = HeaderMap::new();
        h.insert("x-real-ip", "203.0.113.77".parse().unwrap());
        assert_eq!(
            client_ip_from_parts(&h, Some(&addr("10.0.0.5")), true, 1),
            "203.0.113.77"
        );
    }

    #[test]
    fn hops_2_ignores_x_real_ip() {
        // ≥2 hop: X-Real-IP do proxy gần app ghi = IP proxy trước nó,
        // không phải client → bỏ qua, parse XFF theo hop.
        let mut h = HeaderMap::new();
        h.insert("x-real-ip", "104.16.1.1".parse().unwrap());
        h.insert(
            "x-forwarded-for",
            "203.0.113.10, 104.16.1.1".parse().unwrap(),
        );
        assert_eq!(
            client_ip_from_parts(&h, Some(&addr("10.0.0.5")), true, 2),
            "203.0.113.10"
        );
    }

    #[test]
    fn no_trust_falls_back_to_connect_info() {
        let h = hm_xff("203.0.113.10");
        assert_eq!(
            client_ip_from_parts(&h, Some(&addr("198.51.100.9")), false, 1),
            "198.51.100.9"
        );
    }

    #[test]
    fn invalid_xff_entries_skipped() {
        let h = hm_xff("garbage, 203.0.113.10, not-an-ip");
        assert_eq!(
            client_ip_from_parts(&h, Some(&addr("10.0.0.5")), true, 1),
            "203.0.113.10"
        );
    }

    #[test]
    fn private_ip_detection() {
        // IP private/loopback = dấu hiệu proxy giấu IP thật
        assert!(is_private_ip("10.187.247.1"));
        assert!(is_private_ip("172.17.0.1"));
        assert!(is_private_ip("192.168.1.1"));
        assert!(is_private_ip("127.0.0.1"));
        assert!(is_private_ip("169.254.1.1"));
        assert!(is_private_ip("::1"));
        assert!(is_private_ip("unknown"));
        // IP public thật → KHÔNG private
        assert!(!is_private_ip("203.0.113.10"));
        assert!(!is_private_ip("163.44.96.79"));
        assert!(!is_private_ip("2402:800:61f7::1"));
    }
}

#[cfg(test)]
mod cookie_value_tests {
    use super::{anon_cookie_value, cookie_value, session_cookie_value, ANON_COOKIE};
    use axum::http::HeaderMap;

    fn hm_cookie(v: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(axum::http::header::COOKIE, v.parse().unwrap());
        h
    }

    #[test]
    fn reads_session_cookie() {
        let h = hm_cookie("theme=dark; kg_session=abc123; other=x");
        assert_eq!(session_cookie_value(&h).as_deref(), Some("abc123"));
    }

    #[test]
    fn reads_anon_cookie() {
        let h = hm_cookie("ls_anon=uuid-xyz; theme=dark");
        assert_eq!(anon_cookie_value(&h).as_deref(), Some("uuid-xyz"));
    }

    #[test]
    fn no_cookie_returns_none() {
        let h = hm_cookie("theme=dark");
        assert!(session_cookie_value(&h).is_none());
        assert!(anon_cookie_value(&h).is_none());
    }

    #[test]
    fn prefix_collision_rejected() {
        // Cookie "kg_session_old" không bị nhầm là "kg_session"
        let h = hm_cookie("kg_session_old=abc");
        assert!(session_cookie_value(&h).is_none());
        assert_eq!(cookie_value(&h, "kg_session"), None);
        let _ = ANON_COOKIE; // silence unused warning nếu cần
    }
}

// ============================================================
// v2.4.0 — REQUEST TIMEOUT MIDDLEWARE
// ------------------------------------------------------------
// Chống "hang forever": nếu 1 request exceeds timeout (mặc định 30s),
// middleware ngắt → trả 504 Gateway Timeout cho client. Tránh tình
// trạng thái 1 query DB chậm / loop vô hạn / pool exhausted giữ
// connection treo mãi — operator không cần kill process thủ công.
//
// Layer ordering: đặt OUTERMOST (sau CompressionLayer) để cover
// mọi handler. Skip upgrade WebSocket (có heartbeat riêng 30s).
//
// Timeout đọc từ REQUEST_TIMEOUT_SECS env (default 30s). 0 = tắt.
// ============================================================
pub async fn request_timeout(request: Request, next: Next) -> Response {
    // WebSocket upgrade — skip timeout (WS có heartbeat 30s riêng,
    // không thể ngắt qua HTTP timeout vì upgrade đã chuyển sang WS).
    if request.headers().get(axum::http::header::UPGRADE).is_some() {
        return next.run(request).await;
    }
    let secs: u64 = std::env::var("REQUEST_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .filter(|v: &u64| *v > 0 && *v <= 600)
        .unwrap_or(30);
    let timeout = Duration::from_secs(secs);
    match tokio::time::timeout(timeout, next.run(request)).await {
        Ok(resp) => resp,
        Err(_) => {
            tracing::error!(
                "Request timeout sau {secs}s — client sẽ nhận 504. \
                 Có thể do DB query chậm, markdown render nặng, hoặc \
                 pool exhausted. Tăng REQUEST_TIMEOUT_SECS nếu cần."
            );
            let mut resp = (
                StatusCode::GATEWAY_TIMEOUT,
                "Yêu cầu xử lý quá thời gian — vui lòng thử lại sau.",
            )
                .into_response();
            resp.headers_mut().insert(
                axum::http::header::CACHE_CONTROL,
                HeaderValue::from_static("no-store"),
            );
            resp.headers_mut().insert(
                axum::http::header::RETRY_AFTER,
                HeaderValue::from_static("5"),
            );
            resp
        }
    }
}
