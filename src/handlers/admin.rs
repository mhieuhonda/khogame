use crate::error::{AppError, AppResult};
use crate::handlers::auth::unread_count;
use crate::middleware::AuthUser;
use crate::models::report::ReportStatus;
use crate::repositories::{
    AdminLogRepo, AiAgentRepo, CategoryRepo, CommentRepo, GameRepo, NewsRepo, NotificationRepo,
    RepoRepo, ReportRepo, SessionRepo, SettingsRepo, StatsRepo, UserRepo,
};
use crate::state::AppState;
use crate::templates::{AdminTemplate, AdminReportsTemplate, CommentItemPartial, AdminGamesTemplate, AdminUsersTemplate, AdminCommentsTemplate, AdminCategoriesTemplate, AdminReposTemplate, AdminSettingsTemplate, AdminAuditTemplate, AdminSessionsTemplate, AdminAiAgentsTemplate, AdminAiReportsTemplate, AdminUserDetailTemplate, AdminNewsPendingTemplate, AdminNewsAllTemplate};
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

// ============= Helper: ghi audit log (best-effort) =============
async fn audit(
    state: &AppState,
    admin_id: Uuid,
    action: &str,
    target_type: &str,
    target_id: &str,
    detail: &str,
) {
    let _ = AdminLogRepo::log(
        &state.db,
        admin_id,
        action,
        target_type,
        target_id,
        detail,
        None,
    )
    .await;
}

// ============================================================
// DASHBOARD (kèm chart 7 ngày)
// ============================================================
pub async fn dashboard(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<AdminTemplate> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    // 10 truy vấn độc lập — join! chạy song song thay vì cộng dồn
    // round-trip DB khi admin mở dashboard. Thêm news stats (pending + total).
    let db = &state.db;
    let (
        total_games,
        total_users,
        total_downloads,
        pending_reports,
        recent_reports,
        recent_games,
        recent_comments,
        daily_stats,
        total_repos,
        pending_repos,
        status_counts,
        pending_news,
        total_news,
    ) = tokio::join!(
        GameRepo::count_published(db),
        UserRepo::count_all(db),
        async {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM downloads")
                .fetch_one(db)
                .await
                .unwrap_or(0)
        },
        ReportRepo::count_pending(db),
        ReportRepo::list(db, Some("pending"), 10, 0),
        GameRepo::list_published(db, 10, 0, "latest"),
        CommentRepo::list_recent(db, 5, 0),
        StatsRepo::daily_last_7_days(db),
        RepoRepo::count_approved(db),
        RepoRepo::pending_count(db),
        GameRepo::count_by_status(db),
        NewsRepo::count_pending(db),
        NewsRepo::count_by_status(db, crate::models::news::NewsStatus::Published),
    );
    let total_games = total_games.unwrap_or(0);
    let total_users = total_users.unwrap_or(0);
    let pending_reports = pending_reports.unwrap_or(0);
    let recent_reports = recent_reports.unwrap_or_default();
    let recent_games = recent_games.unwrap_or_default();
    let recent_comments = recent_comments.unwrap_or_default();
    let daily_stats = daily_stats.unwrap_or_default();
    let total_repos = total_repos.unwrap_or(0);
    let pending_repos = pending_repos.unwrap_or(0);
    let status_counts = status_counts
        .unwrap_or_default()
        .into_iter()
        .map(|(key, count)| crate::templates::StatusCountChip {
            label: crate::models::game::GameStatus::from_str(&key)
                .label()
                .to_string(),
            key,
            count,
        })
        .collect();
    let max_views = daily_stats
        .iter()
        .map(|d| d.views)
        .max()
        .unwrap_or(1)
        .max(1);
    let max_downloads = daily_stats
        .iter()
        .map(|d| d.downloads)
        .max()
        .unwrap_or(1)
        .max(1);
    let unread = unread_count(&state, user.id).await;
    Ok(AdminTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        total_games,
        total_users,
        total_downloads,
        pending_reports,
        recent_reports,
        recent_games,
        recent_comments,
        daily_stats,
        total_repos,
        pending_repos,
        status_counts,
        max_views,
        max_downloads,
        pending_news: pending_news.unwrap_or(0),
        total_news: total_news.unwrap_or(0),
    })
}

// ============================================================
// REPORTS (giữ hành vi cũ)
// ============================================================
#[derive(Deserialize, Default)]
pub struct ReportsQuery {
    pub status: Option<String>,
    pub page: Option<i64>,
}

pub async fn reports(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Query(q): Query<ReportsQuery>,
) -> AppResult<AdminReportsTemplate> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    // Chuẩn hoá status: None/""/whitespace → không filter (tránh branch
    // SQL lệch nhau giữa list và count).
    let status = q.status.as_deref().filter(|s| !s.trim().is_empty());
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 50;
    let offset = (page - 1) * per_page;
    // list + count độc lập — join! song song
    let (reports_res, total_res) = tokio::join!(
        ReportRepo::list(&state.db, status, per_page, offset),
        ReportRepo::count(&state.db, status),
    );
    let reports = reports_res?;
    let total = total_res.unwrap_or(0);
    let unread = unread_count(&state, user.id).await;
    Ok(AdminReportsTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        reports,
        status_filter: q.status,
        page,
        per_page,
        total,
    })
}

