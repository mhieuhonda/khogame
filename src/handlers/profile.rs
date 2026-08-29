use crate::error::{AppError, AppResult};
use crate::handlers::auth::unread_count;
use crate::middleware::{AuthUser, CurrentUser};
use crate::models::{SocialLinks, PLATFORMS};
use crate::repositories::{
    AiAgentRepo, CollectionRepo, GameRepo, GamificationRepo, InteractionRepo, UserRepo,
};
use crate::state::AppState;
use crate::templates::{BookmarksTemplate, EditProfileTemplate, ProfileTemplate};
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect};
use axum::Form;
use serde::Deserialize;
use std::sync::Arc;

// ============= View profile =============
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn show_profile(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Path(username): Path<String>,
) -> AppResult<ProfileTemplate> {
    let user = UserRepo::find_by_username(&state.db, &username)
        .await?
        .ok_or_else(|| AppError::NotFound("Người dùng không tồn tại".into()))?;
    // Ẩn hồ sơ user bị ban khỏi HTML (API /api/v1/users đã chặn từ trước —
    // thiếu nhất quán giữa 2 giao diện của cùng dữ liệu).
    if user.is_banned {
        return Err(AppError::NotFound("Người dùng không tồn tại".into()));
    }
    let is_self = current_user.as_ref().is_some_and(|u| u.id == user.id);
    // v2.7.0 — social_links là query thứ 7 chạy SONG SONG trong cùng
    // wave (không tăng round-trip tuần tự).
    let (
        stats_res,
        games_res,
        following_res,
        prefs_res,
        ai_profile_res,
        socials_res,
        level_res,
        streak_res,
        ach_res,
        activity_res,
        collections_res,
        unread_res,
    ) = tokio::join!(
        UserRepo::stats(&state.db, user.id),
        GameRepo::by_user(&state.db, user.id, 24, 0),
        async {
            match current_user.as_ref() {
                Some(cu) if !is_self => InteractionRepo::is_following(&state.db, cu.id, user.id)
                    .await
                    .unwrap_or(false),
                _ => false,
            }
        },
        UserRepo::get_preferences(&state.db, user.id),
        async {
            if user.role.is_ai_agent() {
                AiAgentRepo::find_profile_by_user_id(&state.db, user.id)
                    .await
                    .ok()
                    .flatten()
            } else {
                None
            }
        },
        async {
            // Lỗi social links KHÔNG được làm chết cả trang hồ sơ —
            // fail-open thành rỗng (link chỉ là tính năng phụ, mất
            // tooltip/icon vẫn tốt hơn 500 cả trang).
            UserRepo::social_links(&state.db, user.id)
                .await
                .unwrap_or_default()
        },
        // v2.9.0 — gamification block (level, streak, huy hiệu, activity,
        // collections) — tất cả fail-open để hồ sơ không bao giờ 500 vì
        // lỗi gamification.
        async {
            GamificationRepo::level_of(&state.db, user.id)
                .await
                .unwrap_or(crate::models::gamification::level_from_xp(0))
        },
        async {
            GamificationRepo::current_streak(&state.db, user.id)
                .await
                .unwrap_or(0)
        },
        async {
            // v2.9.2 FIX: query này trước đây chạy HAI LẦN y hệt (ach_res +
            // showcased_res đều gọi user_achievements) — giờ chỉ query 1 lần,
            // clone kết quả cho cả 2 consumer bên dưới (tiết kiệm 1 round-trip
            // DB mỗi lần xem hồ sơ).
            GamificationRepo::user_achievements(&state.db, user.id)
                .await
                .unwrap_or_default()
        },
        async {
            GamificationRepo::recent_activity(&state.db, user.id, 10)
                .await
                .unwrap_or_default()
        },
        async {
            CollectionRepo::list_public_with_owner(&state.db, user.id)
                .await
                .unwrap_or_default()
        },
        async {
            match &current_user {
                Some(u) => unread_count(&state, u.id).await,
                None => 0,
            }
        },
    );
    let stats = stats_res?;
    let games = games_res?;
    let is_following = following_res;
    let preferences = prefs_res.unwrap_or_default();
    // Lấy hồ sơ AI Agent nếu user là AI Agent
    let ai_profile = ai_profile_res;
    let socials = socials_res;
    let unread = unread_res;
    // v2.9.0 — gamification: level/streak/huy hiệu/activity/collections
    let level = level_res;
    let streak = streak_res;
    let all_achievements = ach_res;
    // v2.9.2 — showcased + achievements cùng duyệt 1 kết quả query duy nhất
    // (trước đây 2 query y hệt chạy song song cho 2 danh sách này).
    let showcased: Vec<crate::models::gamification::Achievement> = all_achievements
        .iter()
        .filter(|(_, _, is_shown)| *is_shown)
        .map(|(a, _, _)| a.clone())
        .take(3)
        .collect();
    let achievements: Vec<crate::models::gamification::Achievement> = all_achievements
        .iter()
        .map(|(a, _, _)| a.clone())
        .take(12)
        .collect();
    let achievements_count = (
        all_achievements.len(),
        GamificationRepo::list_achievements(&state.db)
            .await
            .unwrap_or_default()
            .len(),
    );
    let activity = activity_res;
    let collections = collections_res;
    // v3.0.0 — heatmap 13 tuần (dữ liệu thô → grid 7×13)
    let heat_rows = crate::repositories::ActivityRepo::heatmap(&state.db, user.id)
        .await
        .unwrap_or_default();
    let heatmap = build_heatmap_widget(&heat_rows);
    // v3.0.0 — completeness: avatar (35%) + bio (35%) + socials (30%)
    let mut completeness_pct = 0i32;
    if user.avatar_url.as_deref().map(|a| !a.is_empty()).unwrap_or(false) {
        completeness_pct += 35;
    }
    if user.bio.as_deref().map(|b| !b.is_empty()).unwrap_or(false) {
        completeness_pct += 35;
    }
    if !socials.is_empty() {
        completeness_pct += 30;
    }
    let member_months = (chrono::Utc::now() - user.created_at).num_days() / 30;

    Ok(ProfileTemplate {
        current_user,
        unread_notifications: unread,
        user,
        stats,
        games,
        is_following,
        is_self,
        preferences,
        ai_profile,
        socials,
        level,
        streak,
        achievements,
        showcased,
        achievements_count,
        activity,
        collections,
        heatmap,
        completeness_pct,
        member_months,
    })
}

