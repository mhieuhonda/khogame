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

fn iso8601_utc(dt: chrono::DateTime<chrono::Utc>) -> String {
    dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

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
    let repos = RepoRepo::list_approved(&state.db, 60, 0, &sort).await?;
    let total = RepoRepo::count_approved(&state.db).await.unwrap_or(0);
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
    let url = format!("https://api.github.com/repos/{}/{}", owner, repo);
    let mut req = state
        .http_client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(token) = &state.config.github_token {
        req = req.bearer_auth(token);
    }
    let resp = req.send().await.map_err(|e| {
        AppError::OAuth(format!("Không kết nối được GitHub API: {}", e))
    })?;
    match resp.status().as_u16() {
        200 => Ok(resp.json().await?),
        404 => Err(AppError::NotFound(format!(
            "Repo {}/{} không tồn tại hoặc ở chế độ riêng tư",
            owner, repo
        ))),
        403 => Err(AppError::OAuth(
            "GitHub API giới hạn tốc độ. Thử lại sau ít phút hoặc cấu hình GITHUB_TOKEN.".into(),
        )),
        code => Err(AppError::OAuth(format!("GitHub API lỗi HTTP {}", code))),
    }
}

// ============= Tạo repo =============
pub async fn create(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Form(form): Form<RepoForm>,
) -> AppResult<Redirect> {
    let (owner, name) = RepoRepo::parse_github_url(&form.url)
        .ok_or_else(|| AppError::BadRequest("URL repo không hợp lệ. Dùng dạng https://github.com/owner/repo hoặc owner/repo".into()))?;

    // Lấy metadata từ GitHub
    let meta = fetch_github_meta(&state, &owner, &name).await?;

    // Tự duyệt nếu người dùng là staff, ngược lại chờ duyệt khi cấu hình yêu cầu
    let auto_approve = user.role.is_staff()
        || SettingsRepo::get(&state.db, "repo_auto_approve")
            .await?
            .map(|v| v == "on")
            .unwrap_or(true);

    // Liên kết game (nếu có)
    let game_id = match form.game_slug.as_deref().filter(|s| !s.is_empty()) {
        Some(slug) => crate::repositories::GameRepo::find_by_slug(&state.db, slug)
            .await?
            .map(|g| g.id),
        None => None,
    };

    let description = if form.description.trim().is_empty() {
        meta.description.clone().unwrap_or_default()
    } else {
        form.description.trim().to_string()
    };

    let _id = RepoRepo::create(
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

    // Nếu có auto_approve = false -> chuyển sang pending sau khi insert
    if !auto_approve {
        if let Ok(Some(id)) = RepoRepo::list_by_user(&state.db, user.id)
            .await
            .map(|rs| rs.first().map(|r| r.id))
        {
            let _ = RepoRepo::set_status(&state.db, id, "pending").await;
        }
    }

    tracing::info!("Repo registered: {}/{} by {}", owner, name, user.username);
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
    let repos = RepoRepo::list_by_user(&state.db, user.id).await?;
    let items: Vec<String> = repos
        .iter()
        .map(|r| {
            format!(
                r#"<a href="{}" class="repo-mini-card" target="_blank" rel="noopener">
                    <span class="repo-name">{}</span>
                    <span class="repo-stars">⭐ {} · 🍴 {}</span>
                </a>"#,
                r.html_url(),
                r.full_name(),
                crate::utils::format_number(r.stars),
                crate::utils::format_number(r.forks),
            )
        })
        .collect();
    Ok(Html(items.join("\n")))
}

#[allow(dead_code)]
fn unused_iso(dt: &Option<chrono::DateTime<chrono::Utc>>) -> String {
    dt.map(iso8601_utc).unwrap_or_default()
}
