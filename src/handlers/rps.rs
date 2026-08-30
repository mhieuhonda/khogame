//! v3.1.0 — Handlers game Oẳn tù tì / Kéo búa bao (/rps).
//!
//! 2 endpoint:
//! - GET /rps — trang chơi (3 nút chọn Búa/Bao/Kéo).
//! - POST /rps/play — HTMX endpoint, body form `choice=rock|paper|scissors`,
//!   trả partial kết quả (user choice vs bot choice + XP nếu thắng).

use crate::error::{AppError, AppResult};
use crate::middleware::{AuthUser, CurrentUser};
use crate::repositories::RpsRepo;
use crate::state::AppState;
use crate::templates::RpsTemplate;
use axum::extract::State;
use serde::Deserialize;
use std::sync::Arc;

/// GET /rps — trang game (yêu cầu đăng nhập).
/// # Errors
/// Trả lỗi khi chưa đăng nhập / DB fail.
pub async fn rps_page(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<RpsTemplate> {
    let Some(user) = current_user else {
        return Err(AppError::Unauthorized);
    };
    let plays_today = RpsRepo::plays_today_count(&state.db, user.id)
        .await
        .unwrap_or(0);
    let wins_lifetime = RpsRepo::wins_lifetime(&state.db, user.id)
        .await
        .unwrap_or(0);
    let level = crate::repositories::GamificationRepo::level_of(&state.db, user.id)
        .await
        .unwrap_or_else(|_| crate::models::gamification::level_from_xp(0));
    let unread = crate::handlers::auth::unread_count(&state, user.id).await;
    Ok(RpsTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        plays_today,
        wins_lifetime,
        level,
    })
}

#[derive(Debug, Deserialize)]
pub struct RpsPlayForm {
    pub choice: String,
}

/// POST /rps/play — chơi 1 ván (HTMX). Trả partial kết quả.
/// # Errors
/// Trả lỗi khi chưa đăng nhập / choice không hợp lệ / DB fail.
pub async fn play_rps(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    axum::extract::Form(form): axum::extract::Form<RpsPlayForm>,
) -> AppResult<axum::response::Html<String>> {
    use crate::repositories::rps::{RpsChoice, RpsOutcome, RPS_DAILY_CAP};
    let user_choice = RpsChoice::from_form(&form.choice).ok_or_else(|| {
        AppError::BadRequest("Lựa chọn không hợp lệ — phải là rock/paper/scissors".into())
    })?;
    // Bot random — use rand 0..3 (deterministic theo system entropy).
    use rand::RngExt;
    let rand_val: i32 = rand::rng().random_range(0..1000);
    let bot_choice = RpsChoice::random(rand_val);
    let result = RpsRepo::play(&state.db, user.id, user_choice, bot_choice).await?;
    // Spawn achievement check (best-effort — rps_X_wins có thể chạm ngưỡng)
    let db = state.db.clone();
    let uid = user.id;
    tokio::spawn(async move {
        crate::services::gamification::check_achievements(&db, uid).await;
    });
    let outcome_label = match result.outcome {
        RpsOutcome::Win => "🎉 Bạn thắng!",
        RpsOutcome::Lose => "😅 Bạn thua!",
        RpsOutcome::Draw => "🤝 Hòa!",
    };
    let outcome_cls = match result.outcome {
        RpsOutcome::Win => "rps-result rps-win",
        RpsOutcome::Lose => "rps-result rps-lose",
        RpsOutcome::Draw => "rps-result rps-draw",
    };
    let xp_toast = if result.xp_awarded > 0 {
        format!("+{} XP", result.xp_awarded)
    } else {
        String::new()
    };
    Ok(axum::response::Html(format!(
        "<div class='{outcome_cls}' data-xp-toast=\"{xp_toast}\">\
           <div class='rps-choices'>\
             <div class='rps-hand rps-user'><span class='rps-emoji'>{}</span><span class='rps-label'>Bạn — {}</span></div>\
             <div class='rps-vs'>VS</div>\
             <div class='rps-hand rps-bot'><span class='rps-emoji'>{}</span><span class='rps-label'>Bot — {}</span></div>\
           </div>\
           <p class='rps-outcome'>{outcome_label}</p>\
           <p class='rps-stats'>Hôm nay: {}/{} ván · Thắng lifetime: {} · Tổng XP: {} · Cấp {} — {}</p>\
         </div>",
        result.user_choice.emoji(),
        result.user_choice.label(),
        result.bot_choice.emoji(),
        result.bot_choice.label(),
        result.plays_today,
        RPS_DAILY_CAP,
        result.wins_lifetime,
        result.total_xp,
        result.level.level,
        result.level.title,
    )))
}