#[derive(Deserialize)]
pub struct ResolveForm {
    pub status: String,
    pub resolution: Option<String>,
}

pub async fn resolve_report(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Form(form): Form<ResolveForm>,
) -> AppResult<Html<String>> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let status = match form.status.as_str() {
        "reviewing" => ReportStatus::Reviewing,
        "resolved" => ReportStatus::Resolved,
        "dismissed" => ReportStatus::Dismissed,
        _ => return Err(AppError::BadRequest("Trạng thái không hợp lệ".into())),
    };
    // Resolution ≤ 2000 ký tự — cùng giới hạn với description lúc user
    // gửi báo cáo (DB TEXT không constraint, chặn sớm ở handler).
    let resolution = form.resolution.unwrap_or_default();
    if resolution.chars().count() > 2000 {
        return Err(AppError::BadRequest(
            "Nội dung xử lý tối đa 2000 ký tự".into(),
        ));
    }
    ReportRepo::resolve(&state.db, id, user.id, &form.status, &resolution).await?;
    audit(
        &state,
        user.id,
        "report.resolve",
        "report",
        &id.to_string(),
        &format!("-> {status:?}"),
    )
    .await;

    // Lấy đúng 1 report đã cập nhật để re-render row (trước đây fetch
    // cả danh sách 50 report rồi find theo id — 1 query thừa mỗi lần resolve).
    let r = ReportRepo::find_with_game(&state.db, id).await?;
    // Bọc trong .report-row để khớp hx-target="closest .admin-report-row"
    // outerHTML ở trang admin/reports (trước đây mất wrapper → vỡ layout)
    let html = if let Some(r) = r {
        format!(
            r#"<div class="report-row admin-report-row" id="report-{id}"><div class="report-info"><a href="/games/{slug}" class="report-game-title">{title}</a><div class="report-meta"><span class="report-reason">{reason}</span><span class="report-reporter">bởi {reporter}</span><span class="report-time">{time}</span></div></div><div class="report-actions"><span class="status-badge" style="color: {color}">{status_label}</span></div></div>"#,
            id = r.id,
            slug = r.game_slug,
            title = crate::utils::html_escape(&r.game_title),
            reason = r.reason.label(),
            reporter = crate::utils::html_escape(&r.reporter_name),
            time = crate::utils::time_ago(r.created_at),
            color = r.status.color(),
            status_label = r.status.label()
        )
    } else {
        String::new()
    };
    Ok(Html(html))
}

#[derive(Deserialize, Default)]
pub struct HideGameForm {
    pub hide: Option<String>,
}

pub async fn hide_game(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Form(form): Form<HideGameForm>,
) -> AppResult<Html<String>> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let status = if form.hide.is_some() {
        "hidden"
    } else {
        "published"
    };
    GameRepo::set_status(&state.db, id, status).await?;
    audit(
        &state,
        user.id,
        "game.status",
        "game",
        &id.to_string(),
        status,
    )
    .await;
    Ok(Html(format!(
        "<div class='alert alert-success'>Đã {} game.</div>",
        if status == "hidden" { "ẩn" } else { "hiện" }
    )))
}

pub async fn feature_game(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Html<String>> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let game = GameRepo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    GameRepo::set_featured(&state.db, id, !game.is_featured).await?;
    audit(
        &state,
        user.id,
        "game.feature",
        "game",
        &id.to_string(),
        if game.is_featured { "off" } else { "on" },
    )
    .await;
    Ok(Html(format!(
        "<div class='alert alert-success'>Đã {} nổi bật.</div>",
        if game.is_featured {
            "bỏ"
        } else {
            "đặt làm"
        }
    )))
}

pub async fn pin_comment(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Html<String>> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let pinned = CommentRepo::toggle_pin(&state.db, id).await?;
    let comment = CommentRepo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Bình luận không tồn tại".into()))?;
    // Lấy slug game để form trả lời trong item vẫn trỏ đúng URL
    let game_slug = GameRepo::find_by_id(&state.db, comment.game_id)
        .await?
        .map(|g| g.slug)
        .unwrap_or_default();
    let partial = CommentItemPartial {
        comment: &comment,
        game_slug: &game_slug,
        current_user: Some(&user),
        load_replies: true,
    };
    audit(
        &state,
        user.id,
        "comment.pin",
        "comment",
        &id.to_string(),
        if pinned { "on" } else { "off" },
    )
    .await;
    Ok(Html(partial.render()?))
}

// ============================================================
// ADMIN: GAMES
// ============================================================
#[derive(Deserialize, Default)]
pub struct AdminGamesQuery {
    pub status: Option<String>,
    pub page: Option<i64>,
}

