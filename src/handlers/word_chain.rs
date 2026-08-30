//! v3.1.0 → v3.3.0 — Handlers game Nối từ (/word-chain).
//!
//! v3.3.0 — PVP MATCHMAKING: ghép 2 người dùng ngẫu nhiên thật thay vì
//! đấu với bot. State nằm 100% trong PostgreSQL (`word_chain_matches`,
//! migration 026) — an toàn với restart/multi-process:
//! - POST /word-chain/match — vào hàng ghép (join match chờ của người
//!   khác hoặc tạo hàng mới). Hết 120s không có người → tự ghép GLM 5.3.
//! - POST /word-chain/move — đánh 1 từ trong trận đang chạy.
//! - GET /word-chain/match/{id}/status — HTMX poll (3s): cập nhật luân
//!   phiên, thực thi timeout (hết 90s không đánh = thua) server-side.

use crate::error::{AppError, AppResult};
use crate::middleware::{AuthUser, CurrentUser};
use crate::repositories::word_chain::{
    WordChainPvpStatus, WORD_CHAIN_DAILY_CAP, WORD_CHAIN_MOVE_SECS, WORD_CHAIN_PVP_WAIT_SECS,
};
use crate::repositories::{GamificationRepo, WordChainRepo};
use crate::state::AppState;
use crate::templates::WordChainTemplate;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::sync::Arc;

/// GET /word-chain — trang game (yêu cầu đăng nhập).
///
/// v3.4.0 — khi `ARCADE_UNDER_REVIEW = true`, render trang "tính năng
/// đang được Hieu Louis xem xét" thay vì UI chơi (game tạm dừng).
/// # Errors
/// Trả lỗi khi chưa đăng nhập / DB fail.
pub async fn word_chain_page(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<Response> {
    let Some(user) = current_user else {
        return Err(AppError::Unauthorized);
    };
    if crate::handlers::ARCADE_UNDER_REVIEW {
        let unread = crate::handlers::auth::unread_count(&state, user.id).await;
        return Ok(crate::templates::ArcadeReviewTemplate {
            current_user: Some(user),
            unread_notifications: unread,
            game_title: "Nối từ tiếng Việt".into(),
            game_emoji: "🔤".into(),
        }
        .into_response());
    }
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
    }
    .into_response())
}

#[derive(Debug, Deserialize)]
pub struct WordChainMoveForm {
    pub word: String,
}

