use crate::error::{AppError, AppResult};
use crate::middleware::AuthUser;
use crate::repositories::CommentRepo;
use crate::state::AppState;
use crate::templates::CommentItemPartial;
use askama::Template;
use axum::extract::{Path, State};
use axum::response::Html;
use axum::Form;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct CommentForm {
    pub content: String,
    pub parent_id: Option<String>,
}

pub async fn create_comment(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Form(form): Form<CommentForm>,
) -> AppResult<Html<String>> {
    let content = form.content.trim();
    if content.is_empty() {
        return Err(AppError::BadRequest("Nội dung không được để trống".into()));
    }
    if content.len() > 1000 {
        return Err(AppError::BadRequest("Nội dung quá dài (tối đa 1000 ký tự)".into()));
    }
    let game = crate::repositories::GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    let parent_id = form
        .parent_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| Uuid::parse_str(s).ok());
    let _id = CommentRepo::create(&state.db, game.id, user.id, parent_id, content).await?;

    // Return the new comment HTML for HTMX prepend
    let comment = crate::repositories::CommentRepo::find_by_id(&state.db, _id).await?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Failed to load created comment")))?;
    let partial = CommentItemPartial {
        comment: &comment,
        game_slug: &slug,
        current_user: Some(&user),
    };
    Ok(Html(partial.render()?))
}

pub async fn delete_comment(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Html<String>> {
    let comment = CommentRepo::find_by_id(&state.db, id).await?
        .ok_or_else(|| AppError::NotFound("Bình luận không tồn tại".into()))?;
    if comment.user_id != user.id && !user.role.is_staff() {
        return Err(AppError::Forbidden("Bạn không có quyền xóa bình luận này".into()));
    }
    CommentRepo::delete(&state.db, id).await?;
    Ok(Html("".into()))
}

pub async fn like_comment(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Html<String>> {
    let liked = CommentRepo::toggle_like(&state.db, id, user.id).await?;
    let comment = CommentRepo::find_by_id(&state.db, id).await?
        .ok_or_else(|| AppError::NotFound("Bình luận không tồn tại".into()))?;
    let _ = liked;
    Ok(Html(format!("{}", comment.like_count).into()))
}

pub async fn list_replies(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Html<String>> {
    let replies = CommentRepo::list_replies(&state.db, id, Some(user.id)).await?;
    let mut html = String::new();
    for r in &replies {
        let partial = CommentItemPartial {
            comment: r,
            game_slug: "", // not needed for replies list
            current_user: Some(&user),
        };
        html.push_str(&partial.render()?);
    }
    Ok(Html(html))
}