pub async fn games(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Query(q): Query<AdminGamesQuery>,
) -> AppResult<AdminGamesTemplate> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 50;
    let offset = (page - 1) * per_page;
    // 3 query độc lập — join! song song.
    let (games_res, total_res, status_counts_res) = tokio::join!(
        GameRepo::admin_list(&state.db, q.status.as_deref(), per_page, offset),
        GameRepo::count_admin(&state.db, q.status.as_deref()),
        GameRepo::count_by_status(&state.db),
    );
    let games = games_res?;
    let total = total_res.unwrap_or(0);
    let status_counts = status_counts_res
        .unwrap_or_default()
        .into_iter()
        .map(|(key, count)| crate::templates::StatusCountChip {
            label: crate::models::game::GameStatus::from_str(&key)
                .label()
                .to_string(),
            key,
            count,
        })
        .collect();
    let unread = unread_count(&state, user.id).await;
    Ok(AdminGamesTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        games,
        status_filter: q.status,
        status_counts,
        page,
        per_page,
        total,
    })
}

pub async fn delete_game(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Html<String>> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let game = GameRepo::find_by_id(&state.db, id).await?;
    GameRepo::delete(&state.db, id).await?;
    if let Some(g) = game {
        audit(
            &state,
            user.id,
            "game.delete",
            "game",
            &id.to_string(),
            &g.title,
        )
        .await;
    }
    Ok(Html(
        "<div class='alert alert-success'>Đã xóa game vĩnh viễn.</div>".into(),
    ))
}

// ============================================================
// ADMIN: USERS
// ============================================================
#[derive(Deserialize, Default)]
pub struct AdminUsersQuery {
    pub q: Option<String>,
    pub page: Option<i64>,
}

pub async fn users(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Query(q): Query<AdminUsersQuery>,
) -> AppResult<AdminUsersTemplate> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden("Chỉ quản trị viên tối cao".into()));
    }
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 50;
    let offset = (page - 1) * per_page;
    let search = q.q.as_deref().filter(|s| !s.trim().is_empty());
    // 2 query độc lập — join! song song.
    let (users_res, total_res) = tokio::join!(
        UserRepo::list_for_admin(&state.db, search, per_page, offset),
        // Tổng theo bộ lọc (không phải tổng toàn site) để phân trang đúng
        UserRepo::count_for_admin(&state.db, search),
    );
    let users = users_res?;
    let total = total_res.unwrap_or(0);
    let unread = unread_count(&state, user.id).await;
    Ok(AdminUsersTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        users,
        search: q.q.unwrap_or_default(),
        total,
        page,
        per_page,
    })
}

#[derive(Deserialize)]
pub struct RoleForm {
    pub role: String,
}

pub async fn set_role(
    State(state): State<Arc<AppState>>,
    AuthUser(admin): AuthUser,
    Path(id): Path<Uuid>,
    Form(form): Form<RoleForm>,
) -> AppResult<Html<String>> {
    if !admin.role.is_admin() {
        return Err(AppError::Forbidden("Chỉ quản trị viên tối cao".into()));
    }
    if !matches!(form.role.as_str(), "user" | "moderator" | "admin") {
        return Err(AppError::BadRequest("Vai trò không hợp lệ".into()));
    }
    if id == admin.id && form.role != "admin" {
        return Err(AppError::BadRequest(
            "Không thể tự hạ quyền của chính mình".into(),
        ));
    }
    UserRepo::set_role(&state.db, id, &form.role).await?;
    audit(
        &state,
        admin.id,
        "user.role",
        "user",
        &id.to_string(),
        &form.role,
    )
    .await;
    Ok(Html(format!(
        "<span class='role-badge role-{}'>{}</span>",
        form.role,
        match form.role.as_str() {
            "admin" => "Quản trị viên",
            "moderator" => "Điều hành viên",
            _ => "Thành viên",
        }
    )))
}

#[derive(Deserialize, Default)]
pub struct BanForm {
    pub ban: Option<String>,
}

pub async fn set_banned(
    State(state): State<Arc<AppState>>,
    AuthUser(admin): AuthUser,
    Path(id): Path<Uuid>,
    Form(form): Form<BanForm>,
) -> AppResult<Html<String>> {
    if !admin.role.is_admin() {
        return Err(AppError::Forbidden("Chỉ quản trị viên tối cao".into()));
    }
    if id == admin.id {
        return Err(AppError::BadRequest("Không thể tự cấm chính mình".into()));
    }
    let banned = form.ban.is_some();
    UserRepo::set_banned(&state.db, id, banned).await?;
    audit(
        &state,
        admin.id,
        "user.ban",
        "user",
        &id.to_string(),
        if banned { "banned" } else { "unbanned" },
    )
    .await;
    Ok(Html(if banned {
        "<span class='status-badge' style='color:#ef4444'>Bị cấm</span>".into()
    } else {
        "<span class='status-badge' style='color:#10b981'>Hoạt động</span>".into()
    }))
}

// ============================================================
// ADMIN: COMMENTS
// ============================================================
#[derive(Deserialize, Default)]
pub struct AdminCommentsQuery {
    pub page: Option<i64>,
}

pub async fn comments(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Query(q): Query<AdminCommentsQuery>,
) -> AppResult<AdminCommentsTemplate> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    // Phân trang: trước đây list_recent(100) cứng — quá 100 comment là
    // comment cũ mất dạng, admin không thể kiểm duyệt.
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 50;
    let offset = (page - 1) * per_page;
    let (comments_res, total_res) = tokio::join!(
        CommentRepo::list_recent(&state.db, per_page, offset),
        CommentRepo::count_all(&state.db),
    );
    let comments = comments_res?;
    let total = total_res.unwrap_or(0);
    let unread = unread_count(&state, user.id).await;
    Ok(AdminCommentsTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        comments,
        page,
        per_page,
        total,
    })
}

