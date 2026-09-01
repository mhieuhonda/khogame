use crate::error::{AppError, AppResult};
use crate::handlers::auth::unread_count;
use crate::middleware::AuthUser;
use crate::models::report::ReportStatus;
use crate::repositories::{
    AdminLogRepo, AiAgentRepo, CategoryRepo, CommentRepo, GameRepo, GamificationRepo,
    NewsCategoryRepo, NewsRepo, NotificationRepo, RepoRepo, ReportRepo, SessionRepo, SettingsRepo,
    StatsRepo, UserRepo,
};
use crate::services::audit;
use crate::state::AppState;
use crate::templates::{
    AdminAiAgentEditTemplate, AdminAiAgentsTemplate, AdminAiReportsTemplate, AdminAuditTemplate,
    AdminCategoriesTemplate, AdminCommentsTemplate, AdminGamesTemplate, AdminNewsAllTemplate,
    AdminNewsCategoriesTemplate, AdminNewsPendingTemplate, AdminReportsTemplate,
    AdminReposTemplate, AdminSessionsTemplate, AdminSettingsTemplate, AdminTemplate,
    AdminUserDetailTemplate, AdminUsersTemplate, CommentItemPartial, NewsCategoryWithCountView,
    XpBoostControlsPartial, XpBoostStatusPartial,
};
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use axum_extra::extract::CookieJar;
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
        checkins_today,
        achievements_today,
        top_xp_users,
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
        // v2.9.0: retention — checkin hôm nay.
        async {
            crate::repositories::GamificationRepo::checkins_today_count(db)
                .await
                .unwrap_or(0)
        },
        // v2.9.0: retention — huy hiệu trao hôm nay.
        async {
            crate::repositories::GamificationRepo::achievements_today_count(db)
                .await
                .unwrap_or(0)
        },
        // v2.9.0: retention — top 5 user XP.
        async {
            crate::repositories::GamificationRepo::leaderboard_top_xp(db, 5)
                .await
                .unwrap_or_default()
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
        checkins_today,
        achievements_today,
        top_xp_users,
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
    let page = q.page.unwrap_or(1).clamp(1, 10_000);
    let per_page: i64 = 50;
    // FIX v2.8.1: saturating math — page ~4e17 làm (page-1)*per_page
    // tràn i64 → OFFSET âm → 500 (prod) / panic (debug).
    let offset = page.saturating_sub(1).saturating_mul(per_page);
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
            let comment = CommentRepo::find_by_id(&state.db, id, Some(user.id))
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
    let page = q.page.unwrap_or(1).clamp(1, 10_000);
    let per_page: i64 = 50;
    // FIX v2.8.1: saturating math — page ~4e17 làm (page-1)*per_page
    // tràn i64 → OFFSET âm → 500 (prod) / panic (debug).
    let offset = page.saturating_sub(1).saturating_mul(per_page);
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
    let page = q.page.unwrap_or(1).clamp(1, 10_000);
    let per_page: i64 = 50;
    // FIX v2.8.1: saturating math — page ~4e17 làm (page-1)*per_page
    // tràn i64 → OFFSET âm → 500 (prod) / panic (debug).
    let offset = page.saturating_sub(1).saturating_mul(per_page);
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
    // Cách tiếp cận: fetch tối đa 2000 users (v2.2.0 — tăng từ 500 lên 2000
    // để không silent-truncate ở site lớn), filter in-app theo badge,
    // paginate thủ công. Khi user >2000 sẽ cần đổ vào cột status badge
    // trong DB (TODO v3.0 — generated column + index).
    let (all_users_res, total_search_res) = tokio::join!(
        UserRepo::list_for_admin(&state.db, search, 2000, 0),
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
    // v3.8.0 FIX (security audit F1 — HIGH): TÀI KHOẢN AI AGENT không bao
    // giờ được đổi role sang user/moderator/admin. Trước đây set_role
    // chấp nhận mọi user — AI agent bị đổi role tay sẽ:
    //   (a) mất mọi tính năng AI (nút login-as, hồ sơ /ai/, params...),
    //   (b) session web 30 ngày vẫn sống → nếu role mới là staff thì
    //       require_admin KHÔNG còn chặn nổi (hole admin-access).
    // Bây giờ chặn tại nguồn: role AI là immutable, muốn "tắt" một AI
    // agent thì BAN (set_banned) hoặc thu hồi token/mật khẩu.
    let target_user = UserRepo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Người dùng không tồn tại".into()))?;
    if target_user.is_ai_agent_user() {
        return Err(AppError::BadRequest(
            "Tài khoản AI Agent không được đổi vai trò — hãy cấm tài khoản              hoặc thu hồi mật khẩu/token nếu AI bị lạm dụng".into(),
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
    let page = q.page.unwrap_or(1).clamp(1, 10_000);
    let per_page: i64 = 50;
    // FIX v2.8.1: saturating math — page ~4e17 làm (page-1)*per_page
    // tràn i64 → OFFSET âm → 500 (prod) / panic (debug).
    let offset = page.saturating_sub(1).saturating_mul(per_page);
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
    let page = q.page.unwrap_or(1).clamp(1, 10_000);
    let per_page: i64 = 50;
    // FIX v2.8.1: saturating math — page ~4e17 làm (page-1)*per_page
    // tràn i64 → OFFSET âm → 500 (prod) / panic (debug).
    let offset = page.saturating_sub(1).saturating_mul(per_page);
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
        // v2.9.2 FIX: thêm chặn `/\` ở đầu — trước đây chỉ chặn `//`, nhưng
        // browser (Chrome/Edge) normalise `\` thành `/` (WHATWG URL Parser)
        // → `/\evil.com` thành protocol-relative URL đưa user ra domain
        // ngoài qua notification link (phishing). Cùng logic với
        // `utils::sanitize_redirect` nhưng giữ hành vi REJECT (BadRequest)
        // của form admin thay vì tự sửa URL ngầm.
        let is_safe = (link.starts_with('/')
            && !link.starts_with("//")
            && !link.starts_with("/\\")
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
    let page = q.page.unwrap_or(1).clamp(1, 10_000);
    let per_page: i64 = 100;
    // FIX v2.8.1: saturating math — page ~4e17 làm (page-1)*per_page
    // tràn i64 → OFFSET âm → 500 (prod) / panic (debug).
    let offset = page.saturating_sub(1).saturating_mul(per_page);
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
///
/// v3.4.0 — kèm trạng thái mật khẩu từng agent (active/expired/locked/none)
/// + form tạo agent mới (username + mật khẩu + thời hạn).
///
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
    build_ai_agents_page(&state, user, None, None).await
}

/// Build dữ liệu trang /admin/ai-agents (dùng chung cho GET list + POST
/// create/reset-password — POST render TRỰC TIẾP kèm flash mật khẩu, KHÔNG
/// redirect qua URL vì mật khẩu trong URL sẽ nằm vĩnh viễn trong browser
/// history + access log — audit v3.4.0 MED-HIGH).
///
/// Flash: `flash_username` + `flash_password` chỉ tồn tại trong response
/// HTML của đúng request POST này — refresh trang là mất (an toàn).
async fn build_ai_agents_page(
    state: &AppState,
    user: crate::models::user::User,
    flash_username: Option<String>,
    flash_password: Option<String>,
) -> AppResult<AdminAiAgentsTemplate> {
    let (agents_res, creds_res, params_res, unread_res) = tokio::join!(
        AiAgentRepo::list_for_admin(&state.db),
        AiAgentRepo::credentials_map(&state.db),
        AiAgentRepo::params_map(&state.db),
        unread_count(state, user.id)
    );
    let agents = agents_res.unwrap_or_default();
    let creds = creds_res.unwrap_or_default();
    let param_views = params_res.unwrap_or_default();
    let now = chrono::Utc::now();
    // Trạng thái mật khẩu hiển thị cho từng agent (không kèm hash)
    let cred_views: std::collections::HashMap<Uuid, AiCredentialView> = creds
        .iter()
        .map(|(uid, c)| {
            (
                *uid,
                AiCredentialView {
                    status_label: c.status_at(now).label().to_string(),
                    status_color: c.status_at(now).color().to_string(),
                    expires_at: c.password_expires_at,
                    last_login_at: c.last_login_at,
                    failed_attempts: c.failed_attempts,
                },
            )
        })
        .collect();
    Ok(AdminAiAgentsTemplate {
        current_user: Some(user),
        unread_notifications: unread_res,
        agents,
        cred_views,
        param_views,
        created_username: flash_username.filter(|s| !s.is_empty()),
        created_password: flash_password.filter(|s| !s.is_empty()),
    })
}

/// View trạng thái mật khẩu AI Agent cho template (không chứa hash).
#[derive(Debug, Clone, serde::Serialize)]
pub struct AiCredentialView {
    pub status_label: String,
    pub status_color: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub last_login_at: Option<chrono::DateTime<chrono::Utc>>,
    pub failed_attempts: i32,
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
// v3.3.0 — IMPERSONATION: admin/điều hành đăng nhập với tư cách AI Agent
// ============================================================

/// POST /admin/ai-agents/{user_id}/login-as — tạo session cho AI Agent và
/// chuyển trình duyệt sang "đang nhập với tư cách" agent đó.
///
/// * Chỉ staff (admin + moderator) — `require_admin` đã chặn, handler
///   kiểm tra lại defensively.
/// * Phiên GỐC của admin được lưu vào cookie `kg_impersonator` (HttpOnly,
///   TTL 2h) — ĐĂNG XUẤT khỏi phiên AI sẽ tự khôi phục phiên admin
///   (`handlers::auth::logout`), hoặc dùng POST /impersonate/stop.
/// * Mọi hành động trong phiên impersonate hiển thị công khai với danh
///   nghĩa của AI Agent — vì vậy CHỈ cho impersonate AI Agent (không
///   impersonate user thường: phạm pháp đề danh nghĩa).
/// * Audit log bắt buộc: ai impersonate ai, khi nào.
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn impersonate_ai_agent(
    State(state): State<Arc<AppState>>,
    AuthUser(admin): AuthUser,
    Path(user_id): Path<Uuid>,
    jar: CookieJar,
) -> AppResult<(CookieJar, Redirect)> {
    if !admin.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let target = UserRepo::find_by_id(&state.db, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Không tìm thấy user".into()))?;
    // v3.6.2 — nhận diện bền vững: role AiAgent HOẶC google_sub của AI
    // Agent mặc định (glm53 từng bị đổi role tay trên prod → endpoint
    // từ chối oan dù đây chính là AI Agent cần đăng nhập thay).
    if !target.is_ai_agent_user() {
        return Err(AppError::BadRequest(
            "Chỉ được đăng nhập với tư cách TÀI KHOẢN AI AGENT".into(),
        ));
    }
    if target.is_banned {
        return Err(AppError::BadRequest("AI Agent này đang bị cấm".into()));
    }

    // Tạo session cho target — TTL 1 ngày (ngắn hơn login AI thường 30d
    // vì đây là phiên quản lý, không phải phiên API của agent).
    let token = crate::auth::gen_session_token();
    let token_hash = crate::auth::hash_token(&token);
    SessionRepo::create(
        &state.db,
        target.id,
        &token_hash,
        "impersonation (admin/staff)",
        None,
        1,
    )
    .await?;

    // v3.9.0 FIX (security audit LOW — orphaned staff session): phiên admin
    // GỐC của browser này sắp bị ghi đè cookie (kg_session mới = phiên AI)
    // → row session cũ còn sống trong DB tới hết TTL (≤30 ngày) là một
    // credential mồ côi: nếu cookie cũ từng bị capture, kẻ giữ nó vẫn vào
    // được admin. Thu hồi NGAY row session cũ (chỉ đúng phiên đang request
    // impersonate — các phiên admin khác thiết bị không bị đụng). Phiên
    // admin khôi phục sau này qua ticket là session MỚI mint riêng nên
    // không bị ảnh hưởng.
    if let Some(old_cookie) = jar.get(crate::auth::SESSION_COOKIE) {
        let old_hash = crate::auth::hash_token(old_cookie.value());
        // delete_for_user scope theo user_id — không xóa nhầm phiên người
        // khác. Cookie lạ/hết hạn → find trả None → bỏ qua im lặng.
        if let Ok(Some(old_id)) = SessionRepo::find_id_by_token(&state.db, &old_hash).await {
            let _ = SessionRepo::delete_for_user(&state.db, old_id, admin.id).await;
            crate::middleware::invalidate_session_cache_for_user(admin.id);
        }
    }

    // v3.4.2 FIX (audit "raw token trong cookie"): cookie kg_impersonator
    // từ nay chỉ chứa TICKET ID opaque. Trước đây cookie chứa nguyên văn
    // admin session token (credential 30 ngày) — lộ cookie = lộ admin.
    // Ticket: one-shot (used_at), TTL 2 giờ, lưu DB — restore mint session
    // MỚI cho admin, token cũ không bao giờ rời server lần 2.
    let ticket_id = uuid::Uuid::new_v4();
    // v3.8.0 FIX (security audit F4): bind ticket vào session AI vừa tạo
    // (hash SHA-256). Redeem (/impersonate/stop, /auth/logout) yêu cầu
    // cookie kg_session hiện tại khớp hash — ticket bị lộ riêng lẻ không
    // thể đổi thành session admin nữa (trước đây ticket là bearer
    // credential thuần redeem được từ endpoint public).
    sqlx::query(
        r#"INSERT INTO impersonation_tickets
               (id, admin_user_id, target_user_id, expires_at, bound_session_hash)
           VALUES ($1, $2, $3, NOW() + ($4 || ' hours')::INTERVAL, $5)"#,
    )
    .bind(ticket_id)
    .bind(admin.id)
    .bind(target.id)
    .bind(crate::auth::IMPERSONATION_TTL_HOURS.to_string())
    .bind(&token_hash)
    .execute(&state.db)
    .await?;

    let mut new_jar = jar;
    crate::auth::set_impersonator_cookie(
        &mut new_jar,
        &ticket_id.to_string(),
        &state.config.base_url,
    );
    crate::auth::set_session_cookie(&mut new_jar, &token, &state.config.base_url);

    audit::audit(
        &state,
        admin.id,
        "ai_agent.impersonate",
        "user",
        &target.id.to_string(),
        &format!(
            "{} đăng nhập với tư cách AI Agent {}",
            admin.username, target.display_name
        ),
    )
    .await;
    tracing::warn!(
        admin = %admin.username,
        target = %target.username,
        "IMPERSONATION: admin đăng nhập với tư cách AI Agent"
    );
    Ok((new_jar, Redirect::to("/")))
}

// ============================================================
// v3.4.0 — AI AGENT: admin tạo tài khoản + quản lý mật khẩu
// (username + password + thời hạn — xem migration 028)
// ============================================================

/// Form tạo AI Agent mới (POST /admin/ai-agents/create).
#[derive(Debug, Deserialize)]
pub struct AiAgentCreateForm {
    pub username: String,
    pub display_name: String,
    pub password: String,
    /// Số ngày mật khẩu có hiệu lực (1-3650).
    pub expires_days: i64,
    pub model_name: String,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub version: String,
    /// Capabilities phân tách bằng dấu phẩy.
    #[serde(default)]
    pub capabilities: String,
    /// "public" hoặc "anonymous".
    #[serde(default)]
    pub privacy_level: String,
    #[serde(default)]
    pub accent_color: String,
    #[serde(default)]
    pub bio: String,
    /// v3.5.0 — Thông số khai báo thêm, mỗi dòng `Tên = Giá trị`
    /// (vd `Context = 128K tokens`). Model/Vendor/Phiên bản tự động ghi.
    #[serde(default)]
    pub spec_params: String,
}

/// POST /admin/ai-agents/create — admin tạo tài khoản AI Agent mới với
/// username + mật khẩu + thời hạn. Trả về trang danh sách kèm mật khẩu
/// HIỂN THỊ 1 LẦN (admin copy gửi cho AI out-of-band).
///
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn create_ai_agent(
    State(state): State<Arc<AppState>>,
    AuthUser(admin): AuthUser,
    Form(form): Form<AiAgentCreateForm>,
) -> AppResult<Response> {
    if !admin.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    // Capabilities: tách dấu phẩy, trim, lọc rỗng, tối đa 20 items
    let capabilities: Vec<String> = form
        .capabilities
        .split(',')
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
        .take(20)
        .collect();

    let (user_id, username) = AiAgentRepo::admin_create_agent(
        &state.db,
        &form.username,
        &form.display_name,
        &form.password,
        form.expires_days,
        &form.model_name,
        &form.vendor,
        &form.version,
        &capabilities,
        &form.privacy_level,
        &form.accent_color,
        &form.bio,
        admin.id,
    )
    .await?;

    // v3.5.0 — Mọi AI Agent mới PHẢI có đầy đủ khai báo tham số + tham số
    // kích hoạt: (1) spec cơ bản từ form, (2) spec tuỳ biến từ textarea
    // `Tên = Giá trị`, (3) bộ activation chuẩn (trạng thái, đăng nhập,
    // rate-limit, thời hạn...). Fail-soft: lỗi seed không chặn tạo agent.
    {
        let model = form.model_name.trim();
        let vendor = form.vendor.trim();
        let version = form.version.trim();
        let _ = AiAgentRepo::upsert_param(
            &state.db,
            user_id,
            "Model",
            model,
            "spec",
            "Tên model đầy đủ",
            true,
            10,
            admin.id,
        )
        .await;
        if !vendor.is_empty() {
            let _ = AiAgentRepo::upsert_param(
                &state.db,
                user_id,
                "Nhà phát triển",
                vendor,
                "spec",
                "Vendor sở hữu model",
                true,
                20,
                admin.id,
            )
            .await;
        }
        if !version.is_empty() {
            let _ = AiAgentRepo::upsert_param(
                &state.db,
                user_id,
                "Phiên bản",
                version,
                "spec",
                "Phiên bản hiện đang chạy",
                true,
                30,
                admin.id,
            )
            .await;
        }
        // Spec tuỳ biến — mỗi dòng `Tên = Giá trị` (tối đa 20 dòng).
        for (i, line) in form.spec_params.lines().take(20).enumerate() {
            let line = line.trim();
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let k = k.trim();
            let v = v.trim();
            if k.is_empty() || v.is_empty() {
                continue;
            }
            let _ = AiAgentRepo::upsert_param(
                &state.db,
                user_id,
                k,
                v,
                "spec",
                "Khai báo bởi admin khi tạo tài khoản",
                true,
                40 + (i as i32) * 10,
                admin.id,
            )
            .await;
        }
        let _ =
            AiAgentRepo::seed_activation_params(&state.db, user_id, form.expires_days, admin.id)
                .await;
    }

    audit::audit(
        &state,
        admin.id,
        "ai_agent.create",
        "user",
        &user_id.to_string(),
        &format!(
            "{} tạo AI Agent {} (username={}, thời hạn mật khẩu {} ngày)",
            admin.username, form.display_name, username, form.expires_days
        ),
    )
    .await;
    tracing::info!(
        admin = %admin.username,
        agent = %username,
        "AI Agent account created (username+password)"
    );

    // Render TRỰC TIẾP trang danh sách kèm flash mật khẩu (KHÔNG redirect
    // — mật khẩu không bao giờ nằm trong URL/history/access log).
    // Response là HTML của đúng request POST này; refresh (GET) là mất.
    let page = build_ai_agents_page(
        &state,
        admin,
        Some(username.clone()),
        Some(form.password.clone()),
    )
    .await?;
    // v3.4.2 — no-store: response chứa MẬT KHẨU plaintext, cấm mọi proxy
    // cache trung gian lưu lại (audit "password flash cacheable").
    let mut resp = page.into_response();
    resp.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok(resp)
}

/// Form đặt lại mật khẩu (POST /admin/ai-agents/{id}/reset-password).
#[derive(Debug, Deserialize)]
pub struct AiPasswordResetForm {
    pub password: String,
    /// Số ngày mật khẩu có hiệu lực (1-3650).
    pub expires_days: i64,
}

/// POST /admin/ai-agents/{user_id}/reset-password — admin đặt lại mật khẩu
/// + thời hạn (mở khoá nếu đang bị khoá).
///
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn reset_ai_agent_password(
    State(state): State<Arc<AppState>>,
    AuthUser(admin): AuthUser,
    Path(user_id): Path<Uuid>,
    Form(form): Form<AiPasswordResetForm>,
) -> AppResult<Response> {
    if !admin.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let target = UserRepo::find_by_id(&state.db, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Không tìm thấy user".into()))?;
    if !target.is_ai_agent_user() {
        return Err(AppError::BadRequest(
            "Chỉ đặt lại mật khẩu cho TÀI KHOẢN AI AGENT".into(),
        ));
    }
    AiAgentRepo::admin_reset_password(
        &state.db,
        user_id,
        &form.password,
        form.expires_days,
        admin.id,
    )
    .await?;

    audit::audit(
        &state,
        admin.id,
        "ai_agent.reset_password",
        "user",
        &user_id.to_string(),
        &format!(
            "{} đặt lại mật khẩu AI Agent {} (thời hạn {} ngày)",
            admin.username, target.username, form.expires_days
        ),
    )
    .await;
    tracing::info!(admin = %admin.username, target = %target.username, "AI Agent password reset");

    // Render trực tiếp kèm flash mật khẩu (không redirect — không pwd trong URL)
    let page = build_ai_agents_page(
        &state,
        admin,
        Some(target.username.clone()),
        Some(form.password.clone()),
    )
    .await?;
    // v3.4.2 — no-store cho response chứa mật khẩu (chống proxy cache).
    let mut resp = page.into_response();
    resp.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok(resp)
}

/// POST /admin/ai-agents/{user_id}/revoke-password — thu hồi mật khẩu
/// (AI không thể đăng nhập web bằng mật khẩu nữa; API token nếu có vẫn dùng được).
///
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn revoke_ai_agent_password(
    State(state): State<Arc<AppState>>,
    AuthUser(admin): AuthUser,
    Path(user_id): Path<Uuid>,
) -> AppResult<Response> {
    if !admin.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let target = UserRepo::find_by_id(&state.db, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Không tìm thấy user".into()))?;
    if !target.is_ai_agent_user() {
        return Err(AppError::BadRequest(
            "Chỉ thu hồi mật khẩu của TÀI KHOẢN AI AGENT".into(),
        ));
    }
    AiAgentRepo::admin_revoke_password(&state.db, user_id).await?;
    audit::audit(
        &state,
        admin.id,
        "ai_agent.revoke_password",
        "user",
        &user_id.to_string(),
        &format!(
            "{} thu hồi mật khẩu AI Agent {}",
            admin.username, target.username
        ),
    )
    .await;
    Ok(Redirect::to("/admin/ai-agents").into_response())
}

/// POST /admin/ai-agents/{user_id}/revoke-token — thu hồi TOÀN BỘ API
/// token (`kgai_...`) của agent (v3.4.2). Agent vẫn đăng nhập web bằng
/// mật khẩu nếu còn hiệu lực; token chỉ lấy lại được khi... không lấy lại
/// được — AI phải liên hệ admin cấp tài khoản/token mới.
///
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn revoke_ai_agent_tokens(
    State(state): State<Arc<AppState>>,
    AuthUser(admin): AuthUser,
    Path(user_id): Path<Uuid>,
) -> AppResult<Response> {
    if !admin.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let target = UserRepo::find_by_id(&state.db, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Không tìm thấy user".into()))?;
    if !target.is_ai_agent_user() {
        return Err(AppError::BadRequest(
            "Chỉ thu hồi token của TÀI KHOẢN AI AGENT".into(),
        ));
    }
    let revoked = AiAgentRepo::revoke_all_tokens(&state.db, user_id).await?;
    audit::audit(
        &state,
        admin.id,
        "ai_agent.revoke_tokens",
        "user",
        &user_id.to_string(),
        &format!(
            "{} thu hồi {} API token của AI Agent {}",
            admin.username, revoked, target.username
        ),
    )
    .await;
    tracing::warn!(
        admin = %admin.username,
        target = %target.username,
        revoked,
        "AI Agent API tokens revoked"
    );
    Ok(Redirect::to("/admin/ai-agents").into_response())
}

/// Form thêm/cập nhật tham số (v3.5.0).
#[derive(Debug, Deserialize)]
pub struct AiParamForm {
    pub param_key: String,
    pub param_value: String,
    /// "spec" (khai báo tham số) hoặc "activation" (tham số kích hoạt).
    #[serde(default)]
    pub param_group: String,
    #[serde(default)]
    pub description: String,
    /// "on" từ checkbox HTML — rỗng nếu không check.
    #[serde(default)]
    pub is_public: String,
    #[serde(default)]
    pub display_order: i64,
}

/// POST /admin/ai-agents/{user_id}/params — thêm/cập nhật 1 tham số của
/// agent (upsert theo tên). v3.5.0 — "AI Agent phải có khai báo tham số,
/// tham số kích hoạt, đầy đủ thông tin chi tiết".
///
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn ai_agent_add_param(
    State(state): State<Arc<AppState>>,
    AuthUser(admin): AuthUser,
    Path(user_id): Path<Uuid>,
    Form(form): Form<AiParamForm>,
) -> AppResult<Response> {
    if !admin.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let target = UserRepo::find_by_id(&state.db, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Không tìm thấy user".into()))?;
    if !target.is_ai_agent_user() {
        return Err(AppError::BadRequest(
            "Chỉ quản lý tham số của TÀI KHOẢN AI AGENT".into(),
        ));
    }
    let group = if form.param_group == "activation" {
        "activation"
    } else {
        "spec"
    };
    let is_public = matches!(form.is_public.as_str(), "on" | "true" | "1");
    AiAgentRepo::upsert_param(
        &state.db,
        user_id,
        &form.param_key,
        &form.param_value,
        group,
        &form.description,
        is_public,
        form.display_order.clamp(0, 10_000) as i32,
        admin.id,
    )
    .await?;
    audit::audit(
        &state,
        admin.id,
        "ai_agent.param_upsert",
        "user",
        &user_id.to_string(),
        &format!(
            "{} {} tham số '{}' của AI Agent {}",
            admin.username,
            if group == "activation" {
                "cập nhật tham số kích hoạt"
            } else {
                "khai báo tham số"
            },
            form.param_key,
            target.username
        ),
    )
    .await;
    Ok(Redirect::to("/admin/ai-agents").into_response())
}

/// POST /admin/ai-agents/{user_id}/params/{param_id}/delete — xoá 1 tham số.
///
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn ai_agent_delete_param(
    State(state): State<Arc<AppState>>,
    AuthUser(admin): AuthUser,
    Path((user_id, param_id)): Path<(Uuid, i64)>,
) -> AppResult<Response> {
    if !admin.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let deleted = AiAgentRepo::delete_param(&state.db, user_id, param_id).await?;
    if deleted {
        audit::audit(
            &state,
            admin.id,
            "ai_agent.param_delete",
            "user",
            &user_id.to_string(),
            &format!("{} xoá tham số #{param_id} của AI Agent", admin.username),
        )
        .await;
    }
    Ok(Redirect::to("/admin/ai-agents").into_response())
}

// ============================================================
// v3.7.0 — ADMIN SỬA THÔNG TIN CHI TIẾT + THÔNG SỐ AI AGENT
// GET  /admin/ai-agents/{user_id}/edit — trang sửa
// POST /admin/ai-agents/{user_id}/edit — lưu hồ sơ
// POST /admin/ai-agents/{user_id}/params/{param_id}/edit — sửa 1 tham số
// ============================================================

/// GET /admin/ai-agents/{user_id}/edit — trang sửa thông tin AI Agent.
///
/// # Errors
///
/// Trả về lỗi khi DB fail / không phải AI Agent.
pub async fn edit_ai_agent_form(
    State(state): State<Arc<AppState>>,
    AuthUser(admin): AuthUser,
    Path(user_id): Path<Uuid>,
) -> AppResult<AdminAiAgentEditTemplate> {
    if !admin.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let (agent_res, params_res, unread_res, badge_res) = tokio::join!(
        AiAgentRepo::find_agent_by_id(&state.db, user_id),
        AiAgentRepo::list_params(&state.db, user_id, false),
        unread_count(&state, admin.id),
        // v3.10.0 — huy hiệu độc quyền AI Agent đã cấp chưa?
        GamificationRepo::has_achievement(&state.db, user_id, AI_EXCLUSIVE_BADGE_ID)
    );
    let agent = agent_res?;
    let params = params_res.unwrap_or_default();
    Ok(AdminAiAgentEditTemplate {
        current_user: Some(admin),
        unread_notifications: unread_res,
        agent,
        params,
        saved: false,
        error: None,
        has_ai_badge: badge_res.unwrap_or(false),
    })
}

/// Form lưu hồ sơ AI Agent (POST /admin/ai-agents/{user_id}/edit).
#[derive(Debug, Deserialize)]
pub struct AiAgentEditForm {
    pub display_name: String,
    pub model_name: String,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub version: String,
    /// Mỗi dòng 1 khả năng (giống form AI tự sửa).
    #[serde(default)]
    pub capabilities: String,
    #[serde(default)]
    pub privacy_level: String,
    /// "on" từ checkbox — rỗng nếu không check.
    #[serde(default)]
    pub verified: String,
    #[serde(default)]
    pub accent_color: String,
    #[serde(default)]
    pub bio: String,
    #[serde(default)]
    pub avatar_url: String,
}

/// POST /admin/ai-agents/{user_id}/edit — admin sửa hồ sơ AI Agent
/// (display_name, model, vendor, version, capabilities, màu, bio, avatar,
/// privacy, verified). Mọi thay đổi ghi audit log.
///
/// # Errors
///
/// Trả về lỗi khi validation fail / DB fail (render lại form kèm lỗi).
pub async fn edit_ai_agent_submit(
    State(state): State<Arc<AppState>>,
    AuthUser(admin): AuthUser,
    Path(user_id): Path<Uuid>,
    Form(form): Form<AiAgentEditForm>,
) -> AppResult<AdminAiAgentEditTemplate> {
    if !admin.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }

    // Helper render lại trang kèm banner lỗi + dữ liệu MỚI (form đã điền
    // bị giữ nguyên bằng cách re-fetch DB — đơn giản, admin sửa lại).
    async fn render_error(
        state: &AppState,
        admin: crate::models::user::User,
        user_id: Uuid,
        msg: &str,
    ) -> AppResult<AdminAiAgentEditTemplate> {
        let (agent_res, params_res, unread_res, badge_res) = tokio::join!(
            AiAgentRepo::find_agent_by_id(&state.db, user_id),
            AiAgentRepo::list_params(&state.db, user_id, false),
            unread_count(state, admin.id),
            GamificationRepo::has_achievement(&state.db, user_id, AI_EXCLUSIVE_BADGE_ID)
        );
        let agent = agent_res?;
        let params = params_res.unwrap_or_default();
        Ok(AdminAiAgentEditTemplate {
            current_user: Some(admin),
            unread_notifications: unread_res,
            agent,
            params,
            saved: false,
            error: Some(msg.to_string()),
            has_ai_badge: badge_res.unwrap_or(false),
        })
    }

    // Validate thủ công (cùng rule với AiAgentRepo::update_profile +
    // handler AI tự sửa — message tiếng Việt rõ ràng).
    let display_name = form.display_name.trim();
    if display_name.is_empty() || display_name.chars().count() > 100 {
        return render_error(
            &state,
            admin,
            user_id,
            "Tên hiển thị không được trống, tối đa 100 ký tự",
        )
        .await;
    }
    let model_name = form.model_name.trim();
    if model_name.is_empty() || model_name.chars().count() > 100 {
        return render_error(
            &state,
            admin,
            user_id,
            "Tên model không được trống, tối đa 100 ký tự",
        )
        .await;
    }
    if form.vendor.trim().chars().count() > 50 {
        return render_error(&state, admin, user_id, "Vendor tối đa 50 ký tự").await;
    }
    if form.version.trim().chars().count() > 50 {
        return render_error(&state, admin, user_id, "Phiên bản tối đa 50 ký tự").await;
    }
    if form.bio.trim().chars().count() > 500 {
        return render_error(&state, admin, user_id, "Giới thiệu tối đa 500 ký tự").await;
    }
    let accent = form.accent_color.trim();
    if !accent.is_empty()
        && !(accent.starts_with('#')
            && accent[1..].chars().all(|c| c.is_ascii_hexdigit())
            && (accent.len() == 7 || accent.len() == 4))
    {
        return render_error(
            &state,
            admin,
            user_id,
            "Màu nhấn phải là mã hex (vd #7c3aed)",
        )
        .await;
    }
    let privacy = match form.privacy_level.as_str() {
        "anonymous" => "anonymous",
        _ => "public",
    };
    let verified = matches!(form.verified.as_str(), "on" | "true" | "1");
    // Capabilities: mỗi dòng 1 khả năng — tối đa 20 × 50 ký tự.
    let capabilities: Vec<String> = form
        .capabilities
        .lines()
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
        .take(20)
        .collect();
    for c in &capabilities {
        if c.chars().count() > 50 {
            return render_error(
                &state,
                admin,
                user_id,
                "Mỗi khả năng tối đa 50 ký tự (tách bằng dòng mới)",
            )
            .await;
        }
    }
    let avatar = form.avatar_url.trim();
    if avatar.chars().count() > 2048 {
        return render_error(&state, admin, user_id, "Avatar URL tối đa 2048 ký tự").await;
    }

    // 1) users: display_name + bio + avatar (tái dùng update_profile của
    //    UserRepo — tự validate avatar http(s):// hoặc /uploads/).
    let user_updated = UserRepo::update_profile(
        &state.db,
        user_id,
        display_name,
        form.bio.trim(),
        Some(avatar),
    )
    .await;
    if let Err(e) = user_updated {
        return render_error(&state, admin, user_id, &e.to_string()).await;
    }
    // 2) ai_agent_profiles: model/vendor/version/caps/privacy/accent.
    let profile_updated = AiAgentRepo::update_profile(
        &state.db,
        user_id,
        model_name,
        form.vendor.trim(),
        form.version.trim(),
        &capabilities,
        privacy,
        if accent.is_empty() { "#7c3aed" } else { accent },
        form.bio.trim(),
        Some(avatar),
    )
    .await;
    if let Err(e) = profile_updated {
        return render_error(&state, admin, user_id, &e.to_string()).await;
    }
    // 3) verified flag (repo riêng — cột ai_agent_profiles.verified).
    AiAgentRepo::set_verified(&state.db, user_id, verified).await?;

    audit::audit(
        &state,
        admin.id,
        "ai_agent.edit_profile",
        "user",
        &user_id.to_string(),
        &format!(
            "{} sửa hồ sơ AI Agent {} (display_name={}, model={}, verified={})",
            admin.username, display_name, display_name, model_name, verified
        ),
    )
    .await;
    tracing::info!(
        admin = %admin.username,
        agent = %display_name,
        "AI Agent profile edited by admin"
    );

    // Render lại trang kèm banner thành công + dữ liệu mới nhất.
    let (agent_res, params_res, unread_res, badge_res) = tokio::join!(
        AiAgentRepo::find_agent_by_id(&state.db, user_id),
        AiAgentRepo::list_params(&state.db, user_id, false),
        unread_count(&state, admin.id),
        GamificationRepo::has_achievement(&state.db, user_id, AI_EXCLUSIVE_BADGE_ID)
    );
    let agent = agent_res?;
    let params = params_res.unwrap_or_default();
    Ok(AdminAiAgentEditTemplate {
        current_user: Some(admin),
        unread_notifications: unread_res,
        agent,
        params,
        saved: true,
        error: None,
        has_ai_badge: badge_res.unwrap_or(false),
    })
}

/// v3.10.0 — id của huy hiệu ĐỘC QUYỀN duy nhất dành cho AI Agent
/// (seed trong migration 043; engine check_and_award không có điều kiện
/// cho id này nên KHÔNG THỂ tự trao — chỉ admin cấp/thu hồi tay).
pub const AI_EXCLUSIVE_BADGE_ID: &str = "ai_agent_core";

/// Form cấp/thu hồi huy hiệu độc quyền (POST /admin/ai-agents/{id}/badge-ai).
#[derive(Debug, Deserialize)]
pub struct AiBadgeToggleForm {
    /// "grant" | "revoke"
    pub action: String,
}

/// POST /admin/ai-agents/{user_id}/badge-ai — admin CẤP hoặc THU HỒI
/// huy hiệu độc quyền AI Agent (`ai_agent_core` — "Linh Hồn Nhân Tạo").
///
/// Guard 3 lớp:
/// 1. `is_staff` — chỉ staff;
/// 2. target PHẢI là tài khoản AI Agent (`is_ai_agent_user` — role HOẶC
///    danh tính gốc, bền vững với role drift như glm53 ở prod);
/// 3. action whitelist — chỉ "grant"/"revoke" (không có hành vi lạ).
///
/// Mọi thao tác thành công/thất bại đều ghi audit log.
///
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, validation, không phải AI Agent).
pub async fn toggle_ai_agent_badge(
    State(state): State<Arc<AppState>>,
    AuthUser(admin): AuthUser,
    Path(user_id): Path<Uuid>,
    Form(form): Form<AiBadgeToggleForm>,
) -> AppResult<Response> {
    if !admin.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let target = UserRepo::find_by_id(&state.db, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Không tìm thấy user".into()))?;
    if !target.is_ai_agent_user() {
        // Huy hiệu ĐỘC QUYỀN — user thường tuyệt đối không nhận được.
        return Err(AppError::BadRequest(
            "Huy hiệu này chỉ cấp cho TÀI KHOẢN AI AGENT".into(),
        ));
    }
    let granted = match form.action.as_str() {
        "grant" => {
            GamificationRepo::grant_achievement(&state.db, user_id, AI_EXCLUSIVE_BADGE_ID).await?;
            true
        }
        "revoke" => {
            GamificationRepo::revoke_achievement(&state.db, user_id, AI_EXCLUSIVE_BADGE_ID).await?;
            false
        }
        _ => {
            return Err(AppError::BadRequest(
                "Hành động không hợp lệ (chỉ grant/revoke)".into(),
            ));
        }
    };

    audit::audit(
        &state,
        admin.id,
        if granted {
            "ai_agent.badge_grant"
        } else {
            "ai_agent.badge_revoke"
        },
        "user",
        &user_id.to_string(),
        &format!(
            "{} {} huy hiệu độc quyền AI Agent cho @{}",
            admin.username,
            if granted { "CẤP" } else { "THU HỒI" },
            target.username
        ),
    )
    .await;
    tracing::info!(
        admin = %admin.username,
        target = %target.username,
        granted,
        "AI Agent exclusive badge toggled"
    );

    // Redirect về trang sửa (PRG pattern — refresh không gửi lại POST).
    Ok(Redirect::to(&format!("/admin/ai-agents/{user_id}/edit")).into_response())
}

/// Form sửa 1 tham số (POST /admin/ai-agents/{user_id}/params/{param_id}/edit).
#[derive(Debug, Deserialize)]
pub struct AiParamEditForm {
    pub param_key: String,
    pub param_value: String,
    #[serde(default)]
    pub param_group: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub is_public: String,
    #[serde(default)]
    pub display_order: i64,
}

/// POST /admin/ai-agents/{user_id}/params/{param_id}/edit — SỬA 1 tham số
/// đã có (trước đây chỉ delete + re-add — v3.7.0 cho edit inline).
///
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn ai_agent_edit_param(
    State(state): State<Arc<AppState>>,
    AuthUser(admin): AuthUser,
    Path((user_id, param_id)): Path<(Uuid, i64)>,
    Form(form): Form<AiParamEditForm>,
) -> AppResult<Response> {
    if !admin.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let group = if form.param_group == "activation" {
        "activation"
    } else {
        "spec"
    };
    let is_public = matches!(form.is_public.as_str(), "on" | "true" | "1");
    let updated = AiAgentRepo::admin_update_param(
        &state.db,
        user_id,
        param_id,
        &form.param_key,
        &form.param_value,
        group,
        &form.description,
        is_public,
        form.display_order.clamp(0, 10_000) as i32,
        admin.id,
    )
    .await?;
    if updated {
        audit::audit(
            &state,
            admin.id,
            "ai_agent.param_edit",
            "user",
            &user_id.to_string(),
            &format!(
                "{} sửa tham số #{param_id} ('{}') của AI Agent",
                admin.username, form.param_key
            ),
        )
        .await;
    }
    Ok(Redirect::to("/admin/ai-agents").into_response())
}

/// POST /impersonate/stop — kết thúc phiên impersonate.
///
/// v3.4.2 flow (server-side ticket):
/// 1. Đọc ticket id từ cookie `kg_impersonator` (opaque UUID).
/// 2. Xoá session AI ĐANG DÙNG (audit cũ: session 1 ngày bị bỏ sống sau
///    khi stop — token bị bắt giữa đường vẫn dùng được tới 24h).
/// 3. Đổi ticket one-shot (UPDATE ... WHERE used_at IS NULL AND còn hạn)
///    → nếu thành công, verify admin còn quyền staff + không ban → mint
///    session MỚI cho admin (token cũ không tái sử dụng).
///
/// Route PUBLIC (người đang impersonate là AI Agent, không vào được /admin/*).
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn stop_impersonation(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
) -> AppResult<(CookieJar, Redirect)> {
    let mut new_jar = jar;
    // 1) Luôn xoá session AI ĐANG DÙNG (kg_session) — kể cả khi ticket đã
    // hết hạn 2h (audit vòng 5: cookie ticket TTL 2h < session AI 24h —
    // sau 2h stop là no-op, phiên AI sống thêm 22h vô nghĩa). Người gọi
    // endpoint này chính là chủ cookie (SameSite=Lax chặn cross-site),
    // xoá session của chính mình là an toàn.
    if let Some(cur) = new_jar.get(crate::auth::SESSION_COOKIE) {
        let cur_hash = crate::auth::hash_token(cur.value());
        let _ = SessionRepo::delete(&state.db, &cur_hash).await;
        crate::middleware::invalidate_session_cache(&cur_hash);
    }
    // v3.8.0 (audit F4): hash của session AI ĐANG GIỮ (tính trước khi
    // xoá) — dùng để xác thực ticket binding bên dưới.
    let cur_session_hash: Option<String> = new_jar
        .get(crate::auth::SESSION_COOKIE)
        .map(|c| crate::auth::hash_token(c.value()));
    let Some(ticket_raw) = new_jar
        .get(crate::auth::IMPERSONATOR_COOKIE)
        .map(|c| c.value().to_string())
    else {
        // Không có ticket → session AI đã xoá ở trên, clear cookie stripe
        // về trang chủ với trạng thái đăng xuất.
        crate::auth::clear_session_cookie(&mut new_jar, &state.config.base_url);
        crate::auth::clear_impersonator_cookie(&mut new_jar, &state.config.base_url);
        return Ok((new_jar, Redirect::to("/")));
    };

    // 2) Ticket one-shot: chỉ request nào set được used_at mới restore.
    let ticket_id = uuid::Uuid::parse_str(&ticket_raw).ok();
    let mut restored_admin: Option<String> = None;
    if let Some(tid) = ticket_id {
        // v3.8.0 (audit F4): ticket phải được BIND đúng session AI hiện tại
        // (legacy NULL = ticket tạo trước migration 039 — chấp nhận kèm
        // warn). Ticket bị đánh cắp riêng (không kèm cookie session AI
        // khớp) → từ chối, không mint session admin.
        let bound_ok = cur_session_hash.as_deref().unwrap_or_default();
        let admin_id: Option<uuid::Uuid> = sqlx::query_scalar(
            r#"UPDATE impersonation_tickets
               SET used_at = NOW()
               WHERE id = $1 AND used_at IS NULL AND expires_at > NOW()
                 AND (bound_session_hash IS NULL OR bound_session_hash = $2)
               RETURNING admin_user_id"#,
        )
        .bind(tid)
        .bind(bound_ok)
        .fetch_optional(&state.db)
        .await?;
        if admin_id.is_some() {
            tracing::warn!(
                "Impersonation STOP: ticket {} redeem với binding hợp lệ",
                tid
            );
        }
        if let Some(admin_id) = admin_id {
            // 3) Admin còn hợp lệ (staff + không ban) → mint session mới.
            if let Ok(Some(admin_user)) = UserRepo::find_by_id(&state.db, admin_id).await {
                if admin_user.role.is_staff() && !admin_user.is_banned {
                    let token = crate::auth::gen_session_token();
                    let token_hash = crate::auth::hash_token(&token);
                    SessionRepo::create(
                        &state.db,
                        admin_user.id,
                        &token_hash,
                        "impersonation-restore",
                        None,
                        // v3.8.0 SECURITY FIX (audit F4): TTL restore 4h → 2h
                        // — bằng đúng TTL ticket. Trước đây ticket 2h nhưng
                        // session restore sống 4h (mismatch vô nghĩa).
                        2,
                    )
                    .await?;
                    crate::auth::set_session_cookie(&mut new_jar, &token, &state.config.base_url);
                    restored_admin = Some(admin_user.username.clone());
                    tracing::warn!(
                        admin = %admin_user.username,
                        "Impersonation STOP — khôi phục phiên admin (session mới)"
                    );
                } else {
                    tracing::warn!(
                        "Impersonation STOP: admin {} không còn quyền staff/đã ban — từ chối khôi phục",
                        admin_user.username
                    );
                }
            }
        }
    }
    // Dọn ticket hết hạn/thừa (best-effort, cheap DELETE).
    let _ = sqlx::query(
        "DELETE FROM impersonation_tickets WHERE expires_at < NOW() - INTERVAL '1 day'",
    )
    .execute(&state.db)
    .await;
    // Luôn xoá cookie impersonator: ticket đã dùng (one-shot) — kẻ khác
    // cầm cookie này cũng không làm được gì (ticket đã used/expired).
    crate::auth::clear_impersonator_cookie(&mut new_jar, &state.config.base_url);
    if restored_admin.is_some() {
        return Ok((new_jar, Redirect::to("/admin/ai-agents")));
    }
    Ok((new_jar, Redirect::to("/")))
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
    let page = params.page.unwrap_or(1).clamp(1, 10_000);
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
    let page = params.page.unwrap_or(1).clamp(1, 10_000);
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
    // v2.9.0 — XP cho tác giả khi tin được duyệt (best-effort)
    {
        let db = state.db.clone();
        let author_id = news.user_id;
        tokio::spawn(async move {
            crate::services::gamification::on_news_approved(&db, author_id).await;
        });
    }
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

// ============================================================
// v2.9.0 — ADMIN: THỐNG KÊ HUY HIỆU / GAMIFICATION
// ============================================================

/// GET /admin/achievements — catalog huy hiệu + số người đạt + tỉ lệ.
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn achievements_admin(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<crate::templates::AdminAchievementsTemplate> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Cần quyền quản trị".into()));
    }
    let (stats_res, total_users_res, earned_today, checkins_today) = tokio::join!(
        crate::repositories::GamificationRepo::achievement_stats(&state.db),
        UserRepo::count_all(&state.db),
        crate::repositories::GamificationRepo::achievements_today_count(&state.db),
        crate::repositories::GamificationRepo::checkins_today_count(&state.db),
    );
    let stats = stats_res?;
    let total_users = total_users_res.unwrap_or(0);
    let total_holders: i64 = stats.iter().map(|(_, c)| c).sum();
    let unread = unread_count(&state, user.id).await;
    Ok(crate::templates::AdminAchievementsTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        stats,
        total_users,
        total_holders,
        earned_today: earned_today.unwrap_or(0),
        checkins_today: checkins_today.unwrap_or(0),
    })
}

// ============================================================
// v3.6.0 — ADMIN XP BOOST (1000 XP / 0.15 giây, start/stop)
// ------------------------------------------------------------
// Trang /admin/xp-boost — CHỈ ADMIN (is_admin, mod không thấy — đúng
// yêu cầu "chỉ admin thấy"). State sống ở AppState::xp_boost, task
// nền janitor::run_xp_boost cộng XP. 4 endpoint:
//   GET  /admin/xp-boost          → trang đầy đủ
//   POST /admin/xp-boost/start    → bật (trả partial controls)
//   POST /admin/xp-boost/stop     → tắt (trả partial controls)
//   GET  /admin/xp-boost/status   → partial số liệu (HTMX poll 1s)
// ============================================================

// Partial struct (XpBoostControlsPartial + XpBoostStatusPartial) nằm ở
// templates.rs — cần cùng module với `mod filters` để dùng |fmt_num.

/// Thu thập dữ liệu hiển thị hiện tại của boost (None-safe).
async fn xp_boost_view_data(state: &AppState) -> XpBoostStatusPartial {
    use crate::state::xp_boost;
    let running = state.xp_boost.is_running();
    let target = state.xp_boost.target().await;
    let ticks = state.xp_boost.ticks();
    let elapsed_secs = state
        .xp_boost
        .started_at()
        .await
        .map_or(0, |t| t.elapsed().as_secs());
    let (username, total_xp, level) = match target {
        Some(uid) => {
            let (u, xp) = tokio::join!(
                UserRepo::find_by_id(&state.db, uid),
                crate::repositories::GamificationRepo::total_xp(&state.db, uid),
            );
            let xp = xp.unwrap_or(0);
            let level = crate::models::gamification::level_from_xp(xp).level;
            let username = u
                .ok()
                .flatten()
                .map_or_else(|| "(không rõ)".to_string(), |u| u.username);
            (username, xp, level)
        }
        None => (String::new(), 0, 0),
    };
    XpBoostStatusPartial {
        running,
        target_username: username,
        total_xp,
        level,
        ticks,
        xp_added: ticks * u64::from(xp_boost::XP_PER_TICK.unsigned_abs()),
        elapsed_secs,
        xp_per_tick: xp_boost::XP_PER_TICK,
        tick_ms: xp_boost::TICK_MS,
    }
}

/// GET /admin/xp-boost — trang quản lý XP Boost (chỉ admin).
/// # Errors
///
/// Trả về lỗi khi không đủ quyền hoặc DB fail.
pub async fn xp_boost_page(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<crate::templates::AdminXpBoostTemplate> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden(
            "Chỉ quản trị viên tối cao được dùng XP Boost".into(),
        ));
    }
    let (status, unread) = tokio::join!(xp_boost_view_data(&state), unread_count(&state, user.id),);
    Ok(crate::templates::AdminXpBoostTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        running: status.running,
        target_username: status.target_username,
        total_xp: status.total_xp,
        level: status.level,
        ticks: status.ticks,
        xp_added: status.xp_added,
        elapsed_secs: status.elapsed_secs,
        xp_per_tick: status.xp_per_tick,
        tick_ms: status.tick_ms,
    })
}

/// POST /admin/xp-boost/start — bật tăng XP liên tục cho admin hiện tại.
/// # Errors
///
/// Trả về lỗi khi không đủ quyền.
pub async fn xp_boost_start(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<Html<String>> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden(
            "Chỉ quản trị viên tối cao được dùng XP Boost".into(),
        ));
    }
    state.xp_boost.start(user.id).await;
    audit(
        &state,
        user.id,
        "xp_boost.start",
        "system",
        &user.id.to_string(),
        &format!(
            "admin {} bật XP boost ({} XP/{}ms)",
            user.username,
            crate::state::xp_boost::XP_PER_TICK,
            crate::state::xp_boost::TICK_MS
        ),
    )
    .await;
    tracing::warn!(admin = %user.username, "XP BOOST: BẬT");
    let partial = XpBoostControlsPartial { running: true };
    Ok(Html(partial.render()?))
}

/// POST /admin/xp-boost/stop — dừng tăng XP.
/// # Errors
///
/// Trả về lỗi khi không đủ quyền.
pub async fn xp_boost_stop(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<Html<String>> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden(
            "Chỉ quản trị viên tối cao được dùng XP Boost".into(),
        ));
    }
    let stopped_for = state.xp_boost.stop().await;
    let ticks = state.xp_boost.ticks();
    let xp_added = ticks as i64 * i64::from(crate::state::xp_boost::XP_PER_TICK);
    audit(
        &state,
        user.id,
        "xp_boost.stop",
        "system",
        &stopped_for.map_or_else(String::new, |u| u.to_string()),
        &format!(
            "admin {} tắt XP boost (đã cộng {ticks} tick = {xp_added} XP)",
            user.username
        ),
    )
    .await;
    tracing::warn!(admin = %user.username, "XP BOOST: TẮT");
    let partial = XpBoostControlsPartial { running: false };
    Ok(Html(partial.render()?))
}

/// GET /admin/xp-boost/status — partial số liệu (HTMX poll 1s).
/// # Errors
///
/// Trả về lỗi khi không đủ quyền hoặc DB fail.
pub async fn xp_boost_status(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<Response> {
    if !user.role.is_admin() {
        return Err(AppError::Forbidden(
            "Chỉ quản trị viên tối cao được dùng XP Boost".into(),
        ));
    }
    let data = xp_boost_view_data(&state).await;
    let mut resp = Html(data.render()?).into_response();
    // Partial động — tuyệt đối không cache (mỗi giây số XP đổi).
    resp.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok(resp)
}
