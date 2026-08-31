use crate::error::{AppError, AppResult};
use crate::middleware::AuthUser;
use crate::models::game::GameStatus;
use crate::repositories::{GameRepo, InteractionRepo};
use crate::state::AppState;
use crate::templates::{
    BookmarkButtonPartial, FollowButtonPartial, LikeButtonPartial, RatingStarsPartial,
};
use askama::Template;
use axum::extract::{Path, State};
use axum::response::Html;
use axum::Form;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct RateForm {
    pub score: i16,
}

/// Chỉ cho tương tác (like/bookmark/rate) trên game đã xuất bản.
/// Owner/staff được miễn chặn để tự kiểm tra game nháp.
/// Trước đây các endpoint này không kiểm tra status.
fn ensure_interactable(
    game: &crate::models::game::Game,
    user: &crate::models::user::User,
) -> AppResult<()> {
    if game.user_id != user.id && !user.role.is_staff() && game.status != GameStatus::Published {
        return Err(AppError::NotFound("Game không tồn tại".into()));
    }
    Ok(())
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn toggle_like(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
) -> AppResult<Html<String>> {
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    ensure_interactable(&game, &user)?;
    let (is_liked, first_ever) = InteractionRepo::toggle_like(&state.db, game.id, user.id).await?;
    // v2.9.0 — XP cho chủ game + huy hiệu discovery (best-effort)
    if is_liked {
        let db = state.db.clone();
        let (actor, owner) = (user.id, game.user_id);
        tokio::spawn(async move {
            crate::services::gamification::on_like(&db, actor, owner).await;
        });
        // v3.0.0 — quest like_game + heatmap
        // v3.5.1 FIX (audit 5-e F7): chỉ bump quest khi đây là lần ĐẦU user
        // like game này (like_history) — chống farm unlike→like vòng lặp.
        if first_ever {
            let db_ret = state.db.clone();
            let ret_uid = user.id;
            tokio::spawn(async move {
                crate::services::retention::on_action(db_ret, ret_uid, "like_game", 1).await;
            });
        }
    }
    // Đọc lại counter từ DB sau khi toggle để tránh hiển thị giá trị stale
    let like_count = GameRepo::find_by_id(&state.db, game.id)
        .await?
        .map_or(game.like_count, |g| g.like_count);
    let partial = LikeButtonPartial {
        game_id: game.id,
        slug: slug.clone(),
        is_liked,
        like_count,
    };
    Ok(Html(partial.render()?))
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn toggle_bookmark(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
) -> AppResult<Html<String>> {
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    ensure_interactable(&game, &user)?;
    let is_bookmarked = InteractionRepo::toggle_bookmark(&state.db, game.id, user.id).await?;
    // v3.0.0 — onboarding step first_bookmark (chỉ khi ĐANG bookmark)
    if is_bookmarked {
        let db_ob = state.db.clone();
        let uid_ob = user.id;
        tokio::spawn(async move {
            crate::services::retention::onboarding_step(&db_ob, uid_ob, "first_bookmark").await;
        });
    }
    // v2.9.0 — huy hiệu discovery khi bookmark (best-effort)
    if is_bookmarked {
        let db = state.db.clone();
        let uid = user.id;
        tokio::spawn(async move {
            crate::services::gamification::on_bookmark(&db, uid).await;
        });
    }
    let partial = BookmarkButtonPartial {
        game_id: game.id,
        slug: slug.clone(),
        is_bookmarked,
    };
    Ok(Html(partial.render()?))
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn rate(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Form(form): Form<RateForm>,
) -> AppResult<Html<String>> {
    if form.score < 1 || form.score > 5 {
        return Err(AppError::BadRequest("Điểm phải từ 1 đến 5".into()));
    }
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    ensure_interactable(&game, &user)?;
    let first_ever_rating =
        InteractionRepo::set_rating(&state.db, game.id, user.id, form.score).await?;
    // v3.0.0 — quest rate_game + onboarding first_rating + heatmap
    // v3.5.1 FIX (audit 5-e F7): chỉ bump quest khi đây là lần ĐẦU user rate
    // game này — chống farm đổi điểm lặp lại (5★→4★→5★...) hoàn thành quest.
    // Onboarding first_rating vẫn idempotent (bảng riêng complete một lần).
    {
        if first_ever_rating {
            let db_ret = state.db.clone();
            let ret_uid = user.id;
            tokio::spawn(async move {
                crate::services::retention::on_action(db_ret, ret_uid, "rate_game", 1).await;
            });
        }
        let db_ob = state.db.clone();
        let uid_ob = user.id;
        tokio::spawn(async move {
            crate::services::retention::onboarding_step(&db_ob, uid_ob, "first_rating").await;
        });
    }

    // Reload game to get updated rating
    let game = GameRepo::find_by_id(&state.db, game.id)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    let partial = RatingStarsPartial {
        game_id: game.id,
        slug: slug.clone(),
        user_rating: Some(form.score),
        rating_avg: game.rating_avg_f64(),
        rating_count: game.rating_count,
    };
    Ok(Html(partial.render()?))
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn toggle_follow(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(username): Path<String>,
) -> AppResult<Html<String>> {
    let target = crate::repositories::UserRepo::find_by_username(&state.db, &username)
        .await?
        .ok_or_else(|| AppError::NotFound("Người dùng không tồn tại".into()))?;
    // Chặn tự theo dõi tại handler: repo trả Ok(false) thầm lặng nên
    // trước đây user bấm nút "Theo dõi" trên trang của chính mình mãi
    // không có phản ứng gì (partial render lại "chưa theo dõi") —
    // vừa rác query vừa khó hiểu UX.
    if target.id == user.id {
        return Err(AppError::BadRequest(
            "Bạn không thể theo dõi chính mình".into(),
        ));
    }
    let is_following = InteractionRepo::toggle_follow(&state.db, user.id, target.id).await?;
    // v2.9.0 — XP cho người ĐƯỢC theo dõi (best-effort)
    if is_following {
        let db = state.db.clone();
        let followee = target.id;
        tokio::spawn(async move {
            crate::services::gamification::on_follow(&db, followee).await;
        });
    }
    let partial = FollowButtonPartial {
        target_user_id: target.id,
        target_username: username.clone(),
        is_following,
    };
    Ok(Html(partial.render()?))
}
