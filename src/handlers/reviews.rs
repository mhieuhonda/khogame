//! v2.9.0 — Handlers review game (wire-up bảng `reviews` có sẵn từ 001).

use crate::error::{AppError, AppResult};
use crate::middleware::AuthUser;
use crate::models::game::GameStatus;
use crate::repositories::{GameRepo, ReviewRepo};
use crate::services::gamification as gsvc;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct ReviewForm {
    pub rating: i16,
    pub title: String,
    pub content: String,
}

/// POST /games/{slug}/reviews — tạo/cập nhật review (1 user = 1 review/game).
/// # Errors
/// Trả lỗi khi game không tồn tại, chưa publish, validation fail, DB fail.
pub async fn submit_review(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Form(form): Form<ReviewForm>,
) -> AppResult<Redirect> {
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    // Chỉ review game published (owner/staff được miễn để test)
    if game.user_id != user.id && !user.role.is_staff() && game.status != GameStatus::Published {
        return Err(AppError::NotFound("Game không tồn tại".into()));
    }
    // Validation
    if !(1..=5).contains(&form.rating) {
        return Err(AppError::BadRequest("Số sao phải từ 1 đến 5".into()));
    }
    let title = form.title.trim();
    if title.is_empty() {
        return Err(AppError::BadRequest(
            "Tiêu đề review không được để trống".into(),
        ));
    }
    // v3.9.0 — chars().count() thay vì len() (byte) — nhất quán với toàn
    // bộ codebase, tiếng Việt không bị siết oan.
    if title.chars().count() > 200 {
        return Err(AppError::BadRequest(
            "Tiêu đề review tối đa 200 ký tự".into(),
        ));
    }
    if form.content.chars().count() > 4000 {
        return Err(AppError::BadRequest(
            "Nội dung review tối đa 4000 ký tự".into(),
        ));
    }
    // v3.0.0 FIX (XP farm): chỉ cộng XP khi review MỚI được tạo —
    // edit/re-rate review cũ không cộng lại và không notify owner.
    let (_review_id, was_insert) = ReviewRepo::create_or_update(
        &state.db,
        game.id,
        user.id,
        title,
        form.content.trim(),
        form.rating,
    )
    .await?;
    // XP + huy hiệu (best-effort, fire-and-forget) — chỉ khi INSERT
    if was_insert {
        let owner_id = game.user_id;
        let db = state.db.clone();
        let reviewer_id = user.id;
        tokio::spawn(async move {
            gsvc::on_review(&db, reviewer_id, owner_id).await;
        });
        // v3.0.0 — quest review + heatmap
        let db_ret = state.db.clone();
        let ret_uid = reviewer_id;
        tokio::spawn(async move {
            crate::services::retention::on_action(db_ret, ret_uid, "review", 1).await;
        });
    }
    Ok(Redirect::to(&format!("/games/{slug}#reviews")))
}

/// POST /reviews/{id}/helpful — toggle vote "hữu ích".
/// # Errors
/// Trả lỗi khi review không tồn tại hoặc DB fail.
pub async fn toggle_helpful(
    State(state): State<Arc<AppState>>,
    AuthUser(_user): AuthUser,
    Path(id): Path<uuid::Uuid>,
) -> AppResult<Redirect> {
    // Cần biết game slug để redirect về — review join game
    let slug: Option<String> = sqlx::query_scalar(
        "SELECT g.slug FROM reviews r JOIN games g ON g.id = r.game_id WHERE r.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let Some(slug) = slug else {
        return Err(AppError::NotFound("Review không tồn tại".into()));
    };
    ReviewRepo::toggle_helpful(&state.db, id, _user.id).await?;
    Ok(Redirect::to(&format!("/games/{slug}#review-{id}")))
}

/// POST /reviews/{id}/delete — xóa review của chính mình.
/// # Errors
/// Trả lỗi khi review không tồn tại, không phải của mình, hoặc DB fail.
pub async fn delete_review(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<uuid::Uuid>,
) -> AppResult<Redirect> {
    let slug: Option<String> = sqlx::query_scalar(
        "SELECT g.slug FROM reviews r JOIN games g ON g.id = r.game_id WHERE r.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let Some(slug) = slug else {
        return Err(AppError::NotFound("Review không tồn tại".into()));
    };
    // Admin được xóa review của người khác (kiểm duyệt)
    if !user.role.is_staff() {
        let owner: Option<uuid::Uuid> =
            sqlx::query_scalar("SELECT user_id FROM reviews WHERE id = $1")
                .bind(id)
                .fetch_optional(&state.db)
                .await?;
        match owner {
            Some(o) if o == user.id => {}
            _ => return Err(AppError::Forbidden("Không phải review của bạn".into())),
        }
    }
    // v3.0.0 FIX: truyền is_staff vào repo + 404 khi không xóa được dòng
    // nào (trước đây staff xóa review người khác là no-op báo thành công).
    let deleted = ReviewRepo::delete(&state.db, id, user.id, user.role.is_staff()).await?;
    if !deleted {
        return Err(AppError::NotFound("Review không tồn tại".into()));
    }
    Ok(Redirect::to(&format!("/games/{slug}#reviews")))
}
