use crate::error::{AppError, AppResult};
use crate::middleware::{AuthUser, CurrentUser};
use crate::repositories::CommentRepo;
use crate::state::AppState;
use crate::templates::CommentItemPartial;
use askama::Template;
use axum::extract::{Path, Query, State};
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

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
    // Đếm theo số ký tự (char count), không phải byte length, để hỗ trợ unicode tiếng Việt.
    // 1000 ký tự Việt = ~3000 bytes UTF-8, nếu đếm byte sẽ chặn nhầm.
    let char_count = content.chars().count();
    if char_count > 1000 {
        return Err(AppError::BadRequest(format!(
            "Nội dung quá dài (tối đa 1000 ký tự, hiện có {char_count})"
        )));
    }
    let game = crate::repositories::GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    // Chỉ cho bình luận trên game đã xuất bản: tránh việc comment vào
    // game draft/hidden mà người dùng thường không được xem (lỗ hổng
    // ủy quyền — trước đây POST vẫn hoạt động dù trang show đã chặn).
    let is_owner = game.user_id == user.id;
    if !is_owner
        && !user.role.is_staff()
        && game.status != crate::models::game::GameStatus::Published
    {
        return Err(AppError::NotFound("Game không tồn tại".into()));
    }
    let mut parent_id = form
        .parent_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| Uuid::parse_str(s).ok());
    // Chuẩn hoá depth bình luận về tối đa 2 cấp: trả lời một reply
    // sẽ được gắn vào comment gốc để luôn hiển thị đúng vị trí.
    if let Some(pid) = parent_id {
        let parent = CommentRepo::find_by_id(&state.db, pid)
            .await?
            .ok_or_else(|| AppError::BadRequest("Bình luận cha không tồn tại".into()))?;
        // Verify parent belongs to same game — chống IDOR qua parent_id
        // chỉ comment của game khác (sẽ tạo bình luận mồ côi không hiển thị
        // ở đâu, làm rác DB + có thể dẫn đến leak metadata của game khác).
        if parent.game_id != game.id {
            return Err(AppError::BadRequest(
                "Bình luận cha không thuộc game này".into(),
            ));
        }
        if let Some(grand) = parent.parent_id {
            parent_id = Some(grand);
        }
    }
    let _id = CommentRepo::create(&state.db, game.id, user.id, parent_id, content).await?;

    // Mention @username -> thông báo (v2.2.0: batch INSERT thay vì N+1 loop)
    let mentions = CommentRepo::find_mentions(&state.db, content, user.id)
        .await
        .unwrap_or_default();
    if !mentions.is_empty() {
        let _ = crate::repositories::NotificationRepo::create_mentions_batch(
            &state.db, &mentions, user.id, &slug,
        )
        .await;
    }

    // Return the new comment HTML for HTMX prepend
    let comment = crate::repositories::CommentRepo::find_by_id(&state.db, _id)
        .await?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Failed to load created comment")))?;
    let partial = CommentItemPartial {
        comment: &comment,
        game_slug: &slug,
        current_user: Some(&user),
        load_replies: true,
    };
    Ok(Html(partial.render()?))
}