/// Xây grid heatmap 13 tuần × 7 ngày từ danh sách ngày hoạt động.
/// Tuần bắt đầu thứ 2 (cột dọc), ô ngoài phạm vi = None.
fn build_heatmap_widget(
    rows: &[crate::models::retention::HeatmapDay],
) -> crate::templates::HeatmapWidget {
    use chrono::{Datelike, Duration};
    use std::collections::HashMap;
    let today = crate::utils::today_vn();
    let mut counts: HashMap<chrono::NaiveDate, i32> = HashMap::new();
    for r in rows {
        counts.insert(r.day, r.activity_count);
    }
    // Điểm kết thúc = Chủ nhật của tuần hiện tại
    let days_since_monday = i64::from(today.weekday().num_days_from_monday());
    let week_end = today + Duration::days(6 - days_since_monday);
    let week_start = week_end - Duration::days(90); // 13 tuần × 7 - 1
    let mut weeks: Vec<[Option<crate::templates::HeatCell>; 7]> = Vec::with_capacity(13);
    let mut cursor = week_start;
    loop {
        let mut col: [Option<crate::templates::HeatCell>; 7] = Default::default();
        for cell in &mut col {
            if cursor > week_end {
                break;
            }
            let c = counts.get(&cursor).copied().unwrap_or(0);
            *cell = Some(crate::templates::HeatCell::from_count(c));
            cursor += Duration::days(1);
        }
        weeks.push(col);
        if cursor > week_end {
            break;
        }
    }
    crate::templates::HeatmapWidget { weeks }
}

// ============= My profile redirect =============
pub async fn my_profile(AuthUser(user): AuthUser) -> Redirect {
    Redirect::to(&format!("/u/{}", user.username))
}

// ============= Edit profile form =============
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn edit_profile_form(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<EditProfileTemplate> {
    // preferences + socials + unread độc lập — chạy song song
    let (prefs_res, socials_res, unread) = tokio::join!(
        UserRepo::get_preferences(&state.db, user.id),
        UserRepo::social_links(&state.db, user.id),
        unread_count(&state, user.id),
    );
    let preferences = prefs_res.unwrap_or_default();
    let socials = socials_res.unwrap_or_default();
    Ok(EditProfileTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        preferences,
        socials,
        platforms: PLATFORMS,
    })
}