/// POST /word-chain/match — vào hàng ghép ngẫu nhiên (HTMX).
/// # Errors
/// Trả lỗi khi chưa đăng nhập / DB fail.
pub async fn find_match(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<axum::response::Html<String>> {
    // v3.4.0 — arcade tạm dừng chờ Hieu Louis xem xét
    if crate::handlers::ARCADE_UNDER_REVIEW {
        return Err(AppError::Forbidden(
            "Tính năng đang được Hieu Louis xem xét — sẽ sớm quay lại với bản cập nhật fix lỗi và tính năng mới!".into(),
        ));
    }
    let status = WordChainRepo::pvp_join_or_create(&state.db, user.id).await?;
    Ok(axum::response::Html(render_state(status)))
}

/// POST /word-chain/move — đánh 1 từ (HTMX).
///
/// v3.4.0 — gate ARCADE_UNDER_REVIEW (trước đây chỉ gate find_match →
/// trận dở tạo trước deploy vẫn chơi tiếp + cộng XP — audit v3.4.0).
/// # Errors
/// Trả lỗi khi chưa đăng nhập / không có trận / chưa đến lượt / DB fail.
pub async fn move_word(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    axum::extract::Form(form): axum::extract::Form<WordChainMoveForm>,
) -> AppResult<axum::response::Html<String>> {
    // v3.4.0 — arcade tạm dừng chờ Hieu Louis xem xét (chặn cả trận dở)
    if crate::handlers::ARCADE_UNDER_REVIEW {
        return Err(AppError::Forbidden(
            "Tính năng đang được Hieu Louis xem xét — sẽ sớm quay lại với bản cập nhật fix lỗi và tính năng mới!".into(),
        ));
    }

    let status = WordChainRepo::pvp_move(&state.db, user.id, &form.word).await?;
    // Spawn achievement check (word_chain_X có thể chạm ngưỡng).
    let db = state.db.clone();
    let uid = user.id;
    tokio::spawn(async move {
        crate::services::gamification::check_achievements(&db, uid).await;
    });
    Ok(axum::response::Html(render_state(status)))
}

/// GET /word-chain/match/{id}/status — HTMX poll (mỗi 3s khi chờ luân
/// phiên). Thực thi timeout + fallback AI server-side.
/// # Errors
/// Trả lỗi khi chưa đăng nhập / không thuộc match / DB fail.
pub async fn match_status(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(match_id): Path<i64>,
) -> AppResult<axum::response::Html<String>> {
    let status = WordChainRepo::pvp_status(&state.db, user.id, match_id).await?;
    if matches!(status, WordChainPvpStatus::Finished { .. }) {
        // Kết thúc trận — check huy hiệu (word_chain_win không có tier
        // riêng nhưng valid count có thể chạm ngưỡng).
        let db = state.db.clone();
        let uid = user.id;
        tokio::spawn(async move {
            crate::services::gamification::check_achievements(&db, uid).await;
        });
    }
    Ok(axum::response::Html(render_state(status)))
}

// ============================================================
// Partial rendering (root luôn có id="wc-state" — mọi swap dùng
// outerHTML để thay thế nguyên khối, kể cả poller).
// ============================================================

fn render_state(status: WordChainPvpStatus) -> String {
    match status {
        WordChainPvpStatus::Waiting {
            match_id,
            wait_secs,
        } => render_waiting(match_id, wait_secs),
        WordChainPvpStatus::Active {
            match_id,
            my_turn,
            letter,
            words,
            opponent,
            is_ai,
            deadline_secs,
            plays_today,
            valid_lifetime,
            total_xp,
            level,
            notice,
        } => render_active(
            match_id,
            my_turn,
            letter,
            &words,
            &opponent.display_name,
            is_ai,
            deadline_secs,
            plays_today,
            valid_lifetime,
            total_xp,
            level.level.to_string().as_str(),
            level.title,
            notice.as_deref(),
        ),
        WordChainPvpStatus::Finished {
            winner_is_me,
            reason,
            words,
            opponent,
            is_ai,
            total_xp,
            level,
            plays_today,
            valid_lifetime,
            ..
        } => render_finished(
            winner_is_me,
            &reason,
            &words,
            &opponent.display_name,
            is_ai,
            total_xp,
            level.level.to_string().as_str(),
            level.title,
            plays_today,
            valid_lifetime,
        ),
        WordChainPvpStatus::Cancelled => render_cancelled(),
    }
}

/// Partial đang chờ ghép — có poller tự thay thế #wc-state mỗi 3s.
fn render_waiting(match_id: i64, wait_secs: i64) -> String {
    format!(
        "<div id=\"wc-state\" class=\"wc-state wc-waiting\">\
           <div class=\"wc-spinner\" aria-hidden=\"true\"></div>\
           <p class=\"wc-wait-title\">🔍 Đang tìm người chơi ngẫu nhiên...</p>\
           <p class=\"wc-wait-sub\">Tự động ghép với GLM 5.3 sau ~{wait_secs}s nữa nếu không có ai vào hàng.</p>\
           <div class=\"wc-poller\" hx-get=\"/word-chain/match/{match_id}/status\" \
                hx-trigger=\"every 3s\" hx-target=\"#wc-state\" hx-swap=\"outerHTML\"></div>\
         </div>"
    )
}

/// Partial trận đang chạy: chuỗi từ + chữ nối + form (nếu tới lượt) /
/// poller (nếu chờ đối thủ). Poller LUÔN có khi không phải lượt mình —
/// server sẽ tự xử timeout.
#[allow(clippy::too_many_arguments)]
fn render_active(
    match_id: i64,
    my_turn: bool,
    letter: Option<char>,
    words: &[String],
    opponent_name: &str,
    is_ai: bool,
    deadline_secs: Option<i64>,
    plays_today: i64,
    valid_lifetime: i64,
    total_xp: i64,
    level_num: &str,
    level_title: &str,
    notice: Option<&str>,
) -> String {
    let opp_badge = if is_ai {
        "<span class=\"wc-ai-badge\">🤖 AI</span>"
    } else {
        ""
    };
    let letter_html = match letter {
        Some(l) => format!(
            "<div class=\"wc-letter\">Nối từ bắt đầu bằng chữ <strong class=\"wc-letter-ch\">{}</strong></div>",
            crate::utils::html_escape(l.to_uppercase().to_string().as_str())
        ),
        None => "<div class=\"wc-letter\">Nước đầu — đánh từ bất kỳ trong từ điển</div>".into(),
    };
    let chain_html = if words.is_empty() {
        String::new()
    } else {
        let items: Vec<String> = words
            .iter()
            .map(|w| {
                format!(
                    "<span class=\"wc-word\">{}</span>",
                    crate::utils::html_escape(w)
                )
            })
            .collect();
        format!(
            "<div class=\"wc-chain\">{}</div>",
            items.join("<span class=\"wc-chain-arrow\">→</span>")
        )
    };
    let deadline_html = deadline_secs
        .map(|s| format!("<span class=\"wc-deadline\">⏱ còn ~{s}s</span>"))
        .unwrap_or_default();
    let notice_html = notice
        .map(|n| {
            format!(
                "<p class=\"wc-notice\">{}</p>",
                crate::utils::html_escape(n)
            )
        })
        .unwrap_or_default();

    let action_html = if my_turn {
        format!(
            "<form class=\"wc-form\" hx-post=\"/word-chain/move\" hx-target=\"#wc-state\" \
                 hx-swap=\"outerHTML\" hx-disabled-elt=\"button.wc-submit\">\
               <input type=\"text\" name=\"word\" maxlength=\"100\" required \
                    placeholder=\"Gõ từ bắt đầu bằng {letter_hint}...\" autocomplete=\"off\" \
                    class=\"wc-input\" aria-label=\"Từ cần nối\">\
               <button type=\"submit\" class=\"btn btn-primary wc-submit\">Nối!</button>\
             </form>",
            letter_hint = letter
                .map(|l| l.to_string())
                .unwrap_or_else(|| "bất kỳ".into()),
        )
    } else {
        format!(
            "<p class=\"wc-turn-wait\">Đang chờ <strong>{}</strong> đánh... {}</p>\
             <div class=\"wc-poller\" hx-get=\"/word-chain/match/{match_id}/status\" \
                  hx-trigger=\"every 3s\" hx-target=\"#wc-state\" hx-swap=\"outerHTML\"></div>",
            crate::utils::html_escape(opponent_name),
            deadline_html,
        )
    };

    let turn_cls = if my_turn {
        "wc-my-turn"
    } else {
        "wc-their-turn"
    };
    let tail_html = format!("{notice_html}{action_html}");

    format!(
        "<div id=\"wc-state\" class=\"wc-state wc-active {turn_cls}\">\
           <div class=\"wc-opponent\">Đối thủ: <strong>{}</strong> {} {}</div>\
           {}{}{}\
           <p class=\"wc-stats\">Hôm nay: {}/{} lượt · Hợp lệ lifetime: {} · Tổng XP: {} · Cấp {} — {}</p>\
         </div>",
        crate::utils::html_escape(opponent_name),
        opp_badge,
        if my_turn { "<span class=\"wc-my-turn\">✅ TỚI LƯỢT BẠN</span>" } else { "" },
        letter_html,
        chain_html,
        tail_html,
        plays_today,
        WORD_CHAIN_DAILY_CAP,
        valid_lifetime,
        total_xp,
        crate::utils::html_escape(level_num),
        crate::utils::html_escape(level_title),
    )
}

/// Partial kết thúc trận + nút tìm trận mới.
#[allow(clippy::too_many_arguments)]
fn render_finished(
    winner_is_me: bool,
    reason: &str,
    words: &[String],
    opponent_name: &str,
    is_ai: bool,
    total_xp: i64,
    level_num: &str,
    level_title: &str,
    plays_today: i64,
    valid_lifetime: i64,
) -> String {
    let cls = if winner_is_me {
        "wc-result wc-valid"
    } else {
        "wc-result wc-invalid"
    };
    let xp_toast = if winner_is_me { "+4 XP" } else { "" };
    let items: Vec<String> = words
        .iter()
        .map(|w| {
            format!(
                "<span class=\"wc-word\">{}</span>",
                crate::utils::html_escape(w)
            )
        })
        .collect();
    let chain_html = if items.is_empty() {
        String::new()
    } else {
        format!(
            "<div class=\"wc-chain\">{}</div>",
            items.join("<span class=\"wc-chain-arrow\">→</span>")
        )
    };
    let ai_tag = if is_ai { " 🤖" } else { "" };
    format!(
        "<div id=\"wc-state\" class=\"{cls}\" data-xp-toast=\"{xp_toast}\">\
           <p class=\"wc-status\">{}</p>\
           <p class=\"wc-opponent-line\">Đối thủ: <strong>{}{}</strong></p>\
           {}\
           <p class=\"wc-stats\">Hôm nay: {}/{} lượt · Hợp lệ lifetime: {} · Tổng XP: {} · Cấp {} — {}</p>\
           <button class=\"btn btn-primary\" hx-post=\"/word-chain/match\" \
                hx-target=\"#wc-state\" hx-swap=\"outerHTML\" hx-disabled-elt=\"this\">\
             🔄 Tìm trận mới\
           </button>\
         </div>",
        crate::utils::html_escape(reason),
        crate::utils::html_escape(opponent_name),
        ai_tag,
        chain_html,
        plays_today,
        WORD_CHAIN_DAILY_CAP,
        valid_lifetime,
        total_xp,
        crate::utils::html_escape(level_num),
        crate::utils::html_escape(level_title),
    )
}

fn render_cancelled() -> String {
    "<div id=\"wc-state\" class=\"wc-state wc-cancelled\">\
       <p class=\"wc-notice\">Trận đã huỷ (đối thủ rời hàng chờ). Bấm nút để tìm trận mới.</p>\
       <button class=\"btn btn-primary\" hx-post=\"/word-chain/match\" \
            hx-target=\"#wc-state\" hx-swap=\"outerHTML\" hx-disabled-elt=\"this\">\
         🔍 Tìm đối thủ ngẫu nhiên\
       </button>\
     </div>"
        .to_string()
}

/// Hằng số cho template hiển thị (đề phòng cần render deadline).
const _: () = {
    assert!(WORD_CHAIN_PVP_WAIT_SECS > 0);
    assert!(WORD_CHAIN_MOVE_SECS > 0);
};
