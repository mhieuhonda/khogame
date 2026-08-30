//! v3.1.0 — Handlers game Nối từ (/word-chain).
//!
//! 2 endpoint:
//! - GET /word-chain — trang chơi (form nhập từ).
//! - POST /word-chain/play — HTMX endpoint, body form `word=...`,
//!   trả partial kết quả (hợp lệ/không + bot phản hồi).

use crate::error::{AppError, AppResult};
use crate::middleware::{AuthUser, CurrentUser};
use crate::repositories::{GamificationRepo, WordChainRepo};
use crate::state::AppState;
use crate::templates::WordChainTemplate;
use axum::extract::State;
use serde::Deserialize;
use std::sync::Arc;

/// GET /word-chain — trang game (yêu cầu đăng nhập).
/// # Errors
/// Trả lỗi khi chưa đăng nhập / DB fail.
pub async fn word_chain_page(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<WordChainTemplate> {
    let Some(user) = current_user else {
        return Err(AppError::Unauthorized);
    };
    let plays_today = WordChainRepo::plays_today_count(&state.db, user.id)
        .await
        .unwrap_or(0);
    let valid_lifetime = WordChainRepo::valid_lifetime_count(&state.db, user.id)
        .await
        .unwrap_or(0);
    let level = GamificationRepo::level_of(&state.db, user.id)
        .await
        .unwrap_or_else(|_| crate::models::gamification::level_from_xp(0));
    let unread = crate::handlers::auth::unread_count(&state, user.id).await;
    Ok(WordChainTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        plays_today,
        valid_lifetime,
        level,
    })
}

#[derive(Debug, Deserialize)]
pub struct WordChainPlayForm {
    pub word: String,
}

/// POST /word-chain/play — chơi 1 lượt (HTMX). Trả partial kết quả.
/// # Errors
/// Trả lỗi khi chưa đăng nhập / quá cap / DB fail.
pub async fn play_word_chain(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    axum::extract::Form(form): axum::extract::Form<WordChainPlayForm>,
) -> AppResult<axum::response::Html<String>> {
    use crate::repositories::word_chain::WORD_CHAIN_DAILY_CAP;
    // rand_val để bot chọn từ deterministic — dùng rand crate.
    use rand::RngExt;
    let rand_val: i32 = rand::rng().random_range(0..1000);
    let result = WordChainRepo::play(&state.db, user.id, &form.word, rand_val).await?;
    // Spawn achievement check (word_chain_X có thể chạm ngưỡng)
    let db = state.db.clone();
    let uid = user.id;
    tokio::spawn(async move {
        crate::services::gamification::check_achievements(&db, uid).await;
    });
    let (cls, status_label, body) = if result.is_valid {
        let bot_html = if let Some(bw) = &result.bot_word {
            format!(
                "<div class='wc-bot-response'><span class='wc-bot-label'>Bot nối từ:</span> \
                   <strong class='wc-bot-word'>{}</strong></div>",
                crate::utils::html_escape(bw)
            )
        } else {
            String::new()
        };
        (
            "wc-result wc-valid",
            "✅ Hợp lệ!",
            format!(
                "<div class='wc-user-word'>Bạn nối: <strong>{}</strong></div>{bot_html}",
                crate::utils::html_escape(&result.user_word)
            ),
        )
    } else {
        let reason = result
            .invalid_reason
            .as_deref()
            .unwrap_or("Từ không hợp lệ");
        (
            "wc-result wc-invalid",
            "❌ Không hợp lệ",
            format!(
                "<div class='wc-user-word'>Bạn gõ: <strong>{}</strong></div>\
                 <p class='wc-reason'>{} — thử từ tiếng Việt phổ biến khác (gợi ý: yeu, anh, hoc, troi, mai, ...).</p>",
                crate::utils::html_escape(&result.user_word),
                crate::utils::html_escape(reason)
            ),
        )
    };
    let xp_toast = if result.xp_awarded > 0 {
        format!("+{} XP", result.xp_awarded)
    } else {
        String::new()
    };
    Ok(axum::response::Html(format!(
        "<div class='{cls}' data-xp-toast=\"{xp_toast}\">\
           <p class='wc-status'>{status_label}</p>\
           {body}\
           <p class='wc-stats'>Hôm nay: {}/{} lượt · Hợp lệ lifetime: {} · Tổng XP: {} · Cấp {} — {}</p>\
         </div>",
        result.plays_today,
        WORD_CHAIN_DAILY_CAP,
        result.valid_lifetime,
        result.total_xp,
        result.level.level,
        result.level.title,
    )))
}
