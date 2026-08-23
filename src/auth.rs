use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SESSION_COOKIE: &str = "kg_session";
pub const SESSION_TTL_DAYS: i64 = 30;

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

pub fn build_auth_url(state: &AppState, csrf_token: &str) -> String {
    let params = [
        ("client_id", state.config.google_client_id.as_str()),
        ("redirect_uri", state.config.google_redirect_uri.as_str()),
        ("response_type", "code"),
        ("scope", "openid email profile"),
        ("state", csrf_token),
        ("access_type", "offline"),
        ("prompt", "consent"),
    ];
    let query = serde_urlencoded::to_string(params).unwrap_or_default();
    format!("https://accounts.google.com/o/oauth2/v2/auth?{}", query)
}

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
        let txt = resp.text().await.unwrap_or_default();
        return Err(AppError::OAuth(format!("Token exchange failed: {}", txt)));
    }
    let token: GoogleTokenResponse = resp.json().await?;
    Ok(token)
}

pub async fn fetch_userinfo(state: &AppState, access_token: &str) -> AppResult<GoogleUserInfo> {
    let resp = state
        .http_client
        .get("https://www.googleapis.com/oauth2/v3/userinfo")
        .bearer_auth(access_token)
        .send()
        .await?;
    if !resp.status().is_success() {
        let txt = resp.text().await.unwrap_or_default();
        return Err(AppError::OAuth(format!("Userinfo fetch failed: {}", txt)));
    }
    let info: GoogleUserInfo = resp.json().await?;
    Ok(info)
}

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn gen_session_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    hex::encode(bytes)
}

pub fn set_session_cookie(jar: &mut CookieJar, token: &str, base_url: &str) {
    use axum_extra::extract::cookie::{Cookie, SameSite};
    let cookie = Cookie::build((SESSION_COOKIE, token.to_string()))
        .path("/")
        .http_only(true)
        .secure(base_url.starts_with("https://"))
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
        .secure(base_url.starts_with("https://"))
        .same_site(SameSite::Lax)
        .max_age(time::Duration::seconds(0))
        .build();
    *jar = std::mem::take(jar).add(cookie);
}
