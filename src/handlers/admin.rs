use crate::error::{AppError, AppResult};
use crate::handlers::auth::unread_count;
use crate::middleware::AuthUser;
use crate::models::report::ReportStatus;
use crate::repositories::{
    AdminLogRepo, AiAgentRepo, CategoryRepo, CommentRepo, GameRepo, NewsCategoryRepo, NewsRepo,
    NotificationRepo, RepoRepo, ReportRepo, SessionRepo, SettingsRepo, StatsRepo, UserRepo,
};
use crate::services::audit;
use crate::state::AppState;
use crate::templates::{
    AdminAiAgentsTemplate, AdminAiReportsTemplate, AdminAuditTemplate, AdminCategoriesTemplate,
    AdminCommentsTemplate, AdminGamesTemplate, AdminNewsAllTemplate, AdminNewsCategoriesTemplate,
    AdminNewsPendingTemplate, AdminReportsTemplate, AdminReposTemplate, AdminSessionsTemplate,
    AdminSettingsTemplate, AdminTemplate, AdminUserDetailTemplate, AdminUsersTemplate,
    CommentItemPartial, NewsCategoryWithCountView,
};
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

// `audit` helper được tái sử dụng từ `crate::services::audit` — không còn
// private local nữa. Handler khác (vd `ai_agent::register`) cũng có thể
// gọi `crate::services::audit::audit(...)` mà không phải lặp code.