pub async fn delete_comment(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Html<String>> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    CommentRepo::delete(&state.db, id).await?;
    audit(
        &state,
        user.id,
        "comment.delete",
        "comment",
        &id.to_string(),
        "",
    )
    .await;
    Ok(Html(
        "<div class='alert alert-success'>Đã xóa bình luận.</div>".into(),
    ))
}

// ============================================================
// ADMIN: CATEGORIES
// ============================================================
pub async fn categories(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<AdminCategoriesTemplate> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let cats = CategoryRepo::list_with_counts(&state.db).await?;
    let unread = unread_count(&state, user.id).await;
    Ok(AdminCategoriesTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        categories: cats,
    })
}

#[derive(Deserialize)]
pub struct CategoryForm {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub id: Option<String>,
}

pub async fn save_category(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Form(form): Form<CategoryForm>,
) -> AppResult<Redirect> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let name = form.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Tên thể loại không được trống".into()));
    }
    if name.chars().count() > 50 {
        return Err(AppError::BadRequest("Tên thể loại tối đa 50 ký tự".into()));
    }
    let description = form.description.unwrap_or_default();
    let description = description.trim();
    if description.chars().count() > 500 {
        return Err(AppError::BadRequest(
            "Mô tả thể loại tối đa 500 ký tự".into(),
        ));
    }
    let icon = form.icon.unwrap_or_default();
    let icon = icon.trim();
    if icon.chars().count() > 100 {
        return Err(AppError::BadRequest("Icon tối đa 100 ký tự".into()));
    }
    if let Some(id) = form
        .id
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| Uuid::parse_str(s).ok()) {
        CategoryRepo::update(&state.db, id, name, description, icon).await?;
        audit(
            &state,
            user.id,
            "category.update",
            "category",
            &id.to_string(),
            name,
        )
        .await;
    } else {
        let slug = slug::slugify(name);
        let id = CategoryRepo::create(&state.db, name, &slug, description, icon).await?;
        audit(
            &state,
            user.id,
            "category.create",
            "category",
            &id.to_string(),
            name,
        )
        .await;
    }
    Ok(Redirect::to("/admin/categories"))
}

pub async fn delete_category(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Redirect> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden("Chỉ quản trị viên tối cao".into()));
    }
    // Kiểm tra còn game không
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM games WHERE category_id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    if count > 0 {
        return Err(AppError::BadRequest(format!(
            "Thể loại còn {count} game. Hãy chuyển game sang thể loại khác trước."
        )));
    }
    CategoryRepo::delete(&state.db, id).await?;
    audit(
        &state,
        user.id,
        "category.delete",
        "category",
        &id.to_string(),
        "",
    )
    .await;
    Ok(Redirect::to("/admin/categories"))
}

// ============================================================
// ADMIN: REPOS moderation
// ============================================================
#[derive(Deserialize, Default)]
pub struct AdminReposQuery {
    pub status: Option<String>,
    pub page: Option<i64>,
}

pub async fn repos(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Query(q): Query<AdminReposQuery>,
) -> AppResult<AdminReposTemplate> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    // Chuẩn hoá status filter + phân trang 50/trang (trước đây list 100
    // cứng — quá 100 repo là repo cũ không duyệt/xem được).
    let status = q.status.as_deref().filter(|s| !s.trim().is_empty());
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 50;
    let offset = (page - 1) * per_page;
    let (repos_res, total_res) = tokio::join!(
        RepoRepo::list_admin(&state.db, status, per_page, offset),
        RepoRepo::count_admin(&state.db, status),
    );
    let repos = repos_res?;
    let total = total_res.unwrap_or(0);
    let unread = unread_count(&state, user.id).await;
    Ok(AdminReposTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        repos,
        status_filter: q.status,
        page,
        per_page,
        total,
    })
}

#[derive(Deserialize)]
pub struct RepoStatusForm {
    pub status: String,
}

pub async fn set_repo_status(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Form(form): Form<RepoStatusForm>,
) -> AppResult<Html<String>> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    if !matches!(form.status.as_str(), "pending" | "approved" | "hidden") {
        return Err(AppError::BadRequest("Trạng thái không hợp lệ".into()));
    }
    RepoRepo::set_status(&state.db, id, &form.status).await?;
    audit(
        &state,
        user.id,
        "repo.status",
        "repo",
        &id.to_string(),
        &form.status,
    )
    .await;
    Ok(Html(format!(
        "<span class='status-badge' style='color:{}'>{}</span>",
        match form.status.as_str() {
            "pending" => "#f59e0b",
            "approved" => "#10b981",
            _ => "#ef4444",
        },
        match form.status.as_str() {
            "pending" => "Chờ duyệt",
            "approved" => "Đã duyệt",
            _ => "Đã ẩn",
        }
    )))
}

