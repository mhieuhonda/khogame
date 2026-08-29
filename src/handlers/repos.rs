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

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn list(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(q): Query<RepoListQuery>,
) -> AppResult<RepoListTemplate> {
    let sort = q.sort.unwrap_or_else(|| "stars".into());
    // v2.6.0 — 3 queries (repos/total/unread) chạy SONG SONG — trước đây
    // 2 song song rồi unread await tuần tự sau đó. Tránh cộng dồn
    // latency TTFB cho trang danh sách repo.
    let user_id = current_user.as_ref().map(|u| u.id);
    let (repos_res, total_res, unread_res) = tokio::join!(
        RepoRepo::list_approved(&state.db, 60, 0, &sort),
        RepoRepo::count_approved(&state.db),
        async {
            match user_id {
                Some(uid) => unread_count(&state, uid).await,
                None => 0,
            }
        },
    );
    let repos = repos_res?;
    let total = total_res.unwrap_or(0);
    let unread = unread_res;
    Ok(RepoListTemplate {
        current_user,
        unread_notifications: unread,
        repos,
        total,
        sort,
    })
}

// ============= Form đăng repo =============
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
/// Map [`GithubApiError`] của service GitHub → AppError có thông điệp RÕ
/// RÀNG cho user. Hàm độc lập để unit test được (regression v2.8.0/v2.9.1).
///
/// v2.8.0 — FIX "đăng repo liên tục 500": trước đây mọi status ngoài
/// 200/404/403/401 (điển hình: 429 rate-limit mới của GitHub, 5xx của
/// chính GitHub) rơi vào nhánh `AppError::OAuth` → 500 "Oops! Lỗi hệ
/// thống" vô nghĩa cho user. Giờ: mọi lỗi GitHub API đều trả 4xx với
/// message hành động được; lỗi thật sự phía server (401 token sai) mới
/// giữ 500 + log ERROR cho admin.
fn map_github_api_error(
    owner: &str,
    repo: &str,
    e: crate::services::github::GithubApiError,
) -> AppError {
    match e.status {
        None => {
            tracing::warn!("GitHub API không kết nối được ({owner}/{repo}): {e}");
            AppError::BadRequest(
                "Máy chủ tạm thời không kết nối được GitHub API. Vui lòng thử lại sau ít phút."
                    .into(),
            )
        }
        Some(404) => AppError::NotFound(
            "Repo không tồn tại hoặc ở chế độ riêng tư. Kiểm tra lại URL (repo phải là public)."
                .into(),
        ),
        // Rate limit: 403 (primary, theo IP) và 429 (primary/secondary,
        // kèm Retry-After). Máy chủ KHÔNG cấu hình GITHUB_TOKEN sẽ dùng
        // chung quota 60 req/giờ theo IP datacenter — dễ cạn vì các app
        // khác cùng NAT. Khi exhausted, cả 403 lẫn 429 đều về đây.
        Some(s @ (403 | 429)) => {
            let hint = match e.retry_after {
                Some(sec) if sec > 0 && sec < 3600 => {
                    format!(" Vui lòng thử lại sau khoảng {} phút.", (sec / 60).max(1))
                }
                _ => " Vui lòng thử lại sau ít phút.".to_string(),
            };
            tracing::warn!(
                "GitHub API rate limit (HTTP {s}) retry_after={:?}",
                e.retry_after
            );
            AppError::BadRequest(format!(
                "GitHub API đang giới hạn số lượt truy vấn của máy chủ.{hint}"
            ))
        }
        // Token cấu hình sai/hết hạn — lỗi phía server (admin phải sửa
        // GITHUB_TOKEN), giữ 500 nhưng log rõ cho admin.
        Some(401) => {
            tracing::error!("GitHub API 401 — GITHUB_TOKEN cấu hình không hợp lệ hoặc đã hết hạn");
            AppError::OAuth(
                "Máy chủ cấu hình GitHub token không hợp lệ. Vui lòng báo quản trị viên.".into(),
            )
        }
        // 200 nhưng JSON sai định dạng (GitHub đổi schema?) — log raw + 400
        // rõ ràng thay vì 500 mù (trước đây `?` trên resp.json() → Http → 500).
        Some(200) => {
            tracing::warn!("GitHub API trả JSON không deserialize được: {e}");
            AppError::BadRequest(
                "GitHub API trả dữ liệu không đúng định dạng. Vui lòng thử lại sau ít phút.".into(),
            )
        }
        // Các status khác (451 legal, 5xx GitHub...) — sự cố tạm thời
        // phía GitHub, KHÔNG phải lỗi hệ thống của ta → 400 + message rõ.
        Some(code) => {
            tracing::warn!("GitHub API trả HTTP {code} bất thường");
            AppError::BadRequest(format!(
                "GitHub API tạm thời gặp sự cố (HTTP {code}). Vui lòng thử lại sau ít phút."
            ))
        }
    }
}