// ============================================================
// DASHBOARD (kèm chart 7 ngày)
// ============================================================
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn dashboard(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<AdminTemplate> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    // 13 truy vấn độc lập — join! chạy song song thay vì cộng dồn
    // round-trip DB khi admin mở dashboard. v1.4.0 thêm: online users count,
    // recently active users, banned users count, total comments count.
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
        online_users,
        recent_active_users,
        banned_users,
        total_comments,
        total_views,
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
        // v1.4.0: số user online (last_seen trong 15 phút).
        async {
            sqlx::query_scalar::<_, i64>(
                r"SELECT COUNT(*) FROM users
                  WHERE NOT is_banned
                    AND last_seen_at IS NOT NULL
                    AND last_seen_at > NOW() - INTERVAL '15 minutes'",
            )
            .fetch_one(db)
            .await
            .unwrap_or(0)
        },
        // v1.4.0: 5 user hoạt động gần đây nhất.
        async {
            sqlx::query_as::<_, crate::models::user::UserWithGameCount>(
                r"SELECT u.id, u.email, u.username, u.display_name, u.avatar_url, u.bio,
                        u.google_sub, u.role, u.is_banned, u.last_seen_at,
                        u.created_at, u.updated_at,
                        u.signup_ip, u.signup_ua, u.last_login_ip, u.last_login_ua, u.last_login_at,
                        COUNT(g.id) FILTER (WHERE g.status = 'published')::bigint AS games_count
                  FROM users u
                  LEFT JOIN games g ON g.user_id = u.id
                  WHERE NOT u.is_banned
                    AND u.last_seen_at IS NOT NULL
                  GROUP BY u.id
                  ORDER BY u.last_seen_at DESC
                  LIMIT 5",
            )
            .fetch_all(db)
            .await
            .unwrap_or_default()
        },
        // v1.4.0: tổng user bị cấm — cho dashboard admin biết quy mô spam.
        async {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE is_banned")
                .fetch_one(db)
                .await
                .unwrap_or(0)
        },
        // v1.4.0: tổng comment — cho dashboard insight.
        async {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM comments")
                .fetch_one(db)
                .await
                .unwrap_or(0)
        },
        // v1.4.0: tổng view — SUM(view_count) trên games.
        async {
            sqlx::query_scalar::<_, i64>("SELECT COALESCE(SUM(view_count), 0) FROM games")
                .fetch_one(db)
                .await
                .unwrap_or(0)
        },
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
    // v1.4.0: 5 biến mới (online_users, banned_users, total_comments,
    // total_views, recent_active_users) đã unwrap_or/default trong async block
    // — không gọi unwrap_or() lần nữa (i64 không có method đó).
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
        online_users,
        recent_active_users,
        banned_users,
        total_comments,
        total_views,
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

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn pin_comment(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Html<String>> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    // Thử ghim bình luận GAME trước (bảng comments). toggle_pin dùng
    // fetch_one → RowNotFound → AppError::NotFound khi id không có trong
    // bảng → chuyển sang thử bảng news_comments (bình luận TIN TỨC —
    // trước đây pin news comment ở đây luôn 500 vì bảng khác).
    let game_pinned = match CommentRepo::toggle_pin(&state.db, id).await {
        Ok(p) => Some(p),
        Err(AppError::NotFound(_)) => None,
        Err(other) => return Err(other),
    };
    match game_pinned {
        Some(pinned) => {
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
        None => {
            // Bình luận TIN TỨC: toggle pin ở bảng news_comments và trả
            // snippet trạng thái (không có partial game comment item).
            let pinned = CommentRepo::toggle_pin_news(&state.db, id)
                .await?
                .ok_or_else(|| AppError::NotFound("Bình luận không tồn tại".into()))?;
            audit(
                &state,
                user.id,
                "comment.pin",
                "news_comment",
                &id.to_string(),
                if pinned { "on" } else { "off" },
            )
            .await;
            let label = if pinned {
                "📌 Đã ghim"
            } else {
                "Đã bỏ ghim"
            };
            Ok(Html(format!(
                "<span class=\"pin-zone\" id=\"pin-result-{id}\">{label}</span>"
            )))
        }
    }
}

// ============================================================
// ADMIN: GAMES
// ============================================================
#[derive(Deserialize, Default)]
pub struct AdminGamesQuery {
    pub status: Option<String>,
    pub page: Option<i64>,
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
    /// v1.4.0: filter theo trạng thái thật — `banned|new|online|active|inactive|dormant`.
    pub status: Option<String>,
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
    let status_filter = q
        .status
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_default()
        .to_string();
    // Lấy tất cả users thoả mãn search (không có status filter trong SQL —
    // status badge tính từ last_seen_at + created_at + is_banned nên không
    // thể filter trực tiếp trong WHERE_clause mà không nhồi điều kiện).
    // Cách tiếp cận: fetch tối đa 500 users, filter in-app theo badge,
    // paginate thủ công. 500 là đủ cho hầu hết site; khi user many hơn
    // sẽ cần đổ vào cột status badge trong DB (TODO v1.5).
    let (all_users_res, total_search_res) = tokio::join!(
        UserRepo::list_for_admin(&state.db, search, 500, 0),
        UserRepo::count_for_admin(&state.db, search),
    );
    let mut all_users = all_users_res?;
    // total_search: tổng user thoả mãn search TRƯỚC status filter — hiện
    // ở chip "Tất cả" để admin biết quy mô site. Tránh unused warning.
    let total_search = total_search_res.unwrap_or(0);
    let now = chrono::Utc::now();
    // Filter theo badge nếu có status_filter — tính badge 1 lần/user.
    use crate::models::user::UserStatusBadge;
    let key_to_badge = |k: &str| -> Option<UserStatusBadge> {
        match k {
            "banned" => Some(UserStatusBadge::Banned),
            "new" => Some(UserStatusBadge::New),
            "online" => Some(UserStatusBadge::Online),
            "active" => Some(UserStatusBadge::Active),
            "inactive" => Some(UserStatusBadge::Inactive),
            "dormant" => Some(UserStatusBadge::Dormant),
            _ => None,
        }
    };
    let target_badge = if status_filter.is_empty() {
        None
    } else {
        key_to_badge(&status_filter)
    };
    if let Some(b) = target_badge {
        all_users.retain(|u| u.status_badge_at(now) == b);
    }
    // Tính count cho mỗi chip — phải lấy TẤT CẢ user không filter status
    // để có count đúng. Tránh N+1 bằng cách tính trong 1 pass.
    let mut counts: std::collections::HashMap<UserStatusBadge, i64> =
        std::collections::HashMap::new();
    for u in &all_users {
        *counts.entry(u.status_badge_at(now)).or_insert(0) += 1;
    }
    let badge_keys: [(&str, UserStatusBadge); 6] = [
        ("banned", UserStatusBadge::Banned),
        ("new", UserStatusBadge::New),
        ("online", UserStatusBadge::Online),
        ("active", UserStatusBadge::Active),
        ("inactive", UserStatusBadge::Inactive),
        ("dormant", UserStatusBadge::Dormant),
    ];
    let status_options: Vec<(&'static str, &'static str, i64)> = badge_keys
        .iter()
        .map(|(k, b)| (*k, b.label(), *counts.get(b).unwrap_or(&0)))
        .collect();
    // Paginate thủ công sau khi filter.
    let total = i64::try_from(all_users.len()).unwrap_or(0);
    let users: Vec<_> = all_users
        .into_iter()
        .skip(offset as usize)
        .take(per_page as usize)
        .collect();
    let unread = unread_count(&state, user.id).await;
    // Tránh unused warning cho `total_search` — đã bind để debug/sanity check.
    // Nếu không dùng trong template, ít nhất xuất ra tracing để admin/dev
    // biết quy mô search hiện tại.
    tracing::debug!(?total_search, ?status_filter, "admin/users list rendered");
    Ok(AdminUsersTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        users,
        status_options,
        status_filter,
        search: q.q.unwrap_or_default(),
        total,
        page,
        per_page,
        // total_search không dùng trong template — chỉ là tham chiếu
        // cho debug. Trả về `total` = len của filter slice để pagination
        // đúng số trang thật của filter hiện tại.
    })
}

#[derive(Deserialize)]
pub struct RoleForm {
    pub role: String,
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
    // v2.1.0 — xoá session cache của user này để quyền mới lan truyền ngay
    // (SESSION_CACHE có thể đang giữ bản User với role cũ trong 10s).
    crate::middleware::invalidate_session_cache_for_user(id);
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

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
    // v2.1.0 — ban/unban phải có hiệu lực NGAY với session cache (user bị
    // ban không được tiếp tục lướt web trong cửa sổ TTL 10s).
    crate::middleware::invalidate_session_cache_for_user(id);
    audit(
        &state,
        admin.id,
        "user.ban",
        "user",
        &id.to_string(),
        if banned { "banned" } else { "unbanned" },
    )
    .await;
    // v1.4.0: sau khi ban/unban, fetch lại user để tính badge mới thật (chứ
    // không hardcode "Hoạt động" — user có thể vẫn inactive hoặc new). HTMX
    // swap #ban-{id} → hiển thị đúng badge trạng thái thật sau toggle.
    let updated = UserRepo::find_by_id(&state.db, id).await?;
    let badge_html = match updated {
        Some(u) => {
            use crate::models::user::UserStatusBadge;
            let badge = UserStatusBadge::compute(
                u.is_banned,
                u.created_at,
                u.last_seen_at,
                chrono::Utc::now(),
            );
            format!(
                "<span class='status-badge' style='color:{}'>{}</span>",
                badge.color(),
                badge.label()
            )
        }
        None => {
            // User bị xoá giữa chừng (race) — fallback đơn giản.
            if banned {
                "<span class='status-badge' style='color:#ef4444'>Bị cấm</span>".into()
            } else {
                "<span class='status-badge' style='color:#10b981'>Hoạt động</span>".into()
            }
        }
    };
    Ok(Html(badge_html))
}

// ============================================================
// ADMIN: COMMENTS
// ============================================================
#[derive(Deserialize, Default)]
pub struct AdminCommentsQuery {
    pub page: Option<i64>,
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn delete_comment(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Html<String>> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    // Xoá ở CẢ 2 bảng (game trước, news sau) — trước đây chỉ xoá bảng
    // `comments` nên bình luận TIN TỨC không thể xoá từ admin được.
    let kind = CommentRepo::delete_any(&state.db, id).await?;
    if kind.is_none() {
        return Err(AppError::NotFound("Bình luận không tồn tại".into()));
    }
    audit(
        &state,
        user.id,
        "comment.delete",
        "comment",
        &id.to_string(),
        kind.unwrap_or(""),
    )
    .await;
    Ok(Html(
        "<div class='alert alert-success'>Đã xóa bình luận.</div>".into(),
    ))
}

// ============================================================
// ADMIN: CATEGORIES
// ============================================================
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
        .and_then(|s| Uuid::parse_str(s).ok())
    {
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

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
// ADMIN: NEWS CATEGORIES — v1.4.0
// CRUD riêng cho thể loại tin tức (khác với thể loại game).
// ============================================================
/// GET /admin/news-categories — list tất cả category + count số tin.
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn news_categories(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<AdminNewsCategoriesTemplate> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden("Chỉ quản trị viên tối cao".into()));
    }
    let cats = NewsCategoryRepo::list_all_with_counts(&state.db).await?;
    let cats: Vec<NewsCategoryWithCountView> = cats.into_iter().map(Into::into).collect();
    let unread = unread_count(&state, user.id).await;
    Ok(AdminNewsCategoriesTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        categories: cats,
    })
}

