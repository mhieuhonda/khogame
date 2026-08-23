use crate::auth::{hash_token, SESSION_COOKIE};
use crate::error::AppError;
use crate::models::user::User;
use crate::repositories::{SessionRepo, UserRepo};
use crate::state::AppState;
use axum::{
    extract::{ConnectInfo, FromRef, FromRequestParts, Request, State},
    http::{request::Parts, StatusCode},
    middleware::Next,
    response::Response,
};
use axum_extra::extract::CookieJar;
use std::net::SocketAddr;
use std::sync::Arc;

impl FromRef<Arc<AppState>> for AppState {
    fn from_ref(state: &Arc<AppState>) -> Self {
        (**state).clone()
    }
}

/// Extracts the current user from the request, if any.
pub async fn current_user_from_jar(
    state: &AppState,
    jar: &CookieJar,
) -> Option<User> {
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
    let jar = CookieJar::from_headers(&request.headers());
    let user = current_user_from_jar(&state, &jar)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if !user.role.is_staff() {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(next.run(request).await)
}

pub async fn get_client_ip(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> String {
    addr.ip().to_string()
}
