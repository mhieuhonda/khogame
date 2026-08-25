use crate::error::{AppError, AppResult};
use crate::handlers::auth::unread_count;
use crate::middleware::{AuthUser, CurrentUser};
use crate::models::repo::RepoForm;
use crate::repositories::{RepoRepo, SettingsRepo};
use crate::state::AppState;
use crate::templates::{RepoListTemplate, RepoNewTemplate};
use axum::extract::{Path, Query, State};
use axum::response::{Html, Redirect};
use axum::Form;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

// ============= Danh sách repo =============
#[derive(Deserialize, Default)]
pub struct RepoListQuery {
    pub sort: Option<String>,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(q): Query<RepoListQuery>,
) -> AppResult<RepoListTemplate> {
    let sort = q.sort.unwrap_or_else(|| "stars".into());
    // 2 query độc lập — join! song song.
    let (repos_res, total) = tokio::join!(
        RepoRepo::list_approved(&state.db, 60, 0, &sort),
        RepoRepo::count_approved(&state.db),
    );
    let repos = repos_res?;
    let total = total.unwrap_or(0);
    let unread = match current_user.as_ref() {
        Some(u) => unread_count(&state, u.id).await,
        None => 0,
    };
    Ok(RepoListTemplate {
        current_user,
        unread_notifications: unread,
        repos,
        total,
        sort,
    })
}

// ============= Form đăng repo =============
pub async fn new_form(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<RepoNewTemplate> {
    let unread = unread_count(&state, user.id).await;
    Ok(RepoNewTemplate {
        current_user: Some(user),
        unread_notifications: unread,
    })
}

// ============= Gọi GitHub API lấy metadata =============
async fn fetch_github_meta(
    state: &AppState,
    owner: &str,
    repo: &str,
) -> AppResult<crate::models::repo::GithubApiRepo> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}");
    let mut req = state
        .http_client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(token) = &state.config.github_token {
        req = req.bearer_auth(token);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| AppError::OAuth(format!("Không kết nối được GitHub API: {e}")))?;
    match resp.status().as_u16() {
        200 => Ok(resp.json().await?),
        404 => Err(AppError::NotFound(format!(
            "Repo {owner}/{repo} không tồn tại hoặc ở chế độ riêng tư"
        ))),
        403 => Err(AppError::OAuth(
            "GitHub API giới hạn tốc độ. Thử lại sau ít phút hoặc cấu hình GITHUB_TOKEN.".into(),
        )),
        code => Err(AppError::OAuth(format!("GitHub API lỗi HTTP {code}"))),
    }
}

// ============= Tạo repo =============
pub async fn create(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Form(form): Form<RepoForm>,
) -> AppResult<Redirect> {
    // Validate URL: chỉ chấp nhận github.com hoặc định dạng owner/repo.
    // parse_github_url đã kiểm tra format, nhưng cũng chặn sớm URL rỗng.
    let url_trimmed = form.url.trim();
    if url_trimmed.is_empty() {
        return Err(AppError::BadRequest("URL repo không được để trống".into()));
    }
    if url_trimmed.len() > 2048 {
        return Err(AppError::BadRequest(
            "URL repo quá dài (tối đa 2048 ký tự)".into(),
        ));
    }
    let (owner, name) = RepoRepo::parse_github_url(url_trimmed).ok_or_else(|| {
        AppError::BadRequest(
            "URL repo không hợp lệ. Dùng dạng https://github.com/owner/repo hoặc owner/repo".into(),
        )
    })?;
    // Chống chiếm quyền sở hữu repo entry: ON CONFLICT (owner, repo_name)
    // DO UPDATE SET user_id = ... khiến user B đăng lại repo user A đã
    // đăng sẽ LẤT user_id của A. Chặn trước: repo đã có thuộc user khác
    // (không phải staff) → 409 Conflict.
    if let Some(existing) = RepoRepo::find_by_owner_name(&state.db, &owner, &name).await? {
        if existing.user_id != user.id && !user.role.is_staff() {
            tracing::warn!(
                "Repo hijack blocked: {} cố đăng ký {}/{} của user {}",
                user.username,
                owner,
                name,
                existing.user_id
            );
            return Err(AppError::Conflict(
                "Repo này đã được người dùng khác đăng ký. Nếu đây là repo của bạn, hãy liên hệ quản trị viên."
                    .into(),
            ));
        }
    }
    // Validate description length nếu user cung cấp
    let description_user = form.description.trim();
    if description_user.chars().count() > 500 {
        return Err(AppError::BadRequest("Mô tả repo tối đa 500 ký tự".into()));
    }

    // Lấy metadata từ GitHub
    let meta = fetch_github_meta(&state, &owner, &name).await?;

    // Tự duyệt nếu người dùng là staff, ngược lại chờ duyệt khi cấu hình yêu cầu
    let auto_approve = user.role.is_staff()
        || SettingsRepo::get(&state.db, "repo_auto_approve")
            .await?
            .map_or(true, |v| v == "on");

    // Liên kết game (nếu có) — slug sai báo lỗi rõ ràng thay vì lặng lẽ
    // bỏ liên kết (trước đây user chọn game trong dropdown nhưng slug
    // đã bị đổi/xóa giữa chừng → repo đăng thành công mà KHÔNG liên kết
    // game nào, user tưởng đã liên kết).
    let game_id = match form.game_slug.as_deref().filter(|s| !s.is_empty()) {
        Some(slug) => Some(
            crate::repositories::GameRepo::find_by_slug(&state.db, slug)
                .await?
                .ok_or_else(|| {
                    AppError::BadRequest(format!(
                        "Game '{slug}' không tồn tại (có thể đã bị đổi tên hoặc xóa). Vui lòng chọn lại."
                    ))
                })?
                .id,
        ),
        None => None,
    };

    let description = if description_user.is_empty() {
        meta.description.clone().unwrap_or_default()
    } else {
        description_user.to_string()
    };

    let repo_id = RepoRepo::create(
        &state.db,
        user.id,
        game_id,
        &owner,
        &name,
        &description,
        meta.homepage.as_deref().unwrap_or(""),
        meta.language.as_deref().unwrap_or(""),
        meta.stargazers_count.unwrap_or(0),
        meta.forks_count.unwrap_or(0),
        meta.open_issues_count.unwrap_or(0),
        meta.pushed_at,
    )
    .await?;

    // Nếu có auto_approve = false -> chuyển sang pending ngay với id vừa insert.
    if !auto_approve {
        let _ = RepoRepo::set_status(&state.db, repo_id, "pending").await;
    }

    tracing::info!(
        "Repo registered: {}/{} by {} (id={})",
        owner,
        name,
        user.username,
        repo_id
    );
    Ok(Redirect::to("/repos"))
}

