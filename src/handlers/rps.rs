//! v3.1.0 → v3.3.0 — Handlers game Oẳn tù tì / Kéo búa bao (/rps).
//!
//! v3.3.0 — PVP MATCHMAKING: mỗi nước chơi ghép với NGƯỜI DÙNG NGẪU
//! NHIÊN (không còn đấu với bot). State nằm trong PostgreSQL:
//! - POST `/rps/play` — join match `waiting` của người khác (resolve
//!   NGAY) hoặc tạo hàng chờ kèm HTMX poll mỗi 3s.
//! - GET `/rps/match/{id}/status` — poll: hết 90s không có người thì tự
//!   ghép GLM 5.3 (AI Agent mặc định) và resolve.
//!
//! Mỗi ván vẫn ghi `rps_plays` — thống kê + huy hiệu `rps_*` giữ nguyên.

use crate::error::{AppError, AppResult};
use crate::middleware::{AuthUser, CurrentUser};
use crate::repositories::rps::{RpsChoice, RpsOutcome, RpsPvpStatus, RPS_DAILY_CAP};
use crate::repositories::{GamificationRepo, RpsRepo};
use crate::state::AppState;
use crate::templates::RpsTemplate;
use axum::extract::{Path, State};
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
    let level = GamificationRepo::level_of(&state.db, user.id)
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

/// POST /rps/play — PvP: ghép ngẫu nhiên người dùng khác (HTMX).
/// Trả partial kết quả (nếu ghép được ngay) hoặc partial "đang tìm
/// người" với poller tự động.
/// # Errors
/// Trả lỗi khi chưa đăng nhập / choice không hợp lệ / quá daily cap / DB fail.
pub async fn play_rps(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    axum::extract::Form(form): axum::extract::Form<RpsPlayForm>,
) -> AppResult<axum::response::Html<String>> {
    let user_choice = RpsChoice::from_form(&form.choice).ok_or_else(|| {
        AppError::BadRequest("Lựa chọn không hợp lệ — phải là rock/paper/scissors".into())
    })?;
    let status = RpsRepo::pvp_play(&state.db, user.id, user_choice).await?;
    if matches!(status, RpsPvpStatus::Resolved { .. }) {
        spawn_achievement_checks(&state, user.id, &status);
    }
    Ok(axum::response::Html(render_status(status)))
}

/// GET /rps/match/{id}/status — HTMX poll (mỗi 3s khi đang chờ ghép).
/// # Errors
/// Trả lỗi khi không thuộc match / DB fail.
pub async fn match_status(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(match_id): Path<i64>,
) -> AppResult<axum::response::Html<String>> {
    let status = RpsRepo::pvp_status(&state.db, user.id, match_id).await?;
    if matches!(status, RpsPvpStatus::Resolved { .. }) {
        spawn_achievement_checks(&state, user.id, &status);
    }
    Ok(axum::response::Html(render_status(status)))
}

/// Check huy hiệu cho cả tôi và đối thủ (best-effort, fire-and-forget).
/// Đối thủ là AI (GLM 5.3) thì bỏ qua — AI không cần huy hiệu.
fn spawn_achievement_checks(state: &AppState, me: uuid::Uuid, status: &RpsPvpStatus) {
    if let RpsPvpStatus::Resolved { opponent, .. } = status {
        let db = state.db.clone();
        let opp_id = opponent.user_id;
        let opp_is_ai = opponent.is_ai;
        tokio::spawn(async move {
            crate::services::gamification::check_achievements(&db, me).await;
            if !opp_is_ai {
                crate::services::gamification::check_achievements(&db, opp_id).await;
            }
        });
    }
}

// ============================================================
// Partial rendering — root luôn có id="rps-result", mọi swap dùng
// outerHTML (poller tự thay thế chính nó qua hx-target="#rps-result").
// ============================================================

fn render_status(status: RpsPvpStatus) -> String {
    match status {
        RpsPvpStatus::Waiting {
            match_id,
            wait_secs,
        } => render_waiting(match_id, wait_secs),
        RpsPvpStatus::Resolved {
            my_choice,
            opponent_choice,
            outcome,
            xp_awarded,
            total_xp,
            level,
            plays_today,
            wins_lifetime,
            opponent,
            is_ai_fallback,
        } => render_resolved(
            my_choice,
            opponent_choice,
            outcome,
            xp_awarded,
            total_xp,
            plays_today,
            wins_lifetime,
            &level.level.to_string(),
            level.title,
            &opponent.display_name,
            opponent.is_ai,
            is_ai_fallback,
        ),
        RpsPvpStatus::Cancelled => render_cancelled(),
    }
}

