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
    pub collection_id: uuid::Uuid,
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
    // Collection phải thuộc quyền user
    let collection = CollectionRepo::find_by_id(&state.db, form.collection_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Bộ sưu tập không tồn tại".into()))?;
    if collection.user_id != user.id {
        return Err(AppError::Forbidden(
            "Bạn không có quyền sửa bộ sưu tập này".into(),
        ));
    }
    CollectionRepo::add_game(&state.db, collection.id, game.id).await?;
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
    let collection = CollectionRepo::find_by_id(&state.db, form.collection_id)
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
