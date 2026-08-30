use crate::auth;
use crate::error::{AppError, AppResult};
use crate::middleware::{AuthUser, CurrentUser};
use crate::repositories::{ReferralRepo, SessionRepo, UserRepo};
use crate::state::AppState;
use crate::templates::LoginTemplate;
use askama::Template;
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum_extra::extract::CookieJar;
use rand::RngExt;
use serde::Deserialize;
use std::sync::Arc;

/// Tên cookie chứa OAuth `state` (CSRF token) - sống 10 phút.
pub const OAUTH_STATE_COOKIE: &str = "kg_oauth_state";

#[derive(Deserialize)]
pub struct AuthQuery {
    pub next: Option<String>,
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn login_page(
    State(_state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<Response> {
    if current_user.is_some() {
        // Đã đăng nhập → về trang chủ thay vì trả lỗi 400 (trước đây
        // người dùng bấm "Đăng nhập" khi còn phiên sẽ thấy trang lỗi)
        return Ok(Redirect::to("/").into_response());
    }
    // Nút Google trong template trỏ tới `/auth/google` — route duy nhất sinh
    // CSRF state và set cookie `kg_oauth_state`. Không build auth_url tại đây
    // để tránh state không có cookie đối chiếu (bug v0.3.0).
    let tpl = LoginTemplate {
        current_user: None,
        unread_notifications: 0,
    };
    Ok(Html(tpl.render().map_err(AppError::from)?).into_response())
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn google_login(
    State(state): State<Arc<AppState>>,
    Query(q): Query<AuthQuery>,
    jar: CookieJar,
) -> AppResult<(CookieJar, Redirect)> {
    let csrf = gen_csrf();
    let auth_url = auth::build_auth_url(&state, &csrf);

    // Lưu CSRF state vào cookie HttpOnly + SameSite=Lax sống 10 phút.
    // Khi callback về, ta sẽ verify state khớp cookie để chặn login-CSRF.
    let mut new_jar = jar;
    auth::set_oauth_state_cookie(&mut new_jar, &csrf, &state.config.base_url);

    // Lưu `next` vào cookie tạm để redirect sau khi đăng nhập thành công.
    if let Some(next) = q.next.as_deref().filter(|s| !s.is_empty()) {
        // Dùng helper chung: chặn control char, scheme tuyệt đối,
        // protocol-relative — chống header injection & open redirect.
        let safe_next = crate::utils::sanitize_redirect(next);
        auth::set_oauth_next_cookie(&mut new_jar, &safe_next, &state.config.base_url);
    }

    Ok((new_jar, Redirect::to(&auth_url)))
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn google_callback(
    State(state): State<Arc<AppState>>,
    Query(q): Query<CallbackQuery>,
    jar: CookieJar,
    headers: axum::http::HeaderMap,
    connect_info: axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> AppResult<(CookieJar, Redirect)> {
    if let Some(err) = &q.error {
        // Clamp log message: tránh attacker dùng error= rất dài để bloat log.
        let err_short: String = err.chars().take(500).collect();
        tracing::warn!("OAuth error: {}", err_short);
        // v2.7.0 — BadRequest (400) thay vì OAuth (500): user chủ động
        // từ chối consent (access_denied) là luồng BÌNH THƯỜNG, trả 500
        // "Lỗi hệ thống" là sai sự thật + làm nhiễu error monitor. Trả
        // 400 với message rõ để user hiểu chỉ cần thử đăng nhập lại.
        return Err(AppError::BadRequest(format!(
            "Đăng nhập Google không thành công ({err_short}). Vui lòng thử lại và cho phép truy cập khi Google hỏi."
        )));
    }

    // === CSRF verification: state từ Google phải khớp state trong cookie ===
    let cookie_state = jar.get(OAUTH_STATE_COOKIE).map(|c| c.value().to_string());
    let next_path = jar
        .get(auth::OAUTH_NEXT_COOKIE)
        .map(|c| c.value().to_string());
    // v3.0.0 — capture referral cookie TRƯỚC khi jar bị move vào cleanup
    let referral_code_cookie = jar
        .get(crate::handlers::referral::REFERRAL_COOKIE)
        .map(|c| c.value().to_string());
    let mut cleanup_jar = jar;
    // Xoá cookie state và next dù thành công hay thất bại
    auth::clear_oauth_state_cookie(&mut cleanup_jar, &state.config.base_url);
    auth::clear_oauth_next_cookie(&mut cleanup_jar, &state.config.base_url);

    match (q.state.as_deref(), cookie_state.as_deref()) {
        (Some(s_from_google), Some(s_from_cookie))
            // v2.9.2 — constant-time so sánh (nhất quán với AI token; tránh
            // timing oracle dù thực tế khó khai thác qua mạng).
            if crate::utils::constant_time_eq(
                s_from_google.as_bytes(),
                s_from_cookie.as_bytes(),
            ) =>
        {
            // OK - state khớp
        }
        _ => {
            // v3.4.2 FIX (audit "token trong log"): KHÔNG log nguyên giá trị
            // state (log aggregation thường có readership rộng hơn app) —
            // chỉ log 8 ký tự đầu để correlate + độ dài để debug.
            let g_prefix = q.state.as_deref().map(|s| &s[..s.len().min(8)]);
            let c_prefix = cookie_state.as_deref().map(|s| &s[..s.len().min(8)]);
            tracing::warn!(
                google_prefix = ?g_prefix,
                cookie_prefix = ?c_prefix,
                google_len = q.state.as_deref().map_or(0, str::len),
                cookie_len = cookie_state.as_deref().map_or(0, str::len),
                "OAuth state mismatch — từ chối callback (CSRF)"
            );
            return Err(AppError::BadRequest(
                "OAuth state không khớp - có thể bị CSRF. Vui lòng đăng nhập lại.".into(),
            ));
        }
    }

    // Validate code param — Google code thường 100-200 ký tự; 1MB code
    // sẽ được gửi sang Google API → waste bandwidth + log bloat.
    let code: String = q.code.chars().take(2048).collect();
    let token = auth::exchange_code(&state, &code).await?;
    let userinfo = auth::fetch_userinfo(&state, &token.access_token).await?;

    if !userinfo.email_verified.unwrap_or(false) {
        return Err(AppError::BadRequest(
            "Email Google chưa được xác minh".into(),
        ));
    }

    // Find or create user
    let mut is_new_user = false;
    let mut user = match UserRepo::find_by_google_sub(&state.db, &userinfo.sub).await? {
        Some(u) if u.is_banned => {
            return Err(AppError::Forbidden("Tài khoản đã bị cấm".into()));
        }
        Some(u) => u,
        None => {
            is_new_user = true;
            // Capture IP/UA cho admin truy vết (migration 009)
            let ip_new = crate::middleware::client_ip_from_parts(
                &headers,
                Some(&connect_info.0),
                state.config.trust_proxy_headers,
                state.config.trusted_proxy_hops,
            );
            let ua_new = headers
                .get(axum::http::header::USER_AGENT)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .chars()
                .take(1024)
                .collect::<String>();
            UserRepo::create_from_google(
                &state.db,
                &userinfo.sub,
                &userinfo.email,
                // v2.9.1 — NFC normalize: Google đôi khi trả name NFD
                // (decomposed) → dấu tiếng Việt render lệch font trên web.
                &crate::utils::normalize_nfc(userinfo.name.as_deref().unwrap_or("Người dùng")),
                userinfo.picture.as_deref(),
                Some(&ip_new),
                Some(&ua_new),
            )
            .await?
        }
    };

    // Tự động cấp admin cho ADMIN_EMAIL (bootstrap siêu-admin).
    // v3.4.2 FIX (audit "default superuser"): ADMIN_EMAIL không set →
    // config trả chuỗi rỗng → TỪ CHỐI auto-grant (log error 1 lần ở
    // startup). Không còn fallback Gmail cố định trên fork/redeploy.
    if !state.config.admin_email.is_empty()
        && userinfo
            .email
            .eq_ignore_ascii_case(&state.config.admin_email)
        && !user.role.is_admin()
    {
        UserRepo::set_role(&state.db, user.id, "admin").await?;
        user.role = crate::models::user::UserRole::Admin;
        tracing::info!("Granted admin to {} via ADMIN_EMAIL", userinfo.email);
    }

    // Create session — lưu User-Agent (cắt ngắn tránh overflow) và IP
    // client để admin có thể audit / xoá phiên nếu cần.
    let session_token = auth::gen_session_token();
    let token_hash = auth::hash_token(&session_token);
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .chars()
        .take(255)
        .collect::<String>();
    let ip = crate::middleware::client_ip_from_parts(
        &headers,
        Some(&connect_info.0),
        state.config.trust_proxy_headers,
        state.config.trusted_proxy_hops,
    );
    SessionRepo::create(
        &state.db,
        user.id,
        &token_hash,
        &user_agent,
        Some(&ip),
        auth::SESSION_TTL_DAYS,
    )
    .await?;
    UserRepo::update_last_seen(&state.db, user.id).await?;
    // Record login IP/UA cho admin detail view (migration 009).
    // Best-effort: lỗi không block login flow.
    let _ = UserRepo::record_login(&state.db, user.id, Some(&ip), Some(&user_agent)).await;
    // Dọn session hết hạn (best-effort, tránh bảng phình to vô hạn)
    let _ = SessionRepo::cleanup_expired(&state.db).await;

    let mut new_jar = cleanup_jar;
    auth::set_session_cookie(&mut new_jar, &session_token, &state.config.base_url);

    // v3.0.0 — REFERRAL: người MỚI (is_new_user) đăng nhập lần đầu với
    // cookie referral (từ link /r/{code}) → ghi nhận + thưởng XP cả 2
    // phía. Chỉ user MỚI được nhận (tài khoản cũ gắn cookie không có
    // gì thay đổi) — chống self-referral cho account sẵn có.
    if is_new_user {
        if let Some(ref_code) = referral_code_cookie {
            let referrer = ReferralRepo::resolve_code(&state.db, &ref_code)
                .await
                .ok()
                .flatten();
            if let Some(referrer_id) = referrer {
                match ReferralRepo::record_referral(&state.db, referrer_id, user.id).await {
                    Ok(Some(rid)) => {
                        let st = state.clone();
                        let (new_uid, rid) = (user.id, rid);
                        tokio::spawn(async move {
                            crate::handlers::referral::reward_both(&st, rid, new_uid).await;
                        });
                    }
                    Ok(None) => {
                        tracing::info!(code = %ref_code, "Referral bỏ qua (đã có/self)");
                    }
                    Err(e) => tracing::warn!("Referral record fail: {e}"),
                }
            }
            // Xoá cookie referral (dùng kèm path "/" khớp với lúc set)
            use axum_extra::extract::cookie::Cookie;
            new_jar = new_jar.remove(
                Cookie::build((crate::handlers::referral::REFERRAL_COOKIE, ""))
                    .path("/")
                    .build(),
            );
        }
    }

    // Redirect về `next` nếu có, mặc định /
    // Dùng helper sanitize_redirect thống nhất — chặn control char,
    // scheme tuyệt đối, protocol-relative (chống header injection
    // qua Location + chống open redirect).
    let redirect_target = next_path
        .as_deref()
        .map(crate::utils::sanitize_redirect)
        .unwrap_or_else(|| "/".to_string());

    // v2.9.0 — Gamification hooks (fire-and-forget, không block redirect):
    // - User mới: notification chào mừng + hướng dẫn onboarding
    // - Mọi lần login: kiểm huy hiệu (first_login, onboarding đã đạt)
    {
        let db = state.db.clone();
        let (uid, dname, is_new) = (user.id, user.display_name.clone(), is_new_user);
        tokio::spawn(async move {
            if is_new {
                crate::services::gamification::send_welcome(&db, uid, &dname).await;
            }
            crate::services::gamification::on_login(&db, uid).await;
        });
    }

    Ok((new_jar, Redirect::to(&redirect_target)))
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn logout(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    jar: CookieJar,
) -> AppResult<(CookieJar, Redirect)> {
    if let Some(token) = jar.get(auth::SESSION_COOKIE) {
        let token_hash = auth::hash_token(token.value());
        SessionRepo::delete(&state.db, &token_hash).await?;
        // v2.1.0 — xoá session cache để user bị đá ra NGAY (không đợi TTL 10s
        // của SESSION_CACHE — logout mà 10s sau vẫn còn đăng nhập là bug).
        crate::middleware::invalidate_session_cache(&token_hash);
    }
    let mut new_jar = jar;
    auth::clear_session_cookie(&mut new_jar, &state.config.base_url);
    let _ = current_user;

    // v3.5.0 FIX (audit vòng 5) — đang IMPERSONATE AI Agent → đăng xuất
    // khỏi phiên AI là TIÊU THỤ ticket one-shot (đánh dấu used_at) rồi QUAY
    // LẠI phiên admin bằng cách mint session MỚI. Bản v3.3.0 cũ hash ticket
    // UUID rồi tra sessions → không bao giờ khớp (ticket không phải token)
    // → admin bị đăng xuất hẳn MÀ ticket vẫn còn sống 2h (lộ = mở lại được).
    if let Some(imp_raw) = new_jar
        .get(auth::IMPERSONATOR_COOKIE)
        .map(|c| c.value().to_string())
    {
        if let Ok(tid) = uuid::Uuid::parse_str(&imp_raw) {
            let admin_id: Option<uuid::Uuid> = sqlx::query_scalar(
                r#"UPDATE impersonation_tickets
                   SET used_at = NOW()
                   WHERE id = $1 AND used_at IS NULL AND expires_at > NOW()
                   RETURNING admin_user_id"#,
            )
            .bind(tid)
            .fetch_optional(&state.db)
            .await?;
            if let Some(admin_id) = admin_id {
                if let Ok(Some(admin_user)) = UserRepo::find_by_id(&state.db, admin_id).await {
                    if admin_user.role.is_staff() && !admin_user.is_banned {
                        let token = auth::gen_session_token();
                        let token_hash = auth::hash_token(&token);
                        SessionRepo::create(
                            &state.db,
                            admin_user.id,
                            &token_hash,
                            "impersonation-restore",
                            None,
                            30,
                        )
                        .await?;
                        tracing::warn!(
                            admin = %admin_user.username,
                            "Impersonation LOGOUT — tiêu thụ ticket, khôi phục phiên admin"
                        );
                        auth::clear_impersonator_cookie(&mut new_jar, &state.config.base_url);
                        auth::set_session_cookie(&mut new_jar, &token, &state.config.base_url);
                        return Ok((new_jar, Redirect::to("/admin/ai-agents")));
                    }
                }
            }
        }
    }
    auth::clear_impersonator_cookie(&mut new_jar, &state.config.base_url);
    Ok((new_jar, Redirect::to("/")))
}

/// Đăng xuất khỏi MỌI thiết bị: xoá toàn bộ session của user trong DB
/// (laptop, điện thoại, máy khác đang lưu phiên), kể cả phiên hiện tại.
/// Dùng khi nghi ngờ tài khoản bị truy cập trái phép.
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn logout_all(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    jar: CookieJar,
) -> AppResult<(CookieJar, Redirect)> {
    SessionRepo::delete_all_for_user(&state.db, user.id).await?;
    // v2.1.0 — xoá toàn bộ session cache của user khỏi mọi thiết bị.
    crate::middleware::invalidate_session_cache_for_user(user.id);
    tracing::info!(
        user = %user.username,
        "User đăng xuất khỏi tất cả thiết bị (xoá toàn bộ session)"
    );
    let mut new_jar = jar;
    auth::clear_session_cookie(&mut new_jar, &state.config.base_url);
    Ok((new_jar, Redirect::to("/")))
}

fn gen_csrf() -> String {
    let mut rng = rand::rng();
    let bytes: [u8; 16] = rng.random();
    hex::encode(bytes)
}

// Helper to read user's unread notifications count
pub async fn unread_count(state: &AppState, user_id: uuid::Uuid) -> i64 {
    crate::repositories::NotificationRepo::unread_count(&state.db, user_id)
        .await
        .unwrap_or(0)
}