#[derive(Deserialize)]
pub struct NewsCategoryForm {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    /// Có khi form edit gửi kèm id; không có = tạo mới.
    pub id: Option<String>,
    /// v1.4.0 — sort_order để admin sắp xếp category.
    pub sort_order: Option<i32>,
    /// v1.4.0 — is_active checkbox (absent = false do HTML checkbox behaviour).
    pub is_active: Option<String>,
}

/// POST /admin/news-categories/save — tạo hoặc update category.
/// Quy ước giống `save_category` cho game: nếu form có `id` → update,
/// không có → create mới (slug auto-sinh từ name).
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn save_news_category(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Form(form): Form<NewsCategoryForm>,
) -> AppResult<Redirect> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden("Chỉ quản trị viên tối cao".into()));
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
    if icon.chars().count() > 50 {
        return Err(AppError::BadRequest("Icon tối đa 50 ký tự".into()));
    }
    let sort_order = form.sort_order.unwrap_or(0).clamp(0, 9999);
    let is_active = form.is_active.is_some();
    if let Some(id) = form
        .id
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        NewsCategoryRepo::update(
            &state.db,
            id,
            name,
            description,
            icon,
            sort_order,
            is_active,
        )
        .await?;
        audit(
            &state,
            user.id,
            "news_category.update",
            "news_category",
            &id.to_string(),
            name,
        )
        .await;
    } else {
        let slug = slug::slugify(name);
        let id = NewsCategoryRepo::create(&state.db, name, &slug, description, icon).await?;
        audit(
            &state,
            user.id,
            "news_category.create",
            "news_category",
            &id.to_string(),
            name,
        )
        .await;
    }
    Ok(Redirect::to("/admin/news-categories"))
}

