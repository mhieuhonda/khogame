//! v3.0.0 — Handlers nhiệm vụ hằng ngày/tuần (daily/weekly quests).

use crate::error::{AppError, AppResult};
use crate::middleware::{AuthUser, CurrentUser};
use crate::repositories::QuestRepo;
use crate::state::AppState;
use crate::templates::QuestsTemplate;
use axum::extract::State;
use std::sync::Arc;

/// GET /quests — trang nhiệm vụ (yêu cầu đăng nhập).
/// # Errors
/// Trả lỗi khi DB fail.
pub async fn quests_page(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<QuestsTemplate> {
    let Some(user) = current_user else {
        return Err(AppError::Unauthorized);
    };
    let quests = QuestRepo::today_quests(&state.db, user.id).await?;
    let level = crate::repositories::GamificationRepo::level_of(&state.db, user.id)
        .await
        .unwrap_or_else(|_| crate::models::gamification::level_from_xp(0));
    let unread = crate::handlers::auth::unread_count(&state, user.id).await;
    Ok(QuestsTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        quests,
        level,
    })
}

/// POST /quests/{id}/claim — nhận thưởng 1 nhiệm vụ đã hoàn thành.
/// Trả partial HTMX: nút đổi thành "Đã nhận +N XP".
/// # Errors
/// Trả lỗi khi chưa hoàn thành / đã nhận / DB fail.
pub async fn claim_quest(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    axum::extract::Path(quest_id): axum::extract::Path<String>,
) -> AppResult<axum::response::Html<String>> {
    let (xp, _total, _level) = QuestRepo::claim(&state.db, user.id, &quest_id).await?;
    // Huy hiệu level có thể mới chạm ngưỡng sau claim — best-effort
    let db = state.db.clone();
    let uid = user.id;
    tokio::spawn(async move {
        crate::services::gamification::check_achievements(&db, uid).await;
    });
    Ok(axum::response::Html(format!(
        "<span class='quest-claimed' data-xp-toast=\"+{xp} XP\">✓ Đã nhận +{xp} XP</span>"
    )))
}
