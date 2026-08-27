use crate::handlers;
use crate::middleware::{
    error_page_mw, maintenance_guard, origin_check, rate_limit, require_admin, require_ai_agent,
    security_headers,
};
use crate::state::AppState;
use axum::middleware;
use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

pub fn build_router(state: Arc<AppState>) -> Router {
    let public_routes = Router::new()
        .route("/", get(handlers::games::home))
        .route("/login", get(handlers::auth::login_page))
        .route("/auth/google", get(handlers::auth::google_login))
        .route(
            "/auth/google/callback",
            get(handlers::auth::google_callback),
        )
        .route("/auth/logout", post(handlers::auth::logout))
        .route("/auth/logout-all", post(handlers::auth::logout_all))
        // === AI Agent auth (public nhưng yêu cầu secret/token) ===
        .route("/auth/ai/login", get(handlers::ai_agent::login_form))
        .route("/auth/ai/login", post(handlers::ai_agent::login))
        // Register: chỉ AI có secret mới dùng được. Trả 403 nếu secret sai.
        .route("/auth/ai/register", post(handlers::ai_agent::register))
        // Games - CRUD
        .route("/games/new", get(handlers::games::new_game_form))
        .route(
            "/games",
            get(handlers::games::list_all).post(handlers::games::create_game),
        )
        .route("/games/latest", get(handlers::games::list_latest))
        .route("/games/trending", get(handlers::games::list_trending))
        .route("/games/top-rated", get(handlers::games::list_top_rated))
        .route("/games/downloads", get(handlers::games::list_downloads))
        .route("/games/featured", get(handlers::games::list_featured))
        .route("/games/{slug}", get(handlers::games::show_game))
        .route("/games/{slug}/edit", get(handlers::games::edit_game_form))
        .route(
            "/games/{slug}",
            post(handlers::games::update_game).delete(handlers::games::delete_game),
        )
        .route("/games/{slug}/delete", post(handlers::games::delete_game))
        .route("/games/{slug}/publish", post(handlers::games::publish_game))
        .route(
            "/games/{slug}/download",
            post(handlers::games::download_game),
        )
        .route(
            "/games/{slug}/report-form",
            get(handlers::games::report_form),
        )
        .route("/games/{slug}/report", post(handlers::games::submit_report))
        .route(
            "/games/{slug}/like",
            post(handlers::interactions::toggle_like),
        )
        .route(
            "/games/{slug}/bookmark",
            post(handlers::interactions::toggle_bookmark),
        )
        .route("/games/{slug}/rate", post(handlers::interactions::rate))
        .route("/games/{slug}/share", post(handlers::games::share_game))
        .route(
            "/games/{slug}/comments",
            get(handlers::comments::list_comments_page).post(handlers::comments::create_comment),
        )
        // Game của tôi
        .route("/my-games", get(handlers::games::my_games))
        // Comments
        .route(
            "/comments/{id}",
            post(handlers::comments::like_comment).delete(handlers::comments::delete_comment),
        )
        .route(
            "/comments/{id}/like",
            post(handlers::comments::like_comment),
        )
        .route(
            "/comments/{id}/edit",
            post(handlers::comments::edit_comment),
        )
        .route(
            "/comments/{id}/replies",
            get(handlers::comments::list_replies),
        )
        // GitHub Repos
        .route("/repos", get(handlers::repos::list))
        .route("/repos/new", get(handlers::repos::new_form))
        .route("/repos", post(handlers::repos::create))
        .route("/repos/{id}/refresh", post(handlers::repos::refresh))
        .route(
            "/repos/{id}/delete",
            post(handlers::repos::delete_own).delete(handlers::repos::delete_own),
        )
        .route(
            "/u/{username}/repos",
            get(handlers::repos::user_repos_fragment),
        )
        // Users & profile
        .route("/u/{username}", get(handlers::profile::show_profile))
        .route(
            "/u/{username}/follow",
            post(handlers::interactions::toggle_follow),
        )
        .route("/categories", get(handlers::games::list_categories))
        .route("/c/{slug}", get(handlers::games::list_by_category))
        .route("/t/{slug}", get(handlers::games::list_by_tag))
        .route("/search", get(handlers::games::search))
        .route("/profile", get(handlers::profile::my_profile))
        .route("/profile/edit", get(handlers::profile::edit_profile_form))
        .route("/profile", post(handlers::profile::update_profile))
        // AI Agent tự cập nhật hồ sơ (yêu cầu AI Agent session)
        .route("/profile/ai", post(handlers::ai_agent::update_profile))
        .route(
            "/profile/ai/edit",
            get(handlers::ai_agent::edit_profile_form),
        )
        .route("/bookmarks", get(handlers::profile::bookmarks_page))
        .route("/notifications", get(handlers::notifications::list))
        .route(
            "/notifications/{id}/read",
            post(handlers::notifications::mark_read),
        )
        .route(
            "/notifications/mark-all-read",
            post(handlers::notifications::mark_all_read),
        )
        // Static pages
        .route("/terms", get(handlers::pages::terms))
        .route("/privacy", get(handlers::pages::privacy))
        .route("/health", get(handlers::api::health_lb))
        .route("/maintenance", get(handlers::pages::maintenance))
        // === News module ===
        .route(
            "/news",
            get(handlers::news::list).post(handlers::news::create),
        )
        .route("/news/new", get(handlers::news::new_form))
        .route("/news/{slug}", get(handlers::news::show))
        .route("/news/{slug}/edit", get(handlers::news::edit_form))
        .route(
            "/news/{slug}",
            post(handlers::news::update).delete(handlers::news::delete),
        )
        .route("/news/{slug}/like", post(handlers::news::toggle_like))
        .route(
            "/news/{slug}/comments",
            post(handlers::news::create_comment),
        )
        .route("/my-news", get(handlers::news::my_news))
        // === Live Chat ===
        // WebSocket realtime chat — auth qua cookie HTTP trước khi upgrade.
        // Nếu chưa login, ws_handler trả 401 ngay (không open WS rỗng).
        .route("/chat/ws", get(handlers::chat::ws_handler))
        // HTTP fallback: load 50 tin gần nhất + count online + count today
        .route("/chat/history", get(handlers::chat::history))
        .route("/chat/online", get(handlers::chat::online))
        .route("/chat/auth", get(handlers::chat::auth_check))
        .route(
            "/chat/{id}/delete",
            post(handlers::chat::http_delete).delete(handlers::chat::http_delete),
        )
        // === Image uploads (multipart) ===
        // Mỗi endpoint nhận field `file` (multipart/form-data), trả JSON
        // `{"url": "/uploads/<subdir>/<uuid>.<ext>", "size": <bytes>}`.
        // Client HTMX dùng URL để fill hidden field + preview <img>.
        // Tất cả yêu cầu AuthUser (đăng nhập) — xem handlers/uploads.rs.
        .route("/uploads/avatar", post(handlers::uploads::avatar))
        .route("/uploads/game/cover", post(handlers::uploads::game_cover))
        .route("/uploads/news/cover", post(handlers::uploads::news_cover))
        .route("/uploads/repo/image", post(handlers::uploads::repo_image));

    // Public JSON API v1
    let api_routes = Router::new()
        .route("/", get(handlers::api::root))
        .route("/games", get(handlers::api::games_list))
        .route("/games/{slug}", get(handlers::api::game_detail))
        .route("/games/{slug}/related", get(handlers::api::game_related))
        .route("/games/{slug}/comments", get(handlers::api::game_comments))
        .route("/repos", get(handlers::api::repos_list))
        .route("/tags", get(handlers::api::tags_list))
        .route("/categories", get(handlers::api::categories_list))
        .route(
            "/categories/{slug}/games",
            get(handlers::api::games_by_category),
        )
        .route("/tags/{slug}/games", get(handlers::api::games_by_tag))
        .route("/users/{username}", get(handlers::api::user_profile))
        .route("/stats", get(handlers::api::stats_overview))
        .route("/health", get(handlers::api::health_detail))
        // News API
        .route("/news", get(handlers::api::news_list))
        .route("/news/{slug}", get(handlers::api::news_detail));

    // Nội bộ (htmx fetch)
    let internal_routes = Router::new()
        .route("/announcement", get(handlers::api::announcement))
        .route("/check-duplicate", get(handlers::api::check_duplicate))
        .route(
            "/news-check-duplicate",
            get(handlers::api::news_check_duplicate),
        )
        .route("/suggest", get(handlers::api::games_suggest))
        .route("/news-suggest", get(handlers::api::news_suggest))
        .route("/preferences/theme", post(handlers::api::set_theme));

    // === AI Agent internal routes: yêu cầu AI Agent auth ===
    // (Bearer token trong header hoặc session cookie của AI Agent)
    let ai_internal_routes = Router::new()
        .route("/info", get(handlers::ai_agent::info))
        .route("/progress", post(handlers::ai_agent::report_progress))
        .route(
            "/progress.json",
            post(handlers::ai_agent::report_progress_json),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_ai_agent,
        ));

    let admin_routes = Router::new()
        .route("/admin", get(handlers::admin::dashboard))
        .route("/admin/reports", get(handlers::admin::reports))
        .route(
            "/admin/reports/{id}/resolve",
            post(handlers::admin::resolve_report),
        )
        .route("/admin/games/{id}/hide", post(handlers::admin::hide_game))
        .route(
            "/admin/games/{id}/feature",
            post(handlers::admin::feature_game),
        )
        .route(
            "/admin/comments/{id}/pin",
            post(handlers::admin::pin_comment),
        )
        // Games
        .route("/admin/games", get(handlers::admin::games))
        .route(
            "/admin/games/{id}/delete",
            post(handlers::admin::delete_game).delete(handlers::admin::delete_game),
        )
        // Users
        .route("/admin/users", get(handlers::admin::users))
        .route("/admin/users/{id}", get(handlers::admin::user_detail))
        .route("/admin/users/{id}/role", post(handlers::admin::set_role))
        .route("/admin/users/{id}/ban", post(handlers::admin::set_banned))
        // Comments
        .route("/admin/comments", get(handlers::admin::comments))
        .route(
            "/admin/comments/{id}/delete",
            post(handlers::admin::delete_comment).delete(handlers::admin::delete_comment),
        )
        // Categories
        .route("/admin/categories", get(handlers::admin::categories))
        .route(
            "/admin/categories/save",
            post(handlers::admin::save_category),
        )
        .route(
            "/admin/categories/{id}/delete",
            post(handlers::admin::delete_category).delete(handlers::admin::delete_category),
        )
        // v1.4.0 — News Categories (CRUD riêng cho tin tức, khác với categories game)
        .route(
            "/admin/news-categories",
            get(handlers::admin::news_categories),
        )
        .route(
            "/admin/news-categories/save",
            post(handlers::admin::save_news_category),
        )
        .route(
            "/admin/news-categories/{id}/delete",
            post(handlers::admin::delete_news_category)
                .delete(handlers::admin::delete_news_category),
        )
        // Repos
        .route("/admin/repos", get(handlers::admin::repos))
        .route(
            "/admin/repos/{id}/status",
            post(handlers::admin::set_repo_status),
        )
        // === AI Agent admin pages ===
        .route("/admin/ai-agents", get(handlers::admin::ai_agents))
        .route("/admin/ai-reports", get(handlers::admin::ai_reports))
        // Settings
        .route(
            "/admin/settings",
            get(handlers::admin::settings_page).post(handlers::admin::save_settings),
        )
        .route("/admin/broadcast", post(handlers::admin::broadcast))
        // Audit & export
        .route("/admin/audit", get(handlers::admin::audit_log))
        .route("/admin/sessions", get(handlers::admin::sessions))
        .route(
            "/admin/sessions/{id}/revoke",
            post(handlers::admin::revoke_session),
        )
        .route("/admin/export", get(handlers::admin::export))
        // === News admin (chỉ admin, không phải mod) ===
        .route("/admin/news/pending", get(handlers::admin::news_pending))
        .route("/admin/news/all", get(handlers::admin::news_all))
        .route(
            "/admin/news/{id}/approve",
            post(handlers::admin::news_approve),
        )
        .route(
            "/admin/news/{id}/reject",
            post(handlers::admin::news_reject),
        )
        .route(
            "/admin/news/{id}/archive",
            post(handlers::admin::news_archive),
        )
        .route(
            "/admin/news/{id}/feature",
            post(handlers::admin::news_feature),
        )
        .route(
            "/admin/news/{id}/unfeature",
            post(handlers::admin::news_unfeature),
        )
        .route(
            "/admin/news/{id}/delete",
            post(handlers::admin::news_delete).delete(handlers::admin::news_delete),
        )
        .route_layer(middleware::from_fn_with_state(state.clone(), require_admin));

    Router::new()
        .merge(public_routes)
        .nest("/api/v1", api_routes)
        .nest("/api", internal_routes)
        .nest("/ai", ai_internal_routes)
        .merge(admin_routes)
        // Static assets (v2.1.0 PERF — 2 tầng cache):
        //   /static/fonts/* — cache 1 NĂM immutable: font là variable font
        //     self-hosted, tên file ổn định (inter-var-latin.woff2...), không
        //     bao giờ sửa in-place (muốn đổi font thì đổi tên file). Returning
        //     visitor bỏ hẳn 1 round-trip tải font (~100KB) — FCP nhanh hơn rõ.
        //   /static/* (css/js/img) — cache 30 ngày + stale-while-revalidate
        //     1 ngày (tăng từ 7 ngày v2.0.0): CSS/JS đổi qua cache-bust
        //     ?v=x.y.z trong URL nên kéo dài cache an toàn tuyệt đối.
        .nest_service(
            "/static/fonts",
            tower::ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::if_not_present(
                    axum::http::header::CACHE_CONTROL,
                    axum::http::HeaderValue::from_static("public, max-age=31536000, immutable"),
                ))
                .service(ServeDir::new("static/fonts")),
        )
        .nest_service(
            "/static",
            tower::ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::if_not_present(
                    axum::http::header::CACHE_CONTROL,
                    axum::http::HeaderValue::from_static(
                        "public, max-age=2592000, stale-while-revalidate=86400",
                    ),
                ))
                .service(ServeDir::new("static")),
        )
        // Uploaded files (avatar/cover/news/repo) — served from STORAGE_DIR.
        // Cache immutable (1 năm) vì filename là UUID — không bao giờ override
        // cùng URL. Coolify volume mount `khogame-storage:/app/storage`
        // (deploy/compose.prod.yml) ensures file tồn tại qua container
        // restart/redeploy.
        .nest_service(
            "/uploads",
            tower::ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::if_not_present(
                    axum::http::header::CACHE_CONTROL,
                    axum::http::HeaderValue::from_static("public, max-age=31536000, immutable"),
                ))
                .service(ServeDir::new(
                    std::env::var("STORAGE_DIR")
                        .ok()
                        .filter(|s| !s.is_empty())
                        .map(std::path::PathBuf::from)
                        .unwrap_or_else(|| std::path::PathBuf::from("storage")),
                )),
        )
        .route("/rss.xml", get(handlers::api::rss))
        .route("/news.rss", get(handlers::api::news_rss))
        .route(
            "/opensearch-suggest",
            get(handlers::api::opensearch_suggestions),
        )
        .route("/sitemap.xml", get(handlers::api::sitemap))
        .route("/robots.txt", get(handlers::api::robots))
        .route("/opensearch.xml", get(handlers::api::opensearch))
        .route("/manifest.json", get(handlers::api::manifest))
        .route(
            "/.well-known/security.txt",
            get(handlers::api::security_txt),
        )
        .fallback(handlers::pages::not_found)
        // Layer ordering (outermost LAST):
        //   1. rate_limit          (innermost — quyết định per-path/IP, có thể 429)
        //   2. origin_check        (v2.1.0 — CSRF: verify Origin cho unsafe method)
        //   3. error_page_mw       (v2.1.0 — render trang lỗi full UI cho browser)
        //   4. maintenance_guard    (có thể 503 khi bảo trì)
        //   5. security_headers     (áp dụng cho MỌI response, kể cả 429/503)
        //   6. PropagateRequestIdLayer
        //   7. TraceLayer
        //   8. SetRequestIdLayer    (set x-request-id đầu tiên cho các layer khác log)
        //   9. CompressionLayer    (outermost — nén body cuối cùng)
        //
        // TRƯỚC ĐÂY (bug): security_headers được `.layer()` đầu tiên → thành
        // INNERMOST. Khi rate_limit short-circuit trả 429, response KHÔNG
        // qua security_headers → thiếu X-Frame-Options, CSP, HSTS trên 429.
        // Tương tự 503 maintenance thiếu headers. Bất kỳ XSS/iframe-embedding
        // nào qua response 429/503 sẽ không bị CSP chặn.
        // FIX: đặt security_headers SAU rate_limit + maintenance_guard (outer
        // hơn) để mọi response — kể cả short-circuit 429/503 — đều có security
        // headers. Comment cũ "đặt ngoài cùng" đã nói đúng ý nhưng code sai
        // thứ tự `.layer()` (layer gọi sau sẽ là outer).
        .layer(middleware::from_fn_with_state(state.clone(), rate_limit))
        // v2.1.0 — CSRF defense-in-depth: verify Origin/Referer cho mọi
        // POST/PUT/PATCH/DELETE. Đặt NGOÀI rate_limit để request bị chặn
        // CSRF không tốn quota rate-limit; đặt TRONG error_page_mw để
        // lỗi 403 từ check này được render thành trang đầy đủ cho browser.
        .layer(middleware::from_fn_with_state(state.clone(), origin_check))
        // v2.1.0 — render trang lỗi đầy đủ giao diện cho browser navigation
        // (fix bug "HTML thuần" khi bấm link 404/403). Xem middleware.rs.
        .layer(middleware::from_fn_with_state(state.clone(), error_page_mw))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            maintenance_guard,
        ))
        .layer(middleware::from_fn(security_headers))
        .layer(PropagateRequestIdLayer::new(
            axum::http::HeaderName::from_static("x-request-id"),
        ))
        .layer(TraceLayer::new_for_http())
        .layer(SetRequestIdLayer::new(
            axum::http::HeaderName::from_static("x-request-id"),
            MakeRequestUuid,
        ))
        .layer(CompressionLayer::new())
        .with_state(state)
}