// ============================================================
// ADMIN: SETTINGS + ANNOUNCEMENT + MAINTENANCE
// ============================================================
pub async fn settings_page(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<AdminSettingsTemplate> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden("Chỉ quản trị viên tối cao".into()));
    }
    let mut map = SettingsRepo::get_map(
        &state.db,
        &[
            "site_name",
            "site_description",
            "maintenance_mode",
            "announcement",
            "announcement_type",
            "footer_text",
            "repo_auto_approve",
        ],
    )
    .await?;
    let get = |m: &mut std::collections::HashMap<String, String>, k: &str| {
        m.remove(k).unwrap_or_default()
    };
    let unread = unread_count(&state, user.id).await;
    Ok(AdminSettingsTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        site_name: get(&mut map, "site_name"),
        site_description: get(&mut map, "site_description"),
        maintenance_mode: get(&mut map, "maintenance_mode") == "on",
        announcement: get(&mut map, "announcement"),
        announcement_type: get(&mut map, "announcement_type"),
        footer_text: get(&mut map, "footer_text"),
        repo_auto_approve: get(&mut map, "repo_auto_approve") != "off",
        saved: false,
    })
}

#[derive(Deserialize, Default)]
pub struct SettingsForm {
    pub site_name: Option<String>,
    pub site_description: Option<String>,
    pub maintenance_mode: Option<String>,
    pub announcement: Option<String>,
    pub announcement_type: Option<String>,
    pub footer_text: Option<String>,
    pub repo_auto_approve: Option<String>,
}

pub async fn save_settings(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Form(form): Form<SettingsForm>,
) -> AppResult<AdminSettingsTemplate> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden("Chỉ quản trị viên tối cao".into()));
    }
    // Validate độ dài các trường text để chống lạm dụng (DB field là TEXT,
    // không có length limit, admin có thể vô tình paste payload lớn).
    let site_name = form.site_name.as_deref().unwrap_or("Louis Space").trim();
    if site_name.chars().count() > 100 {
        return Err(AppError::BadRequest("Tên site tối đa 100 ký tự".into()));
    }
    let site_description = form.site_description.as_deref().unwrap_or("").trim();
    if site_description.chars().count() > 500 {
        return Err(AppError::BadRequest("Mô tả site tối đa 500 ký tự".into()));
    }
    let announcement = form.announcement.as_deref().unwrap_or("").trim();
    if announcement.chars().count() > 500 {
        return Err(AppError::BadRequest("Announcement tối đa 500 ký tự".into()));
    }
    let footer_text = form.footer_text.as_deref().unwrap_or("").trim();
    if footer_text.chars().count() > 500 {
        return Err(AppError::BadRequest("Footer text tối đa 500 ký tự".into()));
    }
    // Validate announcement_type phải là một trong các giá trị hợp lệ
    let announcement_type = form
        .announcement_type
        .as_deref()
        .filter(|s| matches!(*s, "info" | "success" | "warning" | "danger"))
        .unwrap_or("info");
    let uid = user.id;
    // 7 lần ghi settings độc lập — join! song song thay vì tuần tự
    // (trước đây 7 round-trip liên tiếp mỗi lần admin bấm Lưu).
    let (r1, r2, r3, r4, r5, r6, r7) = tokio::join!(
        SettingsRepo::set(&state.db, "site_name", site_name, Some(uid)),
        SettingsRepo::set(&state.db, "site_description", site_description, Some(uid)),
        SettingsRepo::set(
            &state.db,
            "maintenance_mode",
            if form.maintenance_mode.is_some() {
                "on"
            } else {
                "off"
            },
            Some(uid),
        ),
        SettingsRepo::set(&state.db, "announcement", announcement, Some(uid)),
        SettingsRepo::set(&state.db, "announcement_type", announcement_type, Some(uid)),
        SettingsRepo::set(&state.db, "footer_text", footer_text, Some(uid)),
        SettingsRepo::set(
            &state.db,
            "repo_auto_approve",
            if form.repo_auto_approve.is_some() {
                "on"
            } else {
                "off"
            },
            Some(uid),
        ),
    );
    r1?;
    r2?;
    r3?;
    r4?;
    r5?;
    r6?;
    r7?;
    audit(&state, user.id, "settings.save", "settings", "", "").await;

    let mut map = SettingsRepo::get_map(
        &state.db,
        &[
            "site_name",
            "site_description",
            "maintenance_mode",
            "announcement",
            "announcement_type",
            "footer_text",
            "repo_auto_approve",
        ],
    )
    .await?;
    let get = |m: &mut std::collections::HashMap<String, String>, k: &str| {
        m.remove(k).unwrap_or_default()
    };
    let unread = unread_count(&state, user.id).await;
    Ok(AdminSettingsTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        site_name: get(&mut map, "site_name"),
        site_description: get(&mut map, "site_description"),
        maintenance_mode: get(&mut map, "maintenance_mode") == "on",
        announcement: get(&mut map, "announcement"),
        announcement_type: get(&mut map, "announcement_type"),
        footer_text: get(&mut map, "footer_text"),
        repo_auto_approve: get(&mut map, "repo_auto_approve") != "off",
        saved: true,
    })
}

// Gửi thông báo hệ thống tới toàn bộ người dùng
#[derive(Deserialize)]
pub struct BroadcastForm {
    pub title: String,
    pub content: Option<String>,
    pub link: Option<String>,
}

