//! v2.9.0 — Handlers bộ sưu tập game (collections).

use crate::error::{AppError, AppResult};
use crate::middleware::AuthUser;
use crate::repositories::{CollectionRepo, GameRepo};
use crate::state::AppState;
use crate::templates::{CollectionShowTemplate, MyCollectionsTemplate};
use axum::extract::{Path, State};
use axum::response::Redirect;
use axum::Form;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct CollectionForm {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub is_public: Option<String>,
}

#[derive(Deserialize)]
pub struct CollectionGameForm {
    // v3.6.0 FIX (400 trần): trước đây `collection_id: uuid::Uuid` — select
    // có option rỗng "— Chọn bộ sưu tập —", submit khi chưa chọn (JS off,
    // autofill, submit bằng Enter) → serde parse UUID từ chuỗi rỗng fail →
    // rejection 400 text/plain trơn. Giờ nhận String + parse thủ công để
    // trả BadRequest có message tiếng Việt thân thiện.
    #[serde(default)]
    pub collection_id: Option<String>,
}

/// Parse collection_id từ form → UUID, kèm thông báo lỗi thân thiện.
fn parse_collection_id(form: &CollectionGameForm) -> AppResult<uuid::Uuid> {
    let raw = form
        .collection_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    if raw.is_empty() {
        return Err(AppError::BadRequest(
            "Bạn chưa chọn bộ sưu tập — hãy chọn 1 bộ sưu tập từ danh sách.".into(),
        ));
    }
    raw.parse::<uuid::Uuid>().map_err(|_| {
        AppError::BadRequest("Bộ sưu tập được chọn không hợp lệ — thử tải lại trang.".into())
    })
}

/// GET /collections — danh sách bộ sưu tập của tôi.
/// # Errors
/// Trả lỗi khi chưa đăng nhập hoặc DB fail.
pub async fn my_collections(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<MyCollectionsTemplate> {
    let collections = CollectionRepo::list_for_user(&state.db, user.id, true).await?;
    let unread = crate::handlers::auth::unread_count(&state, user.id).await;
    Ok(MyCollectionsTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        collections,
    })
}

/// POST /collections — tạo bộ sưu tập mới.
/// # Errors
/// Trả lỗi khi validation fail, quá 20 bộ sưu tập, hoặc DB fail.
pub async fn create(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Form(form): Form<CollectionForm>,
) -> AppResult<Redirect> {
    let title = form.title.trim();
    if title.is_empty() {
        return Err(AppError::BadRequest(
            "Tên bộ sưu tập không được trống".into(),
        ));
    }
    if title.len() > 100 {
        return Err(AppError::BadRequest("Tên tối đa 100 ký tự".into()));
    }
    if form.description.len() > 300 {
        return Err(AppError::BadRequest("Mô tả tối đa 300 ký tự".into()));
    }
    // v3.0.0 — LEVEL PERK: giới hạn số bộ sưu tập tăng theo cấp độ.
    // Cơ sở 5; +2 từ Lv.3, +5 từ Lv.7, +10 từ Lv.10 → max 15.
    let (count, level) = {
        let (c, l) = tokio::join!(
            CollectionRepo::count_for_user(&state.db, user.id),
            crate::repositories::GamificationRepo::level_of(&state.db, user.id),
        );
        (
            c.unwrap_or(0),
            l.unwrap_or_else(|_| crate::models::gamification::level_from_xp(0)),
        )
    };
    let max_collections = collection_limit_for_level(level.level);
    if count >= i64::from(max_collections) {
        return Err(AppError::BadRequest(format!(
            "Bạn đã đạt giới hạn {} bộ sưu tập ở Cấp {} — lên cấp cao hơn để mở thêm (hoặc xoá bớt)!",
            max_collections, level.level
        )));
    }
    CollectionRepo::create(
        &state.db,
        user.id,
        title,
        form.description.trim(),
        form.is_public.as_deref() == Some("1"),
    )
    .await?;
    Ok(Redirect::to("/collections"))
}

/// v3.0.0 — Giới hạn bộ sưu tập theo cấp độ (hàm thuần — test được).
/// Lv.1-2: 5 · Lv.3-6: 7 · Lv.7-9: 12 · Lv.10+: 20.
/// v3.1.0 — level: i64 (BIGINT — hỗ trợ 500 tỷ).
#[must_use]
pub fn collection_limit_for_level(level: i64) -> i32 {
    match level {
        0..=2 => 5,
        3..=6 => 7,
        7..=9 => 12,
        _ => 20,
    }
}