/// Partial đang tìm người — poller tự thay #rps-result mỗi 3s.
fn render_waiting(match_id: i64, wait_secs: i64) -> String {
    format!(
        "<div id=\"rps-result\" class=\"rps-waiting\" aria-live=\"polite\">\
           <div class=\"rps-spinner\" aria-hidden=\"true\"></div>\
           <p class=\"rps-wait-title\">🔍 Đang tìm người chơi ngẫu nhiên...</p>\
           <p class=\"rps-wait-sub\">Chọn đã ghi — tự ghép với GLM 5.3 sau ~{wait_secs}s nếu không có ai.</p>\
           <div class=\"rps-poller\" hx-get=\"/rps/match/{match_id}/status\" \
                hx-trigger=\"every 3s\" hx-target=\"#rps-result\" hx-swap=\"outerHTML\"></div>\
         </div>"
    )
}

#[allow(clippy::too_many_arguments)]
fn render_resolved(
    my_choice: RpsChoice,
    opponent_choice: RpsChoice,
    outcome: RpsOutcome,
    xp_awarded: i32,
    total_xp: i64,
    plays_today: i64,
    wins_lifetime: i64,
    level_num: &str,
    level_title: &str,
    opponent_name: &str,
    opponent_is_ai: bool,
    is_ai_fallback: bool,
) -> String {
    let outcome_label = match outcome {
        RpsOutcome::Win => "🎉 Bạn thắng!",
        RpsOutcome::Lose => "😅 Bạn thua!",
        RpsOutcome::Draw => "🤝 Hòa!",
    };
    let outcome_cls = match outcome {
        RpsOutcome::Win => "rps-result rps-win",
        RpsOutcome::Lose => "rps-result rps-lose",
        RpsOutcome::Draw => "rps-result rps-draw",
    };
    let opp_label = if opponent_is_ai || is_ai_fallback {
        format!("{} (AI)", crate::utils::html_escape(opponent_name))
    } else {
        crate::utils::html_escape(opponent_name)
    };
    let xp_toast = if xp_awarded > 0 {
        format!("+{xp_awarded} XP")
    } else {
        String::new()
    };
    // Confetti khi thắng — CSS animation (kg-confetti-fall), 12 mảnh.
    let confetti = if outcome == RpsOutcome::Win {
        let colors = [
            "#22c55e", "#0ea5e9", "#f59e0b", "#7c3aed", "#ef4444", "#14b8a6",
        ];
        let bits: Vec<String> = (0..12)
            .map(|i| {
                let left = 5 + i * 8;
                let color = colors[i as usize % colors.len()];
                let x = (i % 2 * 2 - 1) * (20 + i * 3);
                format!(
                    "<span class=\"confettiPiece\" style=\"left:{left}%;background:{color};\
                     --confetti-x:{x}px;animation-delay:.{}s\"></span>",
                    (i % 5) as f32 * 0.08
                )
            })
            .collect();
        format!(
            "<div class=\"rps-confetti\" aria-hidden=\"true\">{}</div>",
            bits.join("")
        )
    } else {
        String::new()
    };
    format!(
        "<div id=\"rps-result\" class='{outcome_cls}' data-xp-toast=\"{xp_toast}\">\
           {confetti}\
           <div class='rps-choices'>\
             <div class='rps-hand rps-user'><span class='rps-emoji'>{}</span><span class='rps-label'>Bạn — {}</span></div>\
             <div class='rps-vs'>VS</div>\
             <div class='rps-hand rps-bot'><span class='rps-emoji'>{}</span><span class='rps-label'>{opp_label} — {}</span></div>\
           </div>\
           <p class='rps-outcome'>{outcome_label}</p>\
           <p class='rps-stats'>Hôm nay: {plays_today}/{RPS_DAILY_CAP} ván · Thắng lifetime: {wins_lifetime} · Tổng XP: {total_xp} · Cấp {level_num} — {level_title}</p>\
         </div>",
        my_choice.emoji(),
        my_choice.label(),
        opponent_choice.emoji(),
        opponent_choice.label(),
    )
}

fn render_cancelled() -> String {
    "<div id=\"rps-result\" class='rps-result rps-draw' aria-live=\"polite\">\
       <p class='rps-outcome'>Trận đã huỷ — người chơi kia rời hàng chờ.</p>\
       <p class='rps-stats'>Chọn lại bất kỳ để tìm đối thủ mới.</p>\
     </div>"
        .to_string()
}
