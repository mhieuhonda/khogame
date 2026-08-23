use crate::handlers;
use crate::middleware::require_admin;
use crate::state::AppState;
use axum::middleware;
use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

pub fn build_router(state: Arc<AppState>) -> Router {
    let public_routes = Router::new()
        .route("/", get(handlers::games::home))
        .route("/login", get(handlers::auth::login_page))
        .route("/auth/google", get(handlers::auth::google_login))
        .route("/auth/google/callback", get(handlers::auth::google_callback))
        .route("/auth/logout", post(handlers::auth::logout))
        .route("/games/new", get(handlers::games::new_game_form))
        .route("/games", post(handlers::games::create_game))
        .route("/games/latest", get(handlers::games::list_latest))
        .route("/games/trending", get(handlers::games::list_trending))
        .route("/games/top-rated", get(handlers::games::list_top_rated))
        .route("/games/downloads", get(handlers::games::list_downloads))
        .route("/games/featured", get(handlers::games::list_featured))
        .route("/games/{slug}", get(handlers::games::show_game))
        .route("/games/{slug}/edit", get(handlers::games::edit_game_form))
        .route("/games/{slug}", post(handlers::games::update_game).delete(handlers::games::delete_game))
        .route("/games/{slug}/delete", post(handlers::games::delete_game))
        .route("/games/{slug}/download", post(handlers::games::download_game))
        .route("/games/{slug}/report-form", get(handlers::games::report_form))
        .route("/games/{slug}/report", post(handlers::games::submit_report))
        .route("/games/{slug}/like", post(handlers::interactions::toggle_like))
        .route("/games/{slug}/bookmark", post(handlers::interactions::toggle_bookmark))
        .route("/games/{slug}/rate", post(handlers::interactions::rate))
        .route("/games/{slug}/share", post(handlers::games::share_game))
        .route("/games/{slug}/comments", post(handlers::comments::create_comment))
        .route("/comments/{id}", post(handlers::comments::like_comment).delete(handlers::comments::delete_comment))
        .route("/comments/{id}/like", post(handlers::comments::like_comment))
        .route("/comments/{id}/replies", get(handlers::comments::list_replies))
        .route("/u/{username}", get(handlers::profile::show_profile))
        .route("/u/{username}/follow", post(handlers::interactions::toggle_follow))
        .route("/categories", get(handlers::games::list_categories))
        .route("/c/{slug}", get(handlers::games::list_by_category))
        .route("/t/{slug}", get(handlers::games::list_by_tag))
        .route("/search", get(handlers::games::search))
        .route("/profile", get(handlers::profile::my_profile))
        .route("/profile/edit", get(handlers::profile::edit_profile_form))
        .route("/profile", post(handlers::profile::update_profile))
        .route("/bookmarks", get(handlers::profile::bookmarks_page))
        .route("/notifications", get(handlers::notifications::list))
        .route("/notifications/{id}/read", post(handlers::notifications::mark_read))
        .route("/notifications/mark-all-read", post(handlers::notifications::mark_all_read))
        .route("/terms", get(handlers::pages::terms))
        .route("/privacy", get(handlers::pages::privacy))
        .route("/health", get(handlers::pages::health));

    let admin_routes = Router::new()
        .route("/admin", get(handlers::admin::dashboard))
        .route("/admin/reports", get(handlers::admin::reports))
        .route("/admin/reports/{id}/resolve", post(handlers::admin::resolve_report))
        .route("/admin/games/{id}/hide", post(handlers::admin::hide_game))
        .route("/admin/games/{id}/feature", post(handlers::admin::feature_game))
        .route("/admin/comments/{id}/pin", post(crate::handlers::admin::pin_comment))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_admin));

    Router::new()
        .merge(public_routes)
        .merge(admin_routes)
        .nest_service("/static", ServeDir::new("static"))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .with_state(state)
}