/// GET /collections/{id} — xem bộ sưu tập (public hoặc chủ sở hữu).
/// # Errors
/// Trả lỗi khi không tồn tại, là private của người khác, hoặc DB fail.
pub async fn show(
    State(state): State<Arc<AppState>>,
    crate::middleware::CurrentUser(current_user): crate::middleware::CurrentUser,
    Path(id): Path<uuid::Uuid>,
) -> AppResult<CollectionShowTemplate> {
    let collection = CollectionRepo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Bộ sưu tập không tồn tại".into()))?;
    let is_owner = current_user
        .as_ref()
        .is_some_and(|u| u.id == collection.user_id);
    if !collection.is_public
        && !is_owner
        && !current_user.as_ref().is_some_and(|u| u.role.is_staff())
    {
        return Err(AppError::NotFound("Bộ sưu tập không tồn tại".into()));
    }
    let (games, owner) = tokio::join!(
        CollectionRepo::games(&state.db, id, 48, 0),
        crate::repositories::UserRepo::find_by_id(&state.db, collection.user_id),
    );
    let games = games?;
    let owner = owner?.ok_or_else(|| AppError::NotFound("Chủ sở hữu không tồn tại".into()))?;
    let unread = match current_user.as_ref() {
        Some(u) => crate::handlers::auth::unread_count(&state, u.id).await,
        None => 0,
    };
    Ok(CollectionShowTemplate {
        current_user,
        unread_notifications: unread,
        collection,
        games,
        owner_name: owner.display_name,
        owner_username: owner.username,
        owner_avatar: owner.avatar_url,
    })
}

/// POST /collections/{id}/delete — xóa bộ sưu tập của mình.
/// # Errors
/// Trả lỗi khi không tồn tại, không phải của mình, hoặc DB fail.
pub async fn delete(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<uuid::Uuid>,
) -> AppResult<Redirect> {
    CollectionRepo::delete(&state.db, id, user.id).await?;
    Ok(Redirect::to("/collections"))
}

/// POST /games/{slug}/add-to-collection — thêm game vào bộ sưu tập.
/// # Errors
/// Trả lỗi khi collection không phải của mình / game chưa publish / DB fail.
pub async fn add_game(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Form(form): Form<CollectionGameForm>,
) -> AppResult<Redirect> {
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    let collection_id = parse_collection_id(&form)?;
    // Collection phải thuộc quyền user
    let collection = CollectionRepo::find_by_id(&state.db, collection_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Bộ sưu tập không tồn tại".into()))?;
    if collection.user_id != user.id {
        return Err(AppError::Forbidden(
            "Bạn không có quyền sửa bộ sưu tập này".into(),
        ));
    }
    let added = CollectionRepo::add_game(&state.db, collection.id, game.id).await?;
    // v3.0.0 — quest add_collection (chỉ khi add THÀNH CÔNG — đã có trong
    // collection thì add_game trả false, không bump ảo)
    if added {
        let db_ret = state.db.clone();
        let ret_uid = user.id;
        tokio::spawn(async move {
            crate::services::retention::on_action(db_ret, ret_uid, "add_collection", 1).await;
        });
    }
    Ok(Redirect::to(&format!("/games/{slug}")))
}

/// POST /games/{slug}/remove-from-collection — xóa game khỏi bộ sưu tập.
/// # Errors
/// Trả lỗi khi collection không phải của mình hoặc DB fail.
pub async fn remove_game(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Form(form): Form<CollectionGameForm>,
) -> AppResult<Redirect> {
    let collection_id = parse_collection_id(&form)?;
    let collection = CollectionRepo::find_by_id(&state.db, collection_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Bộ sưu tập không tồn tại".into()))?;
    if collection.user_id != user.id {
        return Err(AppError::Forbidden(
            "Bạn không có quyền sửa bộ sưu tập này".into(),
        ));
    }
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    CollectionRepo::remove_game(&state.db, collection.id, game.id).await?;
    Ok(Redirect::to(&format!("/games/{slug}")))
}

#[cfg(test)]
mod tests {
    use super::collection_limit_for_level;

    #[test]
    fn test_collection_limit_by_level() {
        assert_eq!(collection_limit_for_level(1), 5);
        assert_eq!(collection_limit_for_level(2), 5);
        assert_eq!(collection_limit_for_level(3), 7);
        assert_eq!(collection_limit_for_level(6), 7);
        assert_eq!(collection_limit_for_level(7), 12);
        assert_eq!(collection_limit_for_level(9), 12);
        assert_eq!(collection_limit_for_level(10), 20);
        assert_eq!(collection_limit_for_level(99), 20);
        // v3.1.0 — i64 case (huge level).
        assert_eq!(collection_limit_for_level(1_000_000), 20);
        assert_eq!(collection_limit_for_level(i64::MAX), 20);
    }
}