/// Sửa bình luận của chính mình (trong 5 phút)
#[derive(Deserialize)]
pub struct EditCommentForm {
    pub content: String,
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn edit_comment(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Form(form): Form<EditCommentForm>,
) -> AppResult<Html<String>> {
    let content = form.content.trim();
    if content.is_empty() {
        return Err(AppError::BadRequest("Nội dung không được để trống".into()));
    }
    // Giới hạn 1000 ký tự phải áp dụng cả khi sửa (trước đây chỉ kiểm tra
    // lúc tạo — user có thể sửa comment thành chuỗi dài vô hạn, DB field
    // TEXT không có constraint).
    let char_count = content.chars().count();
    if char_count > 1000 {
        return Err(AppError::BadRequest(format!(
            "Nội dung quá dài (tối đa 1000 ký tự, hiện có {char_count})"
        )));
    }
    let existing = CommentRepo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Bình luận không tồn tại".into()))?;
    // Chỉ chủ sở hữu được sửa (repo cũng kiểm tra + giới hạn 5 phút)
    if existing.user_id != user.id {
        return Err(AppError::Forbidden(
            "Bạn chỉ có thể sửa bình luận của chính mình".into(),
        ));
    }
    let updated = CommentRepo::update_content(&state.db, id, user.id, content).await?;
    // Lấy đúng slug của game để form trả lời trong item vẫn hoạt động
    let game_slug = crate::repositories::GameRepo::find_by_id(&state.db, updated.game_id)
        .await?
        .map(|g| g.slug)
        .unwrap_or_default();
    let partial = CommentItemPartial {
        comment: &updated,
        game_slug: &game_slug,
        current_user: Some(&user),
        load_replies: true,
    };
    Ok(Html(partial.render()?))
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn delete_comment(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Html<String>> {
    let comment = CommentRepo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Bình luận không tồn tại".into()))?;
    if comment.user_id != user.id && !user.role.is_staff() {
        return Err(AppError::Forbidden(
            "Bạn không có quyền xóa bình luận này".into(),
        ));
    }
    CommentRepo::delete(&state.db, id).await?;
    Ok(Html(String::new()))
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn like_comment(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Html<String>> {
    let _liked = CommentRepo::toggle_like(&state.db, id, user.id).await?;
    // v2.2.0 — re-render toàn bộ comment item partial thay vì chỉ count.
    // Bug trước đây: chỉ trả like_count text → button HTMX swap outerHTML
    // nhưng aria-pressed / class "active" không update → UI không phản ánh
    // state like mới. Render lại full partial đảm bảo nút like, count, aria
    // đều đồng bộ.
    let comment = CommentRepo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Bình luận không tồn tại".into()))?;
    // Lấy game_slug để partial render nút reply/edit đúng endpoint
    let game_slug = crate::repositories::GameRepo::find_by_id(&state.db, comment.game_id)
        .await?
        .map(|g| g.slug)
        .unwrap_or_default();
    let partial = CommentItemPartial {
        comment: &comment,
        game_slug: &game_slug,
        current_user: Some(&user),
        load_replies: false,
    };
    Ok(Html(partial.render()?))
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn list_replies(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Path(id): Path<Uuid>,
) -> AppResult<Html<String>> {
    // Cho phép cả khách chưa đăng nhập xem replies (container tải lười
    // khi cuộn tới; trước đây yêu cầu đăng nhập → 401 với khách)
    let replies =
        CommentRepo::list_replies(&state.db, id, current_user.as_ref().map(|u| u.id)).await?;
    let mut html = String::new();
    for r in &replies {
        let partial = CommentItemPartial {
            comment: r,
            game_slug: "", // not needed for replies list
            current_user: current_user.as_ref(),
            load_replies: false,
        };
        html.push_str(&partial.render()?);
    }
    Ok(Html(html))
}

// ============= Load-more comments (GET, HTMX) =============
#[derive(Deserialize, Default)]
pub struct CommentsPageQuery {
    pub page: Option<i64>,
}

/// GET /games/{slug}/comments?page=N — trả về HTML các comment trang N
/// để nút "Tải thêm" chèn vào cuối danh sách. Trước đây trang game chỉ
/// load 50 comment đầu và KHÔNG có cách nào xem phần cũ.
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn list_comments_page(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Path(slug): Path<String>,
    Query(q): Query<CommentsPageQuery>,
) -> AppResult<Html<String>> {
    let game = crate::repositories::GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 50;
    let offset = (page - 1) * per_page;
    let comments = CommentRepo::list_by_game(
        &state.db,
        game.id,
        current_user.as_ref().map(|u| u.id),
        per_page,
        offset,
    )
    .await?;
    let loaded = offset + comments.len() as i64;
    let remaining = (i64::from(game.comment_count) - loaded).max(0);
    let tpl = crate::templates::CommentsPageTemplate {
        current_user,
        comments,
        game_slug: slug,
        page,
        has_more: remaining > 0,
        remaining,
    };
    Ok(Html(tpl.render()?))
}
