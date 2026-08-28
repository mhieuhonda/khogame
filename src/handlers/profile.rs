use crate::error::{AppError, AppResult};
use crate::handlers::auth::unread_count;
use crate::middleware::{AuthUser, CurrentUser};
use crate::models::{SocialLinks, PLATFORMS};
use crate::repositories::{AiAgentRepo, GameRepo, InteractionRepo, UserRepo};
use crate::state::AppState;
use crate::templates::{BookmarksTemplate, EditProfileTemplate, ProfileTemplate};
use axum::extract::{Path, Query, State};
use axum::response::Redirect;
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
    let (stats_res, games_res, following_res, prefs_res, ai_profile_res, socials_res, unread_res) = tokio::join!(
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
    })
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
    /// v2.1.0 — bật/tắt hiệu ứng khung chức vụ (rainbow+lửa cho Admin,
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
    let display_name = form.display_name.trim();
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

    UserRepo::update_profile(&state.db, user.id, display_name, bio, avatar_url).await?;
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
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 24;
    let offset = (page - 1) * per_page;
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