pub async fn broadcast(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Form(form): Form<BroadcastForm>,
) -> AppResult<Html<String>> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden("Chỉ quản trị viên tối cao".into()));
    }
    if form.title.trim().is_empty() {
        return Err(AppError::BadRequest("Tiêu đề thông báo trống".into()));
    }
    if form.title.chars().count() > 200 {
        return Err(AppError::BadRequest("Tiêu đề tối đa 200 ký tự".into()));
    }
    // Validate link: chỉ chấp nhận relative URL (bắt đầu bằng '/') hoặc
    // http(s):// tuyệt đối — chặn javascript: scheme để chống XSS khi user
    // click vào notification link.
    let link = form.link.as_deref().unwrap_or("").trim();
    if !link.is_empty() {
        let is_safe = link.starts_with('/')
            && !link.starts_with("//") // protocol-relative
            || crate::utils::is_safe_url(link);
        if !is_safe {
            return Err(AppError::BadRequest(
                "Link thông báo phải là đường dẫn nội bộ (/path) hoặc http(s):// URL".into(),
            ));
        }
        if link.len() > 2048 {
            return Err(AppError::BadRequest(
                "Link quá dài (tối đa 2048 ký tự)".into(),
            ));
        }
    }
    let content = form.content.as_deref().unwrap_or("").trim();
    if content.chars().count() > 1000 {
        return Err(AppError::BadRequest("Nội dung tối đa 1000 ký tự".into()));
    }
    let sent = NotificationRepo::broadcast(&state.db, form.title.trim(), content, link).await?;
    audit(
        &state,
        user.id,
        "notification.broadcast",
        "system",
        "",
        &format!("{sent} users"),
    )
    .await;
    Ok(Html(format!(
        "<div class='alert alert-success'>Đã gửi thông báo tới {sent} người dùng.</div>"
    )))
}

// ============================================================
// ADMIN: AUDIT LOG
// ============================================================
#[derive(Deserialize, Default)]
pub struct AuditQuery {
    pub page: Option<i64>,
}

pub async fn audit_log(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Query(q): Query<AuditQuery>,
) -> AppResult<AdminAuditTemplate> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden("Chỉ quản trị viên tối cao".into()));
    }
    // Phân trang: trước đây list(200) cứng — audit log tích lũy vô hạn
    // (mọi action admin), quá 200 dòng là lịch sử cũ không xem được.
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 100;
    let offset = (page - 1) * per_page;
    let (logs_res, total_res) = tokio::join!(
        AdminLogRepo::list(&state.db, per_page, offset),
        AdminLogRepo::count(&state.db),
    );
    let logs = logs_res?;
    let total = total_res.unwrap_or(0);
    let unread = unread_count(&state, user.id).await;
    Ok(AdminAuditTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        logs,
        page,
        per_page,
        total,
    })
}

// ============================================================
// SESSIONS — quản lý phiên đăng nhập đang hoạt động
// ============================================================
pub async fn sessions(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<AdminSessionsTemplate> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden("Chỉ quản trị viên tối cao".into()));
    }
    let sessions = SessionRepo::list_active(&state.db, 200).await?;
    let unread = unread_count(&state, user.id).await;
    Ok(AdminSessionsTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        sessions,
    })
}

/// Thu hồi 1 phiên (buộc đăng xuất thiết bị đó). Không cho thu hồi
/// phiên của chính mình qua endpoint này — dùng /auth/logout hoặc
/// /auth/logout-all để tránh tự khoá mình khỏi admin giữa phiên.
pub async fn revoke_session(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Redirect> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden("Chỉ quản trị viên tối cao".into()));
    }
    let deleted = SessionRepo::delete_by_id(&state.db, id).await?;
    audit(
        &state,
        user.id,
        "session.revoke",
        "session",
        &id.to_string(),
        if deleted {
            "đã thu hồi"
        } else {
            "không tồn tại"
        },
    )
    .await;
    if !deleted {
        return Err(AppError::NotFound(
            "Phiên không tồn tại (có thể đã hết hạn)".into(),
        ));
    }
    tracing::info!(admin = %user.username, session = %id, "Admin thu hồi phiên đăng nhập");
    Ok(Redirect::to("/admin/sessions"))
}

