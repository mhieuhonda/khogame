//! v2.9.0 — Handlers gamification: điểm danh, bảng xếp hạng, huy hiệu,
//! dòng thời gian người theo dõi, game ngẫu nhiên.

use crate::error::{AppError, AppResult};
use crate::middleware::AuthUser;
use crate::models::gamification::{AchievementWithStatus, LeaderboardEntry, LevelInfo};
use crate::models::GameCard;
use crate::repositories::{GamificationRepo, ViewHistoryRepo};
use crate::services::gamification as gsvc;
use crate::state::AppState;
use crate::templates::{AchievementsTemplate, FollowingTemplate, LeaderboardTemplate};
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect};
use serde::Deserialize;
use std::sync::Arc;

/// Dữ liệu widget điểm danh (nhúng IndexTemplate + partial HTMX).
#[derive(Debug, Clone, Template)]
#[template(path = "partials/checkin_widget.html")]
pub struct CheckinWidget {
    pub current_user: Option<crate::models::user::User>,
    pub checked_in_today: bool,
    pub current_streak: i32,
    pub level: LevelInfo,
}

/// Partial sau khi bấm điểm danh (HTMX swap).
#[derive(Debug, Clone, Template)]
#[template(path = "partials/checkin_partial.html")]
pub struct CheckinPartial {
    pub success: bool,
    pub streak: i32,
    pub xp_awarded: i32,
    pub level: LevelInfo,
    pub already: bool,
}

/// Lấy trạng thái điểm danh của user (helper dùng chung homepage).
pub async fn checkin_status(
    state: &AppState,
    user: &crate::models::user::User,
) -> (bool, i32, LevelInfo) {
    let checked_in = GamificationRepo::today_checkin(&state.db, user.id)
        .await
        .ok()
        .flatten();
    let streak = GamificationRepo::current_streak(&state.db, user.id)
        .await
        .unwrap_or(0);
    let level = GamificationRepo::level_of(&state.db, user.id)
        .await
        .unwrap_or(crate::models::gamification::level_from_xp(0));
    (checked_in.is_some(), streak, level)
}