// ============= Update profile =============
#[derive(Deserialize)]
pub struct ProfileForm {
    pub display_name: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub theme: Option<String>,
    pub language: Option<String>,
    pub email_notifications: Option<String>,
    pub show_online: Option<String>,
    /// v2.1.0 — bật/tắt hiệu ứng khung chức vụ (rainbow cho Admin,
    /// glitch cho Mod). Checkbox có name này = bật, vắng = tắt.
    pub role_effects: Option<String>,
    /// v2.7.0 — mạng xã hội: 10 field `social_<platform_id>` (github,
    /// facebook, zalo, discord, youtube, tiktok, instagram, twitter,
    /// telegram, website). Rỗng/vắng = xóa link — hành vi chuẩn HTML
    /// form (input trống nghĩa là user chủ động xóa).
    pub social_github: Option<String>,
    pub social_facebook: Option<String>,
    pub social_zalo: Option<String>,
    pub social_discord: Option<String>,
    pub social_youtube: Option<String>,
    pub social_tiktok: Option<String>,
    pub social_instagram: Option<String>,
    pub social_twitter: Option<String>,
    pub social_telegram: Option<String>,
    pub social_website: Option<String>,
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn update_profile(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Form(form): Form<ProfileForm>,
) -> AppResult<Redirect> {
    // v2.9.1 — NFC normalize: browser/form có thể gửi tên NFD (dán từ nơi
    // khác, một số IME) → dấu tiếng Việt render lệch font trên web.
    let display_name = crate::utils::normalize_nfc(form.display_name.trim());
    if display_name.is_empty() {
        return Err(AppError::BadRequest(
            "Tên hiển thị không được để trống".into(),
        ));
    }
    // Giới hạn độ dài để chống lạm dụng (DB có giới hạn TEXT nhưng vẫn
    // nên chặn sớm ở handler tránh payload lớn vào DB).
    if display_name.chars().count() > 100 {
        return Err(AppError::BadRequest("Tên hiển thị tối đa 100 ký tự".into()));
    }
    let bio = form.bio.unwrap_or_default();
    let bio = bio.trim();
    // v2.5.0 — bio hỗ trợ Markdown: nâng limit 500 → 1000 ký tự (cú pháp
    // markdown **bold**, [link](url), :emoji:... chiếm chỗ; DB column là
    // TEXT không giới hạn). Render qua services::markdown::render_bio —
    // escape toàn bộ raw HTML, URL allowlist, không ToC/YouTube/callout.
    if bio.chars().count() > 1000 {
        return Err(AppError::BadRequest(
            "Giới thiệu bản thân tối đa 1000 ký tự".into(),
        ));
    }
    // Avatar URL: chấp nhận (1) http(s):// URL bên ngoài (Google avatar
    // từ OAuth, hoặc URL ảnh online khác) HOẶC (2) `/uploads/avatars/...`
    // URL do server tự sinh khi user upload qua POST /uploads/avatar.
    // Chặn mọi scheme khác (javascript:, data:, file:) — XSS vector.
    if let Some(url) = form.avatar_url.as_deref().filter(|s| !s.is_empty()) {
        if crate::services::storage::is_upload_url(url) {
            // URL upload nội bộ — luôn hợp lệ (filename do server sinh).
        } else if !crate::utils::is_safe_url(url) {
            return Err(AppError::BadRequest(
                "Avatar URL phải là http(s):// hoặc /uploads/avatars/...".into(),
            ));
        }
        if url.len() > 2048 {
            return Err(AppError::BadRequest(
                "Avatar URL quá dài (tối đa 2048 ký tự)".into(),
            ));
        }
    }
    let avatar_url = form.avatar_url.as_deref().filter(|s| !s.is_empty());

    // v2.7.0 — Validate 10 link mạng xã hội (allowlist hostname từng
    // nền tảng — xem models::social). Lỗi validation → BadRequest với
    // thông báo rõ ràng, KHÔNG lưu nửa chừng (validate xong mới ghi DB).
    let social_input = [
        ("github", form.social_github.as_deref()),
        ("facebook", form.social_facebook.as_deref()),
        ("zalo", form.social_zalo.as_deref()),
        ("discord", form.social_discord.as_deref()),
        ("youtube", form.social_youtube.as_deref()),
        ("tiktok", form.social_tiktok.as_deref()),
        ("instagram", form.social_instagram.as_deref()),
        ("twitter", form.social_twitter.as_deref()),
        ("telegram", form.social_telegram.as_deref()),
        ("website", form.social_website.as_deref()),
    ];
    let socials = SocialLinks::validate_form(&social_input).map_err(AppError::BadRequest)?;

    UserRepo::update_profile(&state.db, user.id, &display_name, bio, avatar_url).await?;
    // v2.7.0 — Lưu socials SAU khi profile + preferences update thành
    // công. Lỗi save socials → 500 (đáng lẽ tránh:validate đã pass trước
    // nên lỗi chỉ còn DB-level — hiếm, user sửa lại được).
    UserRepo::save_social_links(&state.db, user.id, &socials).await?;

    // Update preferences — whitelist giá trị hợp lệ, giá trị lạ quay về
    // mặc định (trước đây lưu thẳng chuỗi tuỳ ý vào DB, render vào
    // data-theme attr của <html>).
    let theme = match form.theme.as_deref() {
        Some("light") => "light",
        _ => "dark",
    };
    let language = match form.language.as_deref() {
        Some("en") => "en",
        _ => "vi",
    };
    let email_notif = form.email_notifications.is_some();
    let show_online = form.show_online.is_some();
    // Hiệu ứng khung chức vụ: form chỉ render checkbox `role_effects` cho
    // staff (Admin/Mod). Với member, checkbox vắng mặt → nếu ghi thẳng
    // `false` thì khi member được thăng chức sau này, hiệu ứng bị TẮT oan
    // (trái với mặc định bật). Giải pháp: staff đọc từ form, member giữ
    // nguyên giá trị cũ (hoặc default true nếu chưa từng lưu).
    let role_badge_effects = if user.role.is_staff() {
        form.role_effects.is_some()
    } else {
        UserRepo::get_preferences(&state.db, user.id)
            .await
            .map(|p| p.role_badge_effects)
            .unwrap_or(true)
    };
    UserRepo::update_preferences(
        &state.db,
        user.id,
        theme,
        email_notif,
        show_online,
        language,
        role_badge_effects,
    )
    .await?;

    // v2.9.0 — huy hiệu onboarding (avatar/bio/social) — best-effort
    let db_hook = state.db.clone();
    let uid_hook = user.id;
    tokio::spawn(async move {
        crate::services::gamification::on_profile_update(&db_hook, uid_hook).await;
        // v3.0.0 — onboarding steps avatar/bio (best-effort)
        crate::services::retention::check_profile_onboarding(&db_hook, uid_hook).await;
    });
    Ok(Redirect::to(&format!("/u/{}", user.username)))
}

// ============= Bookmarks page =============
#[derive(Deserialize, Default)]
pub struct BookmarksQuery {
    pub page: Option<i64>,
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn bookmarks_page(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Query(q): Query<BookmarksQuery>,
) -> AppResult<BookmarksTemplate> {
    let page = q.page.unwrap_or(1).clamp(1, 10_000);
    let per_page: i64 = 24;
    // FIX v2.8.1: saturating math — page ~4e17 làm (page-1)*per_page
    // tràn i64 → OFFSET âm → 500 (prod) / panic (debug).
    let offset = page.saturating_sub(1).saturating_mul(per_page);
    let games = InteractionRepo::bookmarks_for_user(&state.db, user.id, per_page, offset).await?;
    let total = InteractionRepo::count_bookmarks_for_user(&state.db, user.id)
        .await
        .unwrap_or(0);
    let unread = unread_count(&state, user.id).await;
    Ok(BookmarksTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        games,
        page,
        per_page,
        total,
    })
}

// ============================================================
// v2.9.0 — PHIÊN ĐĂNG NHẬP CỦA TÔI + XUẤT DỮ LIỆU (GDPR)
// ============================================================

/// GET /profile/sessions — danh sách phiên của CHÍNH MÌNH.
/// Tương đương /admin/sessions nhưng scope user tự xem/tự thu hồi.
/// Phiên hiện tại xác định bằng token trong cookie (hash → session id).
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn my_sessions_page(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    jar: axum_extra::extract::CookieJar,
) -> AppResult<crate::templates::ProfileSessionsTemplate> {
    let current_session_id = match jar.get(crate::auth::SESSION_COOKIE) {
        Some(c) => crate::repositories::SessionRepo::find_id_by_token(
            &state.db,
            &crate::auth::hash_token(c.value()),
        )
        .await
        .unwrap_or(None),
        None => None,
    };
    let rows = crate::repositories::SessionRepo::list_own_sessions(&state.db, user.id).await?;
    let unread = unread_count(&state, user.id).await;
    let sessions: Vec<crate::templates::MySessionRow> = rows
        .into_iter()
        .map(|s| crate::templates::MySessionRow {
            id: s.id,
            user_agent: s.user_agent.unwrap_or_else(|| "Không rõ thiết bị".into()),
            ip: s.ip_address,
            created_at: s.created_at,
            expires_at: s.expires_at,
            current: Some(s.id) == current_session_id,
        })
        .collect();
    Ok(crate::templates::ProfileSessionsTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        sessions,
    })
}

