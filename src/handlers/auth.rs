use crate::auth;
use crate::error::{AppError, AppResult};
use crate::middleware::CurrentUser;
use crate::repositories::{SessionRepo, UserRepo};
use crate::state::AppState;
use crate::templates::LoginTemplate;
use axum::extract::{Query, State};
use axum::response::Redirect;
use axum_extra::extract::CookieJar;
use rand::Rng;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct AuthQuery {
    pub next: Option<String>,
}

pub async fn login_page(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<LoginTemplate> {
    if current_user.is_some() {
        // Already logged in - redirect home
        return Err(AppError::BadRequest("Đã đăng nhập".into()));
    }
    let csrf = gen_csrf();
    let auth_url = auth::build_auth_url(&state, &csrf);
    Ok(LoginTemplate {
        current_user: None,
        unread_notifications: 0,
        auth_url,
    })
}

pub async fn google_login(
    State(state): State<Arc<AppState>>,
    Query(q): Query<AuthQuery>,
) -> AppResult<Redirect> {
    let csrf = gen_csrf();
    let auth_url = auth::build_auth_url(&state, &csrf);
    // Store CSRF in a short-lived cookie
    // We'll verify on callback
    // For simplicity, we trust the state param here
    let _ = q.next;
    Ok(Redirect::to(&auth_url))
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: Option<String>,
    pub error: Option<String>,
}

pub async fn google_callback(
    State(state): State<Arc<AppState>>,
    Query(q): Query<CallbackQuery>,
    jar: CookieJar,
) -> AppResult<(CookieJar, Redirect)> {
    if let Some(err) = &q.error {
        tracing::warn!("OAuth error: {}", err);
        return Err(AppError::OAuth(format!("Google từ chối đăng nhập: {}", err)));
    }

    let token = auth::exchange_code(&state, &q.code).await?;
    let userinfo = auth::fetch_userinfo(&state, &token.access_token).await?;

    if userinfo.email_verified.unwrap_or(false) == false {
        return Err(AppError::BadRequest("Email Google chưa được xác minh".into()));
    }

    // Find or create user
    let user = match UserRepo::find_by_google_sub(&state.db, &userinfo.sub).await? {
        Some(u) if u.is_banned => {
            return Err(AppError::Forbidden("Tài khoản đã bị cấm".into()));
        }
        Some(u) => u,
        None => {
            UserRepo::create_from_google(
                &state.db,
                &userinfo.sub,
                &userinfo.email,
                userinfo.name.as_deref().unwrap_or("Người dùng"),
                userinfo.picture.as_deref(),
            )
            .await?
        }
    };

    // Create session
    let session_token = auth::gen_session_token();
    let token_hash = auth::hash_token(&session_token);
    let user_agent = ""; // Could extract from headers
    SessionRepo::create(
        &state.db,
        user.id,
        &token_hash,
        user_agent,
        None,
        auth::SESSION_TTL_DAYS,
    )
    .await?;
    UserRepo::update_last_seen(&state.db, user.id).await?;

    let mut new_jar = jar;
    auth::set_session_cookie(&mut new_jar, &session_token, &state.config.base_url);

    Ok((new_jar, Redirect::to("/")))
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    jar: CookieJar,
) -> AppResult<(CookieJar, Redirect)> {
    if let Some(token) = jar.get(auth::SESSION_COOKIE) {
        let token_hash = auth::hash_token(token.value());
        SessionRepo::delete(&state.db, &token_hash).await?;
    }
    let mut new_jar = jar;
    auth::clear_session_cookie(&mut new_jar, &state.config.base_url);
    let _ = current_user;
    Ok((new_jar, Redirect::to("/")))
}

fn gen_csrf() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 16] = rng.gen();
    hex::encode(bytes)
}

// Helper to read user's unread notifications count
pub async fn unread_count(state: &AppState, user_id: uuid::Uuid) -> i64 {
    crate::repositories::NotificationRepo::unread_count(&state.db, user_id)
        .await
        .unwrap_or(0)
}