/// POST /checkin — điểm danh hôm nay (HTMX trả partial).
/// # Errors
/// Trả lỗi khi chưa đăng nhập hoặc DB fail.
pub async fn do_checkin(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<Html<String>> {
    let (streak, xp_awarded, level) = GamificationRepo::do_checkin(&state.db, user.id).await?;
    // Huy hiệu streak (3/7/30) + level_5/10 — best-effort
    gsvc::check_achievements(&state.db, user.id).await;
    // Đã điểm từ trước (idempotent re-click): xp_awarded == 0 và có streak
    let already = xp_awarded == 0;
    let partial = CheckinPartial {
        success: true,
        streak,
        xp_awarded,
        level,
        already,
    };
    Ok(Html(partial.render()?))
}

/// GET /leaderboard — bảng xếp hạng: top cấp độ + game hot tuần.
/// # Errors
/// Trả lỗi khi DB fail.
pub async fn leaderboard(
    State(state): State<Arc<AppState>>,
    crate::middleware::CurrentUser(current_user): crate::middleware::CurrentUser,
) -> AppResult<LeaderboardTemplate> {
    let (top_users, hot_games) = tokio::join!(
        GamificationRepo::leaderboard_top_xp(&state.db, 20),
        crate::repositories::GameRepo::hot_this_week(&state.db, 10),
    );
    let entries: Vec<LeaderboardEntry> = top_users?;
    let hot_games = hot_games?;
    let unread = match current_user.as_ref() {
        Some(u) => crate::handlers::auth::unread_count(&state, u.id).await,
        None => 0,
    };
    Ok(LeaderboardTemplate {
        current_user,
        unread_notifications: unread,
        entries,
        hot_games,
    })
}

/// GET /achievements — trang huy hiệu cá nhân (yêu cầu đăng nhập).
/// # Errors
/// Trả lỗi khi chưa đăng nhập hoặc DB fail.
pub async fn achievements_page(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<AchievementsTemplate> {
    let (list, level, streak) = tokio::join!(
        GamificationRepo::achievements_with_status(&state.db, user.id),
        GamificationRepo::level_of(&state.db, user.id),
        GamificationRepo::current_streak(&state.db, user.id),
    );
    let list: Vec<AchievementWithStatus> = list?;
    let level = level?;
    let streak = streak?;
    let earned = list.iter().filter(|a| a.earned_at.is_some()).count();
    let unread = crate::handlers::auth::unread_count(&state, user.id).await;
    Ok(AchievementsTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        achievements: list,
        level,
        streak,
        earned_count: earned,
    })
}

/// POST /achievements/{id}/showcase — ghim/bỏ ghim huy hiệu lên hồ sơ.
/// # Errors
/// Trả lỗi khi chưa đăng nhập, chưa sở hữu huy hiệu, hoặc quá quota.
pub async fn toggle_showcase(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Redirect> {
    GamificationRepo::toggle_showcase(&state.db, user.id, &id).await?;
    Ok(Redirect::to("/achievements"))
}

#[derive(Deserialize, Default)]
pub struct FollowingQuery {
    pub page: Option<i64>,
}

/// GET /following — dòng thời gian game mới từ người mình theo dõi.
/// # Errors
/// Trả lỗi khi chưa đăng nhập hoặc DB fail.
pub async fn following_feed(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Query(q): Query<FollowingQuery>,
) -> AppResult<FollowingTemplate> {
    let page = q.page.unwrap_or(1).clamp(1, 10_000);
    let per_page: i64 = 12;
    let offset = page.saturating_sub(1).saturating_mul(per_page);
    let (games, total) = tokio::join!(
        crate::repositories::InteractionRepo::followed_games(&state.db, user.id, per_page, offset),
        crate::repositories::InteractionRepo::count_followed_games(&state.db, user.id),
    );
    let games: Vec<GameCard> = games?;
    let total: i64 = total.unwrap_or(0);
    let total_pages = ((total + per_page - 1) / per_page).max(1);
    let unread = crate::handlers::auth::unread_count(&state, user.id).await;
    Ok(FollowingTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        games,
        page,
        total_pages,
        total,
    })
}

/// GET /games/random — redirect tới 1 game published ngẫu nhiên.
/// Hỗ trợ khám phá (discovery) — nút "Game ngẫu nhiên".
/// # Errors
/// Trả lỗi khi DB fail hoặc site chưa có game nào.
pub async fn random_game(State(state): State<Arc<AppState>>) -> AppResult<Redirect> {
    let slug: Option<String> = sqlx::query_scalar(
        "SELECT slug FROM games WHERE status = 'published' ORDER BY random() LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await?;
    match slug {
        Some(s) => Ok(Redirect::to(&format!("/games/{s}"))),
        None => Err(AppError::NotFound(
            "Chưa có game nào được xuất bản để khám phá".into(),
        )),
    }
}

/// GET /api/checkin-status — JSON trạng thái điểm danh (dùng nội bộ).
/// # Errors
/// Trả lỗi khi DB fail.
pub async fn checkin_status_api(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<axum::Json<serde_json::Value>> {
    let (checked, streak, level) = checkin_status(&state, &user).await;
    Ok(axum::Json(serde_json::json!({
        "checked_in_today": checked,
        "streak": streak,
        "level": level.level,
        "title": level.title,
        "xp": level.xp,
        "next_level_xp": level.next_level_xp,
        "progress_pct": level.progress_pct,
    })))
}

/// GET /checkin-widget — partial HTML widget điểm danh (HTMX lazy-load).
/// Partial này phụ thuộc phiên → KHÔNG được cache (Cache-Control no-store).
/// # Errors
/// Trả lỗi khi DB fail.
pub async fn checkin_widget(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<axum::response::Response> {
    let (checked, streak, level) = checkin_status(&state, &user).await;
    let widget = CheckinWidget {
        current_user: Some(user),
        checked_in_today: checked,
        current_streak: streak,
        level,
    };
    Ok((
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        Html(widget.render()?),
    )
        .into_response())
}

/// Ghi lịch sử xem game (helper gọi từ show_game — fire-and-forget).
pub fn spawn_record_view(db: sqlx::PgPool, user_id: uuid::Uuid, game_id: uuid::Uuid) {
    tokio::spawn(async move {
        let _ = ViewHistoryRepo::record(&db, user_id, game_id).await;
    });
}