/// POST /profile/sessions/{id}/revoke — thu hồi phiên của chính mình.
/// Không cho thu hồi phiên ĐANG DÙNG (dùng /auth/logout để logout chính mình).
/// # Errors
///
/// Trả về lỗi khi phiên không phải của mình / là phiên hiện tại.
pub async fn revoke_own_session(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<uuid::Uuid>,
) -> AppResult<Redirect> {
    // Không cần check "phải phiên hiện tại" — delete_for_user scope theo
    // user_id nên chỉ xóa được phiên của chính mình. Thu hồi cả phiên đang
    // dùng cũng hợp lệ (user sẽ bị logout ở thiết bị này).
    let deleted = crate::repositories::SessionRepo::delete_for_user(&state.db, id, user.id).await?;
    if !deleted {
        return Err(AppError::NotFound("Phiên không tồn tại".into()));
    }
    // v3.0.0 FIX: trước đây chỉ xoá row DB mà không invalidate session
    // cache (TTL 10s) → thiết bị bị thu hồi vẫn dùng được tới 10s, mâu
    // thuẫn với bất biến "user bị đá ra NGAY LẬP TỨC" ở logout/logout-all/
    // admin revoke. invalidate_session_cache_for_user xoá mọi entry cache
    // của user này — an toàn (chỉ tốn 1 cache miss cho các phiên còn lại).
    crate::middleware::invalidate_session_cache_for_user(user.id);
    Ok(Redirect::to("/profile/sessions"))
}