/// POST /admin/news-categories/{id}/delete — xoá vĩnh viễn.
/// Khác với `delete_category` (game): không chặn khi còn tin dùng category,
/// vì `news.category` là VARCHAR text (không có FK) → tin cũ giữ giá trị
/// text nhưng không còn category trong DB để match → form select sẽ hiện
/// "— Không phân loại —". Admin cần tự quyết định có an toàn xoá hay không
/// (xem `news_count` trong bảng list).
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn delete_news_category(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Redirect> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden("Chỉ quản trị viên tối cao".into()));
    }
    NewsCategoryRepo::delete(&state.db, id).await?;
    audit(
        &state,
        user.id,
        "news_category.delete",
        "news_category",
        &id.to_string(),
        "",
    )
    .await;
    Ok(Redirect::to("/admin/news-categories"))
}

// ============================================================
// ADMIN: REPOS moderation
// ============================================================
#[derive(Deserialize, Default)]
pub struct AdminReposQuery {
    pub status: Option<String>,
    pub page: Option<i64>,
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
    // click vào notification link. Path relative phải qua sanitize_redirect
    // để chặn control char (CR/LF) chống header injection khi URL được dùng
    // làm Location header.
    let link = form.link.as_deref().unwrap_or("").trim();
    if !link.is_empty() {
        let is_safe = (link.starts_with('/')
            && !link.starts_with("//")
            && !link.bytes().any(|b| b.is_ascii_control()))
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

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn revoke_session(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    jar: axum_extra::extract::CookieJar,
    Path(id): Path<Uuid>,
) -> AppResult<Redirect> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden("Chỉ quản trị viên tối cao".into()));
    }
    // Chặn self-revoke: admin không thể thu hồi chính phiên đang dùng.
    // Trước đây doc hứa nhưng code không check → admin vô tình click
    // "Thu hồi" trên session của mình → bị đá ra /login giữa task.
    // Lấy target_hash TRƯỚC khi delete (sau khi delete row đã mất, hash
    // không còn để xoá session cache).
    let target_hash = SessionRepo::find_token_hash_by_id(&state.db, id).await?;
    if let Some(my_cookie) = jar.get(crate::auth::SESSION_COOKIE) {
        let my_token_hash = crate::auth::hash_token(my_cookie.value());
        if let Some(hash) = &target_hash {
            if *hash == my_token_hash {
                return Err(AppError::BadRequest(
                    "Không thể thu hồi phiên đang dùng — dùng /auth/logout hoặc /auth/logout-all"
                        .into(),
                ));
            }
        }
    }
    let deleted = SessionRepo::delete_by_id(&state.db, id).await?;
    // v2.1.0 — thu hồi phiên phải đá user ra NGAY, không đợi TTL cache 10s.
    if deleted {
        if let Some(hash) = &target_hash {
            crate::middleware::invalidate_session_cache(hash);
        }
    }
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
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
    // Validate note length — đồng bộ với resolve_report (2000 ký tự).
    // Trước đây không check → admin có thể paste payload lớn vào
    // news.review_note (DB TEXT không constraint) → bloat DB + backup.
    if note.chars().count() > 2000 {
        return Err(AppError::BadRequest(
            "Ghi chú từ chối tối đa 2000 ký tự".into(),
        ));
    }
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