// ============================================================
// ADMIN: EXPORT BACKUP (JSON)
// ============================================================
pub async fn export(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<Response> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden("Chỉ quản trị viên tối cao".into()));
    }
    // 5 truy vấn export độc lập (games/users/repos/comments/reports) —
    // join! song song; với limit 10-20k dòng mỗi bảng mức tiết kiệm đáng kể.
    let (games_res, users_res, repos_res, comments_res, reports_res) = tokio::join!(
        GameRepo::admin_list(&state.db, None, 10000, 0),
        UserRepo::list_for_admin(&state.db, None, 10000, 0),
        RepoRepo::list_admin(&state.db, None, 10000, 0),
        CommentRepo::list_recent(&state.db, 20000, 0),
        ReportRepo::list(&state.db, None, 20000, 0),
    );
    let games = games_res?;
    let users = users_res?;
    let repos = repos_res?;
    // Bình luận + report cũng là dữ liệu cần backup — trước đây export
    // thiếu hoàn toàn, mất dữ liệu kiểm duyệt khi restore.
    let comments = comments_res?;
    let reports = reports_res?;
    let body = serde_json::json!({
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION"),
        "counts": {
            "games": games.len(),
            "users": users.len(),
            "repos": repos.len(),
            "comments": comments.len(),
            "reports": reports.len(),
        },
        "comments": comments,
        "reports": reports,
        "games": games,
        "users": users.iter().map(|u| serde_json::json!({
            "id": u.id, "email": u.email, "username": u.username,
            "display_name": u.display_name, "role": u.role, "is_banned": u.is_banned,
            "games_count": u.games_count, "created_at": u.created_at,
        })).collect::<Vec<_>>(),
        "repos": repos,
    });
    audit(&state, user.id, "system.export", "system", "", "").await;
    Ok((
        [
            (
                axum::http::header::CONTENT_TYPE,
                "application/json; charset=utf-8",
            ),
            (
                axum::http::header::CONTENT_DISPOSITION,
                "attachment; filename=\"khogame-backup.json\"",
            ),
        ],
        serde_json::to_string_pretty(&body).unwrap_or_default(),
    )
        .into_response())
}

// ============================================================
// AI Agent admin handlers
// ============================================================

/// Trang /admin/ai-agents — danh sách tất cả AI Agent (chỉ admin/staff).
pub async fn ai_agents(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<AdminAiAgentsTemplate> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let agents = AiAgentRepo::list_for_admin(&state.db)
        .await
        .unwrap_or_default();
    let unread = unread_count(&state, user.id).await;
    Ok(AdminAiAgentsTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        agents,
    })
}

/// Trang /admin/ai-reports — live feed báo cáo tiến trình từ AI Agent.
pub async fn ai_reports(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<AdminAiReportsTemplate> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let reports = AiAgentRepo::list_progress_recent(&state.db, 100)
        .await
        .unwrap_or_default();
    let total_agents = AiAgentRepo::count_all(&state.db).await.unwrap_or(0);
    let unread = unread_count(&state, user.id).await;
    Ok(AdminAiReportsTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        reports,
        total_agents,
    })
}

// ============================================================
// News admin — chỉ admin (không phải mod) được duyệt tin
// Lý do: duyệt tin tức có tác động lớn đến uy tín site, mod không đủ trust.
// ============================================================

#[derive(Deserialize)]
pub struct NewsListParams {
    pub page: Option<i64>,
}

const ADMIN_NEWS_PER_PAGE: i64 = 20;

/// GET /admin/users/{id} — trang chi tiết 1 user cho admin.
///
/// **QUYỀN**: Chỉ admin (`is_admin`), không phải moderator.
/// Moderator không được xem email/IP/UA/sessions của user.
///
/// Hiển thị:
/// - Email, username, `display_name`, avatar, bio
/// - Vai trò + trạng thái banned
/// - IP/UA lúc signup, IP/UA lúc last login, `last_seen_at`
/// - Danh sách sessions (có `IP/UA/expires_at`)
/// - Số game đã đăng, số news đã đăng
/// - Nút đổi role, ban/unban, revoke all sessions
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn user_detail(
    State(state): State<Arc<AppState>>,
    AuthUser(admin): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<AdminUserDetailTemplate> {
    if !admin.role.is_admin() {
        return Err(AppError::Forbidden(
            "Chỉ admin được xem chi tiết người dùng. Moderator chỉ thấy danh sách rút gọn.".into(),
        ));
    }
    let user = UserRepo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Người dùng không tồn tại".into()))?;

    // Count song song — pass pool clone (PgPool is Arc internally, clone is cheap)
    let db_clone1 = state.db.clone();
    let db_clone2 = state.db.clone();
    let (games_count, news_count, active_sessions, sessions) = tokio::join!(
        async move {
            let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM games WHERE user_id = $1")
                .bind(id)
                .fetch_one(&db_clone1)
                .await
                .unwrap_or(0);
            c
        },
        async move {
            let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM news WHERE user_id = $1")
                .bind(id)
                .fetch_one(&db_clone2)
                .await
                .unwrap_or(0);
            c
        },
        SessionRepo::count_active_for_user(&state.db, id),
        SessionRepo::list_for_user(&state.db, id, 50),
    );
    let is_self = id == admin.id;
    let unread = unread_count(&state, admin.id).await;

    Ok(AdminUserDetailTemplate {
        current_user: Some(admin),
        unread_notifications: unread,
        user,
        games_count,
        news_count,
        active_sessions: active_sessions.unwrap_or(0),
        sessions: sessions.unwrap_or_default(),
        is_self,
        now: chrono::Utc::now(),
    })
}

/// /admin/news/pending — hàng đợi tin chờ duyệt (chỉ admin).
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn news_pending(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Query(params): Query<NewsListParams>,
) -> AppResult<AdminNewsPendingTemplate> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden(
            "Chỉ admin được duyệt tin tức. Moderator không có quyền này.".into(),
        ));
    }
    let page = params.page.unwrap_or(1).max(1);
    let items = NewsRepo::list_pending(&state.db, page, ADMIN_NEWS_PER_PAGE).await?;
    let total = NewsRepo::count_pending(&state.db).await?;
    let total_pages = ((total + ADMIN_NEWS_PER_PAGE - 1) / ADMIN_NEWS_PER_PAGE).max(1);
    let unread = unread_count(&state, user.id).await;
    Ok(AdminNewsPendingTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        items,
        total,
        page,
        total_pages,
    })
}

