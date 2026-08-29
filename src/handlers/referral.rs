//! v3.0.0 — Handlers chương trình giới thiệu (/referral) + link ngắn
//! /r/{code} (set cookie + redirect về trang chủ).

use crate::error::AppResult;
use crate::middleware::AuthUser;
use crate::repositories::referral::REFERRAL_XP;
use crate::repositories::{GamificationRepo, ReferralRepo};
use crate::state::AppState;
use crate::templates::ReferralTemplate;
use axum::extract::State;
use axum::response::{IntoResponse, Redirect};
use std::sync::Arc;

/// Cookie chứa mã người giới thiệu — 30 ngày.
pub const REFERRAL_COOKIE: &str = "ls_ref";
/// TTL cookie referral (30 ngày, theo giây).
pub const REFERRAL_COOKIE_MAX_AGE: i64 = 30 * 24 * 3600;

/// GET /referral — trang giới thiệu (yêu cầu đăng nhập).
/// # Errors
/// Trả lỗi khi DB fail.
pub async fn referral_page(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<ReferralTemplate> {
    let info = ReferralRepo::stats(&state.db, user.id).await?;
    let unread = crate::handlers::auth::unread_count(&state, user.id).await;
    // Host công khai từ config BASE_URL (bỏ scheme) — cho link copy đẹp
    let base_url_host = state
        .config
        .base_url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string();
    Ok(ReferralTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        info,
        base_url_host,
    })
}

/// GET /r/{code} — link ngắn: set cookie referral + về trang chủ.
/// KHÔNG đăng nhập, KHÔNG ghi DB ở đây (ghi khi người mới đăng nhập lần
/// đầu — an toàn tuyệt đối với bot lướt link).
pub async fn short_link(
    axum::extract::Path(code): axum::extract::Path<String>,
) -> impl IntoResponse {
    // Chuẩn hoá: chỉ nhận [A-Z0-9] độ dài hợp lý — chặn header injection
    let safe_code: String = code
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(20)
        .collect();
    let mut resp = Redirect::to("/").into_response();
    if safe_code.len() >= 4 {
        let cookie = format!(
            "{REFERRAL_COOKIE}={safe_code}; Path=/; Max-Age={REFERRAL_COOKIE_MAX_AGE}; SameSite=Lax"
        );
        if let Ok(v) = axum::http::HeaderValue::from_str(&cookie) {
            resp.headers_mut().append(axum::http::header::SET_COOKIE, v);
        }
    }
    resp
}

/// Thưởng referral cho cả 2 phía sau khi ghi nhận thành công (gọi từ
/// auth callback). Best-effort — lỗi chỉ log warn, không fail luồng login.
pub async fn reward_both(state: &AppState, referrer_id: uuid::Uuid, referred_id: uuid::Uuid) {
    // Người giới thiệu
    let _ = GamificationRepo::award_xp(&state.db, referrer_id, "referral", REFERRAL_XP).await;
    let _ = crate::repositories::NotificationRepo::create_system(
        &state.db,
        referrer_id,
        "🎁 Bạn có người mới qua link giới thiệu!",
        &format!(
            "Một thành viên mới vừa tham gia qua mã của bạn — nhận {} XP thưởng!",
            REFERRAL_XP
        ),
        "/referral",
    )
    .await;
    // Người mới được mời
    let _ = GamificationRepo::award_xp(&state.db, referred_id, "referral", REFERRAL_XP).await;
    let _ = crate::repositories::NotificationRepo::create_system(
        &state.db,
        referred_id,
        "🎁 Chào mừng từ bạn bè!",
        &format!(
            "Bạn được tặng {} XP quà khởi đầu từ chương trình giới thiệu. Chơi vui!",
            REFERRAL_XP
        ),
        "/",
    )
    .await;
}