// ============= Làm mới metadata 1 repo =============
pub async fn refresh(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Html<String>> {
    let repo = RepoRepo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Repo không tồn tại".into()))?;
    if repo.user_id != user.id && !user.role.is_staff() {
        return Err(AppError::Forbidden("Không có quyền".into()));
    }
    let meta = fetch_github_meta(&state, &repo.owner, &repo.repo_name).await?;
    RepoRepo::update_meta(
        &state.db,
        id,
        meta.description.as_deref().unwrap_or(""),
        meta.homepage.as_deref().unwrap_or(""),
        meta.language.as_deref().unwrap_or(""),
        meta.stargazers_count.unwrap_or(0),
        meta.forks_count.unwrap_or(0),
        meta.open_issues_count.unwrap_or(0),
        meta.pushed_at,
    )
    .await?;
    Ok(Html(
        "<div class='alert alert-success'>Đã làm mới dữ liệu từ GitHub.</div>".into(),
    ))
}

// ============= Xóa repo của mình =============
pub async fn delete_own(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Redirect> {
    let repo = RepoRepo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Repo không tồn tại".into()))?;
    if repo.user_id != user.id && !user.role.is_staff() {
        return Err(AppError::Forbidden("Không có quyền".into()));
    }
    RepoRepo::delete(&state.db, id).await?;
    Ok(Redirect::to("/repos"))
}

// ============= Partial: danh sách repo của user (cho profile) =============
pub async fn user_repos_fragment(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
) -> AppResult<Html<String>> {
    let user = crate::repositories::UserRepo::find_by_username(&state.db, &username)
        .await?
        .ok_or_else(|| AppError::NotFound("Người dùng không tồn tại".into()))?;
    // Ẩn repo của user bị ban (thống nhất với trang /u/{username})
    if user.is_banned {
        return Err(AppError::NotFound("Người dùng không tồn tại".into()));
    }
    let repos = RepoRepo::list_by_user(&state.db, user.id).await?;
    if repos.is_empty() {
        return Ok(Html(
            r#"<div class="empty-state" style="padding:24px"><div style="font-size:40px">📦</div><h3>Chưa có repo GitHub nào</h3><p><a href="/repos/new">Đăng ký repo</a> để hiển thị tại đây.</p></div>"#
                .to_string(),
        ));
    }
    let items: Vec<String> = repos
        .iter()
        .map(|r| {
            // Escape HTML mọi giá trị động chèn vào markup thủ công —
            // defense-in-depth: owner/repo_name do GitHub API cung cấp
            // (đã bị GitHub validate) nhưng nếu nguồn dữ liệu đổi/bug
            // parser thì chuỗi lạ vẫn không thành thẻ HTML sống được.
            format!(
                r#"<a href="{}" class="repo-mini-card" target="_blank" rel="noopener">
                    <span class="repo-name">{}</span>
                    <span class="repo-stars">⭐ {} · 🍴 {}</span>
                </a>"#,
                crate::utils::html_escape(&r.html_url()),
                crate::utils::html_escape(&r.full_name()),
                crate::utils::format_number(r.stars),
                crate::utils::format_number(r.forks),
            )
        })
        .collect();
    Ok(Html(format!(
        r#"<div class="repo-mini-grid">{}</div>"#,
        items.join("\n")
    )))
}
