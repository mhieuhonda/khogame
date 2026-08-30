use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SESSION_COOKIE: &str = "kg_session";
pub const SESSION_TTL_DAYS: i64 = 30;

/// Tên cookie lưu OAuth `state` (CSRF) - sống ngắn, dùng 1 lần.
pub const OAUTH_STATE_COOKIE: &str = "kg_oauth_state";
/// Tên cookie lưu `next` path sau login.
pub const OAUTH_NEXT_COOKIE: &str = "kg_oauth_next";
/// TTL cho OAuth state/next cookies.
const OAUTH_TEMP_TTL_SECS: i64 = 600; // 10 phút

#[derive(Debug, Clone, Deserialize)]
pub struct GoogleUserInfo {
    pub sub: String,
    pub email: String,
    pub email_verified: Option<bool>,
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub picture: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Serialize)]
struct GoogleTokenRequest<'a> {
    code: &'a str,
    client_id: &'a str,
    client_secret: &'a str,
    redirect_uri: &'a str,
    grant_type: &'a str,
}

#[derive(Debug, Deserialize)]
pub struct GoogleTokenResponse {
    pub access_token: String,
    #[allow(dead_code)]
    pub id_token: Option<String>,
    #[allow(dead_code)]
    pub expires_in: Option<i64>,
    #[allow(dead_code)]
    pub token_type: Option<String>,
    #[allow(dead_code)]
    pub refresh_token: Option<String>,
}