/// /admin/news/all — tất cả tin (published/archived/rejected), chỉ admin.
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn news_all(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Query(params): Query<NewsListParams>,
) -> AppResult<AdminNewsAllTemplate> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden(
            "Chỉ admin xem được tất cả tin tức.".into(),
        ));
    }
    let page = params.page.unwrap_or(1).max(1);
    let offset = (page - 1).max(0) * ADMIN_NEWS_PER_PAGE;
    let items = sqlx::query_as::<_, crate::models::news::NewsForAdmin>(
        r"SELECT n.*, u.display_name AS author_name,
                 u.username AS author_username, u.email AS author_email,
                 u.avatar_url AS author_avatar
          FROM news n
          JOIN users u ON u.id = n.user_id
          ORDER BY n.created_at DESC
          LIMIT $1 OFFSET $2",
    )
    .bind(ADMIN_NEWS_PER_PAGE)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM news")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
    let total_pages = ((total + ADMIN_NEWS_PER_PAGE - 1) / ADMIN_NEWS_PER_PAGE).max(1);
    let unread = unread_count(&state, user.id).await;
    Ok(AdminNewsAllTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        items,
        total,
        page,
        total_pages,
    })
}

#[derive(Deserialize)]
pub struct RejectForm {
    pub note: Option<String>,
}

/// POST /admin/news/{id}/approve — duyệt tin (pending → published).
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn news_approve(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Response> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden("Chỉ admin được duyệt tin".into()));
    }
    let news = NewsRepo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Tin không tồn tại".into()))?;
    NewsRepo::approve(&state.db, id, user.id).await?;
    // Notify tác giả
    let _ = NotificationRepo::create_system(
        &state.db,
        news.user_id,
        &format!("Tin '{}' đã được duyệt", news.title),
        "",
        &format!("/news/{}", news.slug),
    )
    .await;
    audit(
        &state,
        user.id,
        "news_approve",
        "news",
        &id.to_string(),
        &format!("Approved: {}", news.title),
    )
    .await;
    Ok(Redirect::to("/admin/news/pending").into_response())
}

/// POST /admin/news/{id}/reject — từ chối tin (pending → rejected).
pub async fn news_reject(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Form(form): Form<RejectForm>,
) -> AppResult<Response> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden("Chỉ admin được từ chối tin".into()));
    }
    let news = NewsRepo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Tin không tồn tại".into()))?;
    let note = form.note.as_deref().unwrap_or("").trim();
    NewsRepo::reject(&state.db, id, user.id, note).await?;
    let _ = NotificationRepo::create_system(
        &state.db,
        news.user_id,
        &format!("Tin '{}' bị từ chối", news.title),
        note,
        &format!("/news/{}/edit", news.slug),
    )
    .await;
    audit(
        &state,
        user.id,
        "news_reject",
        "news",
        &id.to_string(),
        &format!("Rejected ({}): {}", note, news.title),
    )
    .await;
    Ok(Redirect::to("/admin/news/pending").into_response())
}

/// POST /admin/news/{id}/archive — lưu trữ tin đã published.
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn news_archive(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Response> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden("Chỉ admin".into()));
    }
    let news = NewsRepo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Tin không tồn tại".into()))?;
    NewsRepo::archive(&state.db, id).await?;
    audit(
        &state,
        user.id,
        "news_archive",
        "news",
        &id.to_string(),
        &format!("Archived: {}", news.title),
    )
    .await;
    Ok(Redirect::to("/admin/news/all").into_response())
}

/// POST /admin/news/{id}/feature — đặt tin làm nổi bật.
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn news_feature(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Response> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden("Chỉ admin".into()));
    }
    NewsRepo::set_featured(&state.db, id, true).await?;
    audit(&state, user.id, "news_feature", "news", &id.to_string(), "").await;
    Ok(Redirect::to("/admin/news/all").into_response())
}

/// POST /admin/news/{id}/unfeature — bỏ nổi bật.
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn news_unfeature(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Response> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden("Chỉ admin".into()));
    }
    NewsRepo::set_featured(&state.db, id, false).await?;
    audit(
        &state,
        user.id,
        "news_unfeature",
        "news",
        &id.to_string(),
        "",
    )
    .await;
    Ok(Redirect::to("/admin/news/all").into_response())
}

/// POST /admin/news/{id}/delete — xóa tin.
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn news_delete(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Response> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden("Chỉ admin được xóa tin".into()));
    }
    let news = NewsRepo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Tin không tồn tại".into()))?;
    NewsRepo::delete(&state.db, id).await?;
    audit(
        &state,
        user.id,
        "news_delete",
        "news",
        &id.to_string(),
        &format!("Deleted: {}", news.title),
    )
    .await;
    Ok(Redirect::to("/admin/news/all").into_response())
}
