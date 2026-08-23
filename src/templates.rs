use askama::Template;
use crate::models::*;
use crate::utils;

/// Implement `axum::response::IntoResponse` for a template type by rendering it.
macro_rules! impl_template_response {
    ($($t:ty),* $(,)?) => {
        $(
            impl axum::response::IntoResponse for $t {
                fn into_response(self) -> axum::response::Response {
                    match self.render() {
                        Ok(html) => axum::response::Html(html).into_response(),
                        Err(e) => {
                            tracing::error!("Template render error: {:?}", e);
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
                        }
                    }
                }
            }
        )*
    };
}

impl_template_response!(
    IndexTemplate,
    LoginTemplate,
    NewGameTemplate,
    EditGameTemplate,
    GameShowTemplate,
    SearchTemplate,
    GameListTemplate,
    ProfileTemplate,
    EditProfileTemplate,
    BookmarksTemplate,
    NotificationsTemplate,
    AdminTemplate,
    AdminReportsTemplate,
    ErrorTemplate,
);

/// Home page
#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub featured_games: Vec<GameCard>,
    pub latest_games: Vec<GameCard>,
    pub trending_games: Vec<GameCard>,
    pub top_rated_games: Vec<GameCard>,
    pub categories: Vec<category::CategoryWithCount>,
    pub popular_tags: Vec<tag::Tag>,
    pub total_games: i64,
}

/// Login page
#[derive(Template)]
#[template(path = "auth/login.html")]
pub struct LoginTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub auth_url: String,
}

/// New game form
#[derive(Template)]
#[template(path = "game/new.html")]
pub struct NewGameTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub categories: Vec<category::Category>,
}

/// Edit game form
#[derive(Template)]
#[template(path = "game/edit.html")]
pub struct EditGameTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub categories: Vec<category::Category>,
    pub game: game::Game,
    pub links: Vec<game::GameLink>,
    pub screenshots: Vec<game::GameScreenshot>,
    pub tags: Vec<String>,
}

/// Game detail page
#[derive(Template)]
#[template(path = "game/show.html")]
pub struct GameShowTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub game: game::Game,
    pub author: user::User,
    pub links: Vec<game::GameLink>,
    pub screenshots: Vec<game::GameScreenshot>,
    pub tags: Vec<String>,
    pub category: Option<category::Category>,
    pub comments: Vec<comment::CommentWithUser>,
    pub related_games: Vec<GameCard>,
    pub is_liked: bool,
    pub is_bookmarked: bool,
    pub is_following_author: bool,
    pub is_owner: bool,
    pub user_rating: Option<i16>,
}

/// Search results
#[derive(Template)]
#[template(path = "game/search.html")]
pub struct SearchTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub query: String,
    pub sort: String,
    pub platform: Option<String>,
    pub category_slug: Option<String>,
    pub games: Vec<GameCard>,
    pub categories: Vec<category::Category>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

/// Category listing
#[derive(Template)]
#[template(path = "game/list.html")]
pub struct GameListTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub title: String,
    pub games: Vec<GameCard>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
    pub sort: String,
    pub list_type: String,
    pub category: Option<category::Category>,
    pub tag: Option<tag::Tag>,
}

/// Profile page
#[derive(Template)]
#[template(path = "profile/show.html")]
pub struct ProfileTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub user: user::User,
    pub stats: user::UserStats,
    pub games: Vec<GameCard>,
    pub is_following: bool,
    pub is_self: bool,
    pub preferences: user::UserPreference,
}

/// Edit profile
#[derive(Template)]
#[template(path = "profile/edit.html")]
pub struct EditProfileTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub preferences: user::UserPreference,
}

/// Bookmarks page
#[derive(Template)]
#[template(path = "profile/bookmarks.html")]
pub struct BookmarksTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub games: Vec<GameCard>,
}

/// Notifications
#[derive(Template)]
#[template(path = "notifications/index.html")]
pub struct NotificationsTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub notifications: Vec<notification::NotificationWithActor>,
}

/// Admin dashboard
#[derive(Template)]
#[template(path = "admin/index.html")]
pub struct AdminTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub total_games: i64,
    pub total_users: i64,
    pub total_downloads: i64,
    pub pending_reports: i64,
    pub recent_reports: Vec<report::ReportWithGame>,
    pub recent_games: Vec<GameCard>,
}

/// Admin reports
#[derive(Template)]
#[template(path = "admin/reports.html")]
pub struct AdminReportsTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub reports: Vec<report::ReportWithGame>,
    pub status_filter: Option<String>,
}

/// Error page (standalone, not extending layout)
#[derive(Template)]
#[template(path = "error.html")]
pub struct ErrorTemplate {
    pub status: u16,
    pub message: String,
    pub current_user: Option<user::User>,
}

// ============= HTMX partials =============

#[derive(Template)]
#[template(path = "partials/game_card.html")]
pub struct GameCardPartial<'a> {
    pub game: &'a GameCard,
    pub current_user: Option<&'a user::User>,
}

#[derive(Template)]
#[template(path = "partials/game_grid.html")]
pub struct GameGridPartial<'a> {
    pub games: &'a [GameCard],
    pub current_user: Option<&'a user::User>,
}

#[derive(Template)]
#[template(path = "partials/like_button.html")]
pub struct LikeButtonPartial {
    pub game_id: uuid::Uuid,
    pub slug: String,
    pub is_liked: bool,
    pub like_count: i32,
}