/// GET /profile/export — xuất dữ liệu cá nhân (JSON, GDPR).
/// Gồm: hồ sơ, preferences, socials, games, bookmarks, comments.
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn export_my_data(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<axum::response::Response> {
    use axum::http::header;
    use serde_json::json;
    let (games, bookmarks, comments) = tokio::join!(
        GameRepo::by_user(&state.db, user.id, 1000, 0),
        InteractionRepo::bookmarks_for_user(&state.db, user.id, 1000, 0),
        crate::repositories::SessionRepo::comments_for_export(&state.db, user.id, 1000),
    );
    let data = json!({
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "profile": {
            "username": user.username,
            "display_name": user.display_name,
            "bio": user.bio,
            "avatar_url": user.avatar_url,
            "created_at": user.created_at.to_rfc3339(),
        },
        "games": games?.iter().map(|g| json!({
            "title": g.title, "slug": g.slug,
        })).collect::<Vec<_>>(),
        "bookmarks": bookmarks?.iter().map(|g| json!({
            "title": g.title, "slug": g.slug,
        })).collect::<Vec<_>>(),
        "comments": comments?.iter().map(|(content, created_at)| json!({
            "content": content, "created_at": created_at.to_rfc3339(),
        })).collect::<Vec<_>>(),
    });
    let filename = format!("louis-space-data-{}.json", user.username);
    Ok((
        [
            (header::CONTENT_TYPE, "application/json; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\"").as_str(),
            ),
        ],
        axum::Json(data),
    )
        .into_response())
}