/// Gọi GitHub API lấy metadata — ủy thác cho `services::github` (dùng chung
/// với job nền refresh số sao v2.9.1) rồi map lỗi sang AppError thân thiện.
async fn fetch_github_meta(
    state: &AppState,
    owner: &str,
    repo: &str,
) -> AppResult<crate::models::repo::GithubApiRepo> {
    crate::services::github::fetch_repo_meta(
        &state.http_client,
        state.config.github_token.as_ref(),
        owner,
        repo,
    )
    .await
    .map_err(|e| map_github_api_error(owner, repo, e))
}

// ============= Tạo repo =============
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
    // Chống chiếm quyền sở hữu repo entry: repo đã có thuộc user khác
    // (không phải staff) → 409 Conflict.
    // v2.8.0 — Đăng LẠI repo CỦA CHÍNH MÌNH không còn báo lỗi vô nghĩa
    // (trước đây: INSERT → ON CONFLICT DO NOTHING → rows_affected=0 →
    // 409 "Repo đã tồn tại (có thể vừa được người khác đăng ký cùng lúc)")
    // mà sẽ CẬP NHẬT metadata mới nhất từ GitHub + game link/ảnh mới.
    let mut repost_id: Option<Uuid> = None;
    if let Some(existing) = RepoRepo::find_by_owner_name(&state.db, &owner, &name).await? {
        if existing.user_id == user.id {
            repost_id = Some(existing.id);
        } else if !user.role.is_staff() {
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
        } else {
            // Staff đăng repo của user khác → giữ nguyên sở hữu, chỉ nhắc
            // quản lý qua trang admin thay vì ném 409 khó hiểu.
            return Err(AppError::Conflict(format!(
                "Repo {owner}/{name} đã được đăng ký trong hệ thống. Quản lý nó trong trang Quản trị → Repos."
            )));
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
            .is_none_or(|v| v == "on");

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

    // Lưu ảnh thumbnail custom nếu user upload/điền URL.
    // Validate: chỉ chấp nhận http(s):// hoặc `/uploads/repos/...` URL nội bộ.
    let image_url = form.repo_image_url.trim();
    let safe_image_url: Option<&str> = if !image_url.is_empty() {
        if !crate::utils::is_safe_image_url(image_url) {
            return Err(AppError::BadRequest(
                "Ảnh thumbnail URL phải là http(s):// hoặc /uploads/repos/...".into(),
            ));
        }
        if image_url.len() > 2048 {
            return Err(AppError::BadRequest(
                "Ảnh thumbnail URL quá dài (tối đa 2048 ký tự)".into(),
            ));
        }
        Some(image_url)
    } else {
        None
    };

    // v2.2.0 — Atomic create: 1 INSERT với tất cả fields (image_url + status)
    // Trước đây là 3 round-trip DB (create + set_image_url + set_status),
    // nếu 1 trong 3 fail thì repo tồn tại trong trạng thái inconsistent.
    let final_status = if auto_approve { "approved" } else { "pending" };

    // v2.8.0 — Re-post repo của chính mình: CẬP NHẬT metadata + game link +
    // ảnh mới thay vì chạm UNIQUE constraint rồi báo 409 vô nghĩa. Status
    // duyệt giữ nguyên (không reset pending nếu repo đã approved).
    if let Some(id) = repost_id {
        RepoRepo::update_repost(
            &state.db,
            id,
            game_id,
            &description,
            meta.homepage.as_deref().unwrap_or(""),
            meta.language.as_deref().unwrap_or(""),
            meta.stargazers_count.unwrap_or(0),
            meta.forks_count.unwrap_or(0),
            meta.open_issues_count.unwrap_or(0),
            meta.pushed_at,
            safe_image_url,
        )
        .await?;
        tracing::info!(
            "Repo re-posted (metadata refreshed): {}/{} by {} (id={})",
            owner,
            name,
            user.username,
            id
        );
        return Ok(Redirect::to("/repos"));
    }

    let repo_id = RepoRepo::create_full(
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
        safe_image_url,
        final_status,
    )
    .await?;

    tracing::info!(
        "Repo registered: {}/{} by {} (id={})",
        owner,
        name,
        user.username,
        repo_id
    );
    // v2.9.0 — XP + huy hiệu repo đầu tiên (best-effort)
    {
        let db = state.db.clone();
        let uid = user.id;
        tokio::spawn(async move {
            crate::services::gamification::on_repo(&db, uid).await;
        });
    }
    Ok(Redirect::to("/repos"))
}

// ============= Làm mới metadata 1 repo =============
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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

#[cfg(test)]
mod tests {
    use super::map_github_api_error;
    use crate::services::github::GithubApiError;
    use axum::http::StatusCode;

    /// Helper dựng GithubApiError cho test — thay cho hàm github_api_error
    /// cũ (v2.9.1 tách service github dùng chung, mapping lỗi giữ nguyên ngữ nghĩa).
    fn gh_err(status: u16, retry_after: Option<u64>) -> GithubApiError {
        GithubApiError {
            status: Some(status),
            retry_after,
            message: format!("test HTTP {status}"),
        }
    }

    /// REGRESSION v2.8.0 (bug "đăng repo liên tục 500"): 403/429 rate-limit
    /// PHẢI map sang BadRequest (400) với message rõ — không được rơi vào
    /// nhánh 500 "Oops! Lỗi hệ thống" vô nghĩa.
    #[test]
    fn test_rate_limit_maps_to_bad_request_not_500() {
        for status in [403u16, 429] {
            let e = map_github_api_error("o", "r", gh_err(status, None));
            let (st, msg) = e.status_and_message();
            assert_eq!(st, StatusCode::BAD_REQUEST, "HTTP {status} phải là 400");
            assert!(
                msg.contains("giới hạn"),
                "message phải nói rõ rate limit, thực tế: {msg}"
            );
        }
    }

    /// Retry-After (giây) được nhắc trong message → user biết chờ bao lâu.
    #[test]
    fn test_retry_after_hint_in_message() {
        let e = map_github_api_error("o", "r", gh_err(429, Some(300)));
        let (_, msg) = e.status_and_message();
        assert!(msg.contains("5 phút"), "phải nhắc 5 phút, thực tế: {msg}");
        // Retry-After quá dài (>= 1h) → hint chung, không khớp số phút lẻ.
        let e = map_github_api_error("o", "r", gh_err(403, Some(7200)));
        let (_, msg) = e.status_and_message();
        assert!(msg.contains("thử lại sau ít phút"));
    }

    /// Lỗi network (không có HTTP status) → BadRequest rõ ràng, KHÔNG 500 mù.
    #[test]
    fn test_network_error_maps_to_bad_request() {
        let e = map_github_api_error(
            "o",
            "r",
            GithubApiError {
                status: None,
                retry_after: None,
                message: "dns fail".into(),
            },
        );
        let (st, msg) = e.status_and_message();
        assert_eq!(st, StatusCode::BAD_REQUEST);
        assert!(msg.contains("không kết nối được"));
    }

    /// 404 → NotFound (404) — repo riêng tư/không tồn tại là lỗi user sửa được.
    #[test]
    fn test_404_maps_to_not_found() {
        let e = map_github_api_error("o", "r", gh_err(404, None));
        let (st, _) = e.status_and_message();
        assert_eq!(st, StatusCode::NOT_FOUND);
    }

    /// Status lạ (451 legal, 502/503 GitHub down...) → BadRequest rõ ràng,
    /// KHÔNG phải 500 mù mờ như trước đây (catch-all AppError::OAuth).
    #[test]
    fn test_unexpected_status_maps_to_bad_request_with_code() {
        for status in [451u16, 500, 502, 503] {
            let e = map_github_api_error("o", "r", gh_err(status, None));
            let (st, msg) = e.status_and_message();
            assert_eq!(st, StatusCode::BAD_REQUEST, "HTTP {status} phải là 400");
            assert!(
                msg.contains(&format!("HTTP {status}")),
                "message phải chứa mã HTTP, thực tế: {msg}"
            );
        }
    }

    /// 401 (token server sai) — lỗi cấu hình phía server, giữ 500 + log cho
    /// admin xử lý (user không tự sửa được).
    #[test]
    fn test_401_keeps_500_for_admin() {
        let e = map_github_api_error("o", "r", gh_err(401, None));
        let (st, _) = e.status_and_message();
        assert_eq!(st, StatusCode::INTERNAL_SERVER_ERROR);
    }
}