#[derive(Template)]
#[template(path = "partials/bookmark_button.html")]
pub struct BookmarkButtonPartial {
    pub game_id: uuid::Uuid,
    pub slug: String,
    pub is_bookmarked: bool,
}

#[derive(Template)]
#[template(path = "partials/comment_item.html")]
pub struct CommentItemPartial<'a> {
    pub comment: &'a comment::CommentWithUser,
    pub game_slug: &'a str,
    pub current_user: Option<&'a user::User>,
}

#[derive(Template)]
#[template(path = "partials/comment_list.html")]
pub struct CommentListPartial<'a> {
    pub comments: &'a [comment::CommentWithUser],
    pub game_slug: &'a str,
    pub current_user: Option<&'a user::User>,
}

#[derive(Template)]
#[template(path = "partials/comment_form.html")]
pub struct CommentFormPartial<'a> {
    pub game_slug: &'a str,
    pub parent_id: Option<uuid::Uuid>,
}

#[derive(Template)]
#[template(path = "partials/rating_stars.html")]
pub struct RatingStarsPartial {
    pub game_id: uuid::Uuid,
    pub slug: String,
    pub user_rating: Option<i16>,
    pub rating_avg: f64,
    pub rating_count: i32,
}

#[derive(Template)]
#[template(path = "partials/follow_button.html")]
pub struct FollowButtonPartial {
    pub target_user_id: uuid::Uuid,
    pub target_username: String,
    pub is_following: bool,
}

#[derive(Template)]
#[template(path = "partials/notification_item.html")]
pub struct NotificationItemPartial<'a> {
    pub notification: &'a notification::NotificationWithActor,
}

#[derive(Template)]
#[template(path = "partials/error.html")]
pub struct ErrorPartial {
    pub message: String,
    pub status: u16,
}

#[derive(Template)]
#[template(path = "partials/download_buttons.html")]
pub struct DownloadButtonsPartial<'a> {
    pub links: &'a [game::GameLink],
    pub slug: &'a str,
    pub is_authenticated: bool,
}

#[derive(Template)]
#[template(path = "partials/share_buttons.html")]
pub struct ShareButtonsPartial<'a> {
    pub slug: &'a str,
    pub title: &'a str,
    pub base_url: String,
}

#[derive(Template)]
#[template(path = "partials/report_modal.html")]
pub struct ReportModalPartial<'a> {
    pub slug: &'a str,
}

#[derive(Template)]
#[template(path = "partials/empty_state.html")]
pub struct EmptyStatePartial {
    pub icon: String,
    pub title: String,
    pub message: String,
}

#[derive(Template)]
#[template(path = "partials/pagination.html")]
pub struct PaginationPartial {
    pub current: i64,
    pub total_pages: i64,
    pub base_url: String,
}

// Helper functions exposed to templates
pub mod filters {
    use crate::utils as u;
    use crate::models::user::User;

    pub fn time_ago<S: AsRef<str>>(s: S) -> ::askama::Result<String> {
        let s = s.as_ref();
        let dt = chrono::DateTime::parse_from_rfc3339(s)
            .map_err(|e| ::askama::Error::Custom(e.into()))?;
        Ok(u::time_ago(dt.with_timezone(&chrono::Utc)))
    }

    pub fn fmt_num(n: &i32) -> ::askama::Result<String> {
        Ok(u::format_number(*n))
    }

    pub fn fmt_f64(n: f64) -> ::askama::Result<String> {
        Ok(format!("{:.1}", n))
    }

    pub fn html<S: AsRef<str>>(s: S) -> ::askama::Result<String> {
        Ok(u::safe_markdown_to_html(s.as_ref()))
    }

    pub fn esc<S: AsRef<str>>(s: S) -> ::askama::Result<String> {
        Ok(u::html_escape(s.as_ref()))
    }

    pub fn initials<S: AsRef<str>>(name: S) -> ::askama::Result<String> {
        Ok(u::initials(name.as_ref()))
    }

    pub fn truncate<S: AsRef<str>>(s: S, max: usize) -> ::askama::Result<String> {
        Ok(u::truncate(s.as_ref(), max))
    }

    pub fn avatar_or(user: &User) -> ::askama::Result<String> {
        Ok(user.avatar_url.clone().unwrap_or_else(|| {
            "/static/img/avatar-placeholder.svg".to_string()
        }))
    }

    pub fn slugify<S: AsRef<str>>(s: S) -> ::askama::Result<String> {
        Ok(slug::slugify(s.as_ref()))
    }

    pub fn lower<S: AsRef<str>>(s: S) -> ::askama::Result<String> {
        Ok(s.as_ref().to_lowercase())
    }

    pub fn youtube_embed<S: AsRef<str>>(url: S) -> ::askama::Result<String> {
        let id = u::extract_youtube_id(url.as_ref()).unwrap_or_default();
        Ok(format!("https://www.youtube.com/embed/{}", id))
    }

    pub fn format_date(date: &Option<chrono::NaiveDate>) -> ::askama::Result<String> {
        Ok(date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default())
    }

    pub fn format_date_vn(date: &Option<chrono::NaiveDate>) -> ::askama::Result<String> {
        Ok(date.map(|d| d.format("%d/%m/%Y").to_string()).unwrap_or_default())
    }

    pub fn format_datetime_vn(dt: &chrono::DateTime<chrono::Utc>) -> ::askama::Result<String> {
        Ok(dt.format("%d/%m/%Y").to_string())
    }

    pub fn join_tags(items: &Vec<String>) -> ::askama::Result<String> {
        Ok(items.join(", "))
    }
}