#[must_use]
pub fn build_auth_url(state: &AppState, csrf_token: &str) -> String {
    // v2.1.0 FIX "Google hỏi lại consent mỗi lần đăng nhập":
    // TRƯỚC ĐÂY gửi `prompt=consent` + `access_type=offline` → Google buộc
    // hiển thị màn hình đồng ý (bấm "Tiếp tục") MỖI LẦN đăng nhập, kể cả
    // khi user đã đồng ý trước đó — ngược với hành vi chuẩn của các website
    // khác. refresh_token trả về cũng KHÔNG bao giờ được dùng (dead code).
    //
    // SAU FIX: bỏ cả 2 param. Google giờ nhớ lần đồng ý đầu tiên:
    //   - User đang đăng nhập Google (1 tài khoản) + đã consent →
    //     redirect thẳng về callback KHÔNG hỏi gì thêm (1 cú click).
    //   - User có nhiều tài khoản Google → Google hiện bảng chọn tài khoản
    //     (chuẩn mọi website), vẫn không hỏi lại consent.
    let params = [
        ("client_id", state.config.google_client_id.as_str()),
        ("redirect_uri", state.config.google_redirect_uri.as_str()),
        ("response_type", "code"),
        ("scope", "openid email profile"),
        ("state", csrf_token),
    ];
    let query = serde_urlencoded::to_string(params).unwrap_or_default();
    format!("https://accounts.google.com/o/oauth2/v2/auth?{query}")
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn exchange_code(state: &AppState, code: &str) -> AppResult<GoogleTokenResponse> {
    let body = GoogleTokenRequest {
        code,
        client_id: &state.config.google_client_id,
        client_secret: &state.config.google_client_secret,
        redirect_uri: &state.config.google_redirect_uri,
        grant_type: "authorization_code",
    };
    let resp = state
        .http_client
        .post("https://oauth2.googleapis.com/token")
        .form(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        // Tránh log/echo raw response body — có thể chứa token tạm
        // hoặc thông tin nhạy cảm Google trả kèm. Chỉ giữ status code.
        let status = resp.status();
        tracing::warn!("OAuth token exchange failed: status={status}");
        return Err(AppError::OAuth(format!(
            "Token exchange failed (HTTP {status})"
        )));
    }
    let token: GoogleTokenResponse = resp.json().await?;
    Ok(token)
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn fetch_userinfo(state: &AppState, access_token: &str) -> AppResult<GoogleUserInfo> {
    let resp = state
        .http_client
        .get("https://www.googleapis.com/oauth2/v3/userinfo")
        .bearer_auth(access_token)
        .send()
        .await?;
    if !resp.status().is_success() {
        // Tránh lộ raw body — Google có thể trả kèm token tạm hoặc
        // thông tin session nhạy cảm khi có lỗi. Chỉ giữ status code
        // trong message trả về cho user.
        let status = resp.status();
        tracing::warn!("OAuth userinfo fetch failed: status={status}");
        return Err(AppError::OAuth(format!(
            "Userinfo fetch failed (HTTP {status})"
        )));
    }
    let info: GoogleUserInfo = resp.json().await?;
    Ok(info)
}

#[must_use]
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

#[must_use]
pub fn gen_session_token() -> String {
    use rand::RngExt;
    let mut rng = rand::rng();
    let bytes: [u8; 32] = rng.random();
    hex::encode(bytes)
}

/// Sinh API token dài hạn cho AI Agent.
/// Dài hơn session token (48 bytes = 96 hex chars) để tăng độ khó brute-force.
/// Prefix "kgai_" để phân biệt với session token thường khi debug log.
#[must_use]
pub fn gen_ai_agent_token() -> String {
    use rand::RngExt;
    let mut rng = rand::rng();
    // fill() với mảng mut — 48 bytes ngẫu nhiên (96 hex chars)
    let mut bytes = [0u8; 48];
    rng.fill(&mut bytes[..]);
    format!("kgai_{}", hex::encode(bytes))
}

/// Hash một AI Agent API token. Dùng SHA-256 giống session token.
/// Trả về hex 64 chars.
#[must_use]
pub fn hash_ai_agent_token(token: &str) -> String {
    hash_token(token)
}

/// Lấy cookie `Secure` flag — dựa vào BASE_URL (https://→ true) nhưng
/// cho phép override qua env `COOKIE_SECURE=1` cho prod chạy sau TLS
/// terminating proxy với BASE_URL=http://localhost (mặc định config).
fn should_secure_cookie(base_url: &str) -> bool {
    if std::env::var("COOKIE_SECURE")
        .ok()
        .is_some_and(|v| matches!(v.trim(), "1" | "true" | "yes" | "on"))
    {
        return true;
    }
    base_url.starts_with("https://")
}

pub fn set_session_cookie(jar: &mut CookieJar, token: &str, base_url: &str) {
    use axum_extra::extract::cookie::{Cookie, SameSite};
    let cookie = Cookie::build((SESSION_COOKIE, token.to_string()))
        .path("/")
        .http_only(true)
        .secure(should_secure_cookie(base_url))
        .same_site(SameSite::Lax)
        .max_age(time::Duration::days(SESSION_TTL_DAYS))
        .build();
    *jar = std::mem::take(jar).add(cookie);
}

pub fn clear_session_cookie(jar: &mut CookieJar, base_url: &str) {
    use axum_extra::extract::cookie::{Cookie, SameSite};
    let cookie = Cookie::build((SESSION_COOKIE, ""))
        .path("/")
        .http_only(true)
        .secure(should_secure_cookie(base_url))
        .same_site(SameSite::Lax)
        .max_age(time::Duration::seconds(0))
        .build();
    *jar = std::mem::take(jar).add(cookie);
}

// ============================================================
// v3.3.0 — IMPERSONATION (admin/điều hành đăng nhập với tư cách AI Agent)
// ============================================================

/// Cookie lưu phiên GỐC của admin khi đang impersonate một AI Agent.
/// Chỉ HttpOnly + SameSite=Lax + Secure như session thường — trình duyệt
/// KHÔNG đọc được, JS không leak. Sống 2 giờ: quá 2h phải impersonate lại.
pub const IMPERSONATOR_COOKIE: &str = "kg_impersonator";
/// TTL cookie impersonator.
pub const IMPERSONATION_TTL_HOURS: i64 = 2;

/// Đặt cookie lưu phiên gốc của admin (gọi TRƯỚC khi ghi đè kg_session).
pub fn set_impersonator_cookie(jar: &mut CookieJar, admin_token: &str, base_url: &str) {
    use axum_extra::extract::cookie::{Cookie, SameSite};
    let cookie = Cookie::build((IMPERSONATOR_COOKIE, admin_token.to_string()))
        .path("/")
        .http_only(true)
        .secure(should_secure_cookie(base_url))
        .same_site(SameSite::Lax)
        .max_age(time::Duration::hours(IMPERSONATION_TTL_HOURS))
        .build();
    *jar = std::mem::take(jar).add(cookie);
}

/// Xoá cookie impersonator (khi đã khôi phục phiên / đăng xuất hẳn).
pub fn clear_impersonator_cookie(jar: &mut CookieJar, base_url: &str) {
    use axum_extra::extract::cookie::{Cookie, SameSite};
    let cookie = Cookie::build((IMPERSONATOR_COOKIE, ""))
        .path("/")
        .http_only(true)
        .secure(should_secure_cookie(base_url))
        .same_site(SameSite::Lax)
        .max_age(time::Duration::seconds(0))
        .build();
    *jar = std::mem::take(jar).add(cookie);
}

/// Đặt cookie OAuth state (CSRF token) - sống 10 phút, `HttpOnly` + SameSite=Lax.
pub fn set_oauth_state_cookie(jar: &mut CookieJar, state: &str, base_url: &str) {
    use axum_extra::extract::cookie::{Cookie, SameSite};
    let cookie = Cookie::build((OAUTH_STATE_COOKIE, state.to_string()))
        .path("/")
        .http_only(true)
        .secure(should_secure_cookie(base_url))
        .same_site(SameSite::Lax)
        .max_age(time::Duration::seconds(OAUTH_TEMP_TTL_SECS))
        .build();
    *jar = std::mem::take(jar).add(cookie);
}

/// Xoá cookie OAuth state.
pub fn clear_oauth_state_cookie(jar: &mut CookieJar, base_url: &str) {
    use axum_extra::extract::cookie::{Cookie, SameSite};
    let cookie = Cookie::build((OAUTH_STATE_COOKIE, ""))
        .path("/")
        .http_only(true)
        .secure(should_secure_cookie(base_url))
        .same_site(SameSite::Lax)
        .max_age(time::Duration::seconds(0))
        .build();
    *jar = std::mem::take(jar).add(cookie);
}

/// Đặt cookie OAuth `next` path - sống 10 phút.
pub fn set_oauth_next_cookie(jar: &mut CookieJar, next: &str, base_url: &str) {
    use axum_extra::extract::cookie::{Cookie, SameSite};
    let cookie = Cookie::build((OAUTH_NEXT_COOKIE, next.to_string()))
        .path("/")
        .http_only(true)
        .secure(should_secure_cookie(base_url))
        .same_site(SameSite::Lax)
        .max_age(time::Duration::seconds(OAUTH_TEMP_TTL_SECS))
        .build();
    *jar = std::mem::take(jar).add(cookie);
}

/// Xoá cookie OAuth `next`.
pub fn clear_oauth_next_cookie(jar: &mut CookieJar, base_url: &str) {
    use axum_extra::extract::cookie::{Cookie, SameSite};
    let cookie = Cookie::build((OAUTH_NEXT_COOKIE, ""))
        .path("/")
        .http_only(true)
        .secure(should_secure_cookie(base_url))
        .same_site(SameSite::Lax)
        .max_age(time::Duration::seconds(0))
        .build();
    *jar = std::mem::take(jar).add(cookie);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_token_deterministic_and_hex() {
        let h1 = hash_token("my-secret-token");
        let h2 = hash_token("my-secret-token");
        assert_eq!(h1, h2, "cùng input phải ra cùng hash");
        assert_eq!(h1.len(), 64, "SHA-256 hex = 64 ký tự");
        assert!(h1.chars().all(|c| c.is_ascii_hexdigit()));
        // Input khác → hash khác
        assert_ne!(h1, hash_token("other-token"));
    }

    #[test]
    fn test_gen_session_token_random_and_format() {
        let t1 = gen_session_token();
        let t2 = gen_session_token();
        assert_ne!(t1, t2, "token phải ngẫu nhiên mỗi lần sinh");
        assert_eq!(t1.len(), 64, "32 bytes hex = 64 ký tự");
        assert!(t1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_gen_ai_agent_token_prefix_and_length() {
        let t = gen_ai_agent_token();
        assert!(t.starts_with("kgai_"), "phải có prefix kgai_");
        // 48 bytes = 96 hex chars + prefix 5
        assert_eq!(t.len(), 5 + 96);
        let hex_part = &t[5..];
        assert!(hex_part.chars().all(|c| c.is_ascii_hexdigit()));
        // Ngẫu nhiên
        assert_ne!(t, gen_ai_agent_token());
    }

    #[test]
    fn test_hash_ai_agent_token_delegates() {
        assert_eq!(hash_ai_agent_token("kgai_abc"), hash_token("kgai_abc"));
    }
}
