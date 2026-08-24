use crate::models::*;
use askama::Template;

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
    AiLoginTemplate,
    AiProfileEditTemplate,
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
    AdminGamesTemplate,
    AdminUsersTemplate,
    AdminCommentsTemplate,
    AdminCategoriesTemplate,
    AdminReposTemplate,
    AdminSettingsTemplate,
    AdminAuditTemplate,
    AdminAiAgentsTemplate,
    AdminAiReportsTemplate,
    RepoListTemplate,
    RepoNewTemplate,
    MyGamesTemplate,
    ErrorTemplate,
    CategoriesPageTemplate,
    TermsPageTemplate,
    PrivacyPageTemplate,
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
}

/// AI Agent login page (nhập API token)
#[derive(Template)]
#[template(path = "auth/ai_login.html")]
pub struct AiLoginTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    /// Optional `next` path để redirect sau khi đăng nhập.
    pub next: Option<String>,
}

/// AI Agent profile edit (model_name, vendor, capabilities, ...)
#[derive(Template)]
#[template(path = "profile/ai_edit.html")]
pub struct AiProfileEditTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub profile: ai_agent::AiAgentProfile,
    pub privacy_public_label: &'static str,
    pub privacy_anonymous_label: &'static str,
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
    /// Base URL tuyệt đối (https://...) để dựng share URL đầy đủ.
    pub base_url: String,
    /// JSON-LD <script type="application/ld+json"> đã được serialize sẵn
    /// trong handler. Tránh phải lặp logic ở template (askama không có
    /// filter json_encode builtin ở 0.16).
    pub json_ld: String,
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
    /// Đường dẫn gốc của trang list để dựng link sort/pagination
    /// (vd /games/latest, /c/hanh-dong, /t/2d) — tránh link sort 404.
    pub base_url: String,
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
    /// Nếu user là AI Agent, đây là hồ sơ AI (model_name, vendor, ...).
    /// None nếu user thường.
    pub ai_profile: Option<ai_agent::AiAgentProfile>,
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
    pub recent_comments: Vec<comment::CommentWithGame>,
    pub daily_stats: Vec<settings::DailyStatRow>,
    pub total_repos: i64,
    pub pending_repos: i64,
    pub status_counts: Vec<StatusCountChip>,
    pub max_views: i64,
    pub max_downloads: i64,
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

/// Chip lọc trạng thái game cho trang admin (key + nhãn tiếng Việt + số lượng)
#[derive(Debug, Clone)]
pub struct StatusCountChip {
    pub key: String,
    pub label: String,
    pub count: i64,
}

/// Admin games
#[derive(Template)]
#[template(path = "admin/games.html")]
pub struct AdminGamesTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub games: Vec<game::AdminGameRow>,
    pub status_filter: Option<String>,
    pub status_counts: Vec<StatusCountChip>,
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
}

/// Admin users
#[derive(Template)]
#[template(path = "admin/users.html")]
pub struct AdminUsersTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub users: Vec<user::UserWithGameCount>,
    pub search: String,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

/// Admin comments
#[derive(Template)]
#[template(path = "admin/comments.html")]
pub struct AdminCommentsTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub comments: Vec<comment::CommentWithGame>,
}

/// Admin categories
#[derive(Template)]
#[template(path = "admin/categories.html")]
pub struct AdminCategoriesTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub categories: Vec<category::CategoryWithCount>,
}

/// Admin repos
#[derive(Template)]
#[template(path = "admin/repos.html")]
pub struct AdminReposTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub repos: Vec<repo::GithubRepoCard>,
    pub status_filter: Option<String>,
}

/// Admin settings
#[derive(Template)]
#[template(path = "admin/settings.html")]
pub struct AdminSettingsTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub site_name: String,
    pub site_description: String,
    pub maintenance_mode: bool,
    pub announcement: String,
    pub announcement_type: String,
    pub footer_text: String,
    pub repo_auto_approve: bool,
    pub saved: bool,
}

/// Admin audit log
#[derive(Template)]
#[template(path = "admin/audit.html")]
pub struct AdminAuditTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub logs: Vec<settings::AdminLogWithAdmin>,
}

/// Admin: danh sách AI Agent
#[derive(Template)]
#[template(path = "admin/ai_agents.html")]
pub struct AdminAiAgentsTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub agents: Vec<ai_agent::AiAgentWithProfile>,
}

/// Admin: live feed báo cáo tiến trình từ AI
#[derive(Template)]
#[template(path = "admin/ai_reports.html")]
pub struct AdminAiReportsTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub reports: Vec<ai_agent::AiProgressReportWithAgent>,
    pub total_agents: i64,
}

/// Danh sách GitHub repos
#[derive(Template)]
#[template(path = "repos/index.html")]
pub struct RepoListTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub repos: Vec<repo::GithubRepoCard>,
    pub total: i64,
    pub sort: String,
}

/// Form đăng repo
#[derive(Template)]
#[template(path = "repos/new.html")]
pub struct RepoNewTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
}

/// Game của tôi
#[derive(Template)]
#[template(path = "game/my_games.html")]
pub struct MyGamesTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub games: Vec<game::AdminGameRow>,
}

/// Error page (standalone, not extending layout)
#[derive(Template)]
#[template(path = "error.html")]
pub struct ErrorTemplate {
    pub status: u16,
    pub message: String,
    pub current_user: Option<user::User>,
}

/// Trang danh sách tất cả thể loại
#[derive(Template)]
#[template(path = "pages/categories.html")]
pub struct CategoriesPageTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub categories: Vec<category::CategoryWithCount>,
}

/// Trang điều khoản sử dụng
#[derive(Template)]
#[template(path = "pages/terms.html")]
pub struct TermsPageTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
}

/// Trang chính sách bảo mật
#[derive(Template)]
#[template(path = "pages/privacy.html")]
pub struct PrivacyPageTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
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
    /// Chỉ bật lazy-load replies cho bình luận cấp 1 (reply không có con)
    pub load_replies: bool,
}

#[derive(Template)]
#[template(path = "partials/comment_list.html")]
pub struct CommentListPartial<'a> {
    pub comments: &'a [comment::CommentWithUser],
    pub game_slug: &'a str,
    pub current_user: Option<&'a user::User>,
    /// Truyền xuống từng comment item (lazy-load replies cấp 1)
    pub load_replies: bool,
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
    /// URL tuyệt đối (vd https://domain.com/games/slug)
    pub share_url: String,
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

// Helper functions exposed to templates (Askama 0.16 custom filters)
pub mod filters {
    use crate::models::user::User;
    use crate::utils as u;
    use askama::filters::Safe;
    use askama::Values;
    use std::fmt::Display;

    #[askama::filter_fn]
    pub fn time_ago(s: impl AsRef<str>, _: &dyn Values) -> ::askama::Result<String> {
        let s = s.as_ref();
        let dt = chrono::DateTime::parse_from_rfc3339(s)
            .map_err(|e| ::askama::Error::Custom(e.into()))?;
        Ok(u::time_ago(dt.with_timezone(&chrono::Utc)))
    }

    /// Định dạng số lớn: 1200 -> 1.2K, 3400000 -> 3.4M
    #[askama::filter_fn]
    pub fn fmt_num(n: impl Display, _: &dyn Values) -> ::askama::Result<String> {
        let raw = n.to_string();
        let v: i64 = raw.trim().parse().unwrap_or(0);
        Ok(u::format_number_i64(v))
    }

    /// Số thập phân 1 chữ số: 4.33333 -> 4.3
    #[askama::filter_fn]
    pub fn fmt_f64(n: impl Display, _: &dyn Values) -> ::askama::Result<String> {
        let raw = n.to_string();
        let v: f64 = raw.trim().parse().unwrap_or(0.0);
        Ok(format!("{:.1}", v))
    }

    /// Markdown an toàn -> HTML (không escape lần 2)
    #[askama::filter_fn]
    pub fn html(s: impl AsRef<str>, _: &dyn Values) -> ::askama::Result<Safe<String>> {
        Ok(Safe(u::safe_markdown_to_html(s.as_ref())))
    }

    /// Escape HTML thủ công (không escape lần 2)
    #[askama::filter_fn]
    pub fn esc(s: impl AsRef<str>, _: &dyn Values) -> ::askama::Result<Safe<String>> {
        Ok(Safe(u::html_escape(s.as_ref())))
    }

    /// Escape HTML + đổi \\n thành <br> (cho bình luận)
    #[askama::filter_fn]
    pub fn nl2br(s: impl AsRef<str>, _: &dyn Values) -> ::askama::Result<Safe<String>> {
        Ok(Safe(u::html_escape(s.as_ref()).replace('\n', "<br>")))
    }

    /// Chữ cái đầu của tên (avatar fallback)
    #[askama::filter_fn]
    pub fn initials(name: impl AsRef<str>, _: &dyn Values) -> ::askama::Result<String> {
        Ok(u::initials(name.as_ref()))
    }

    /// Cắt chuỗi kèm "…"
    #[askama::filter_fn]
    pub fn truncate(s: impl AsRef<str>, _: &dyn Values, max: usize) -> ::askama::Result<String> {
        Ok(u::truncate(s.as_ref(), max))
    }

    /// Avatar của user hoặc placeholder
    #[askama::filter_fn]
    pub fn avatar_or(user: &User, _: &dyn Values) -> ::askama::Result<String> {
        Ok(user
            .avatar_url
            .clone()
            .unwrap_or_else(|| "/static/img/avatar-placeholder.svg".to_string()))
    }

    #[askama::filter_fn]
    pub fn slugify(s: impl AsRef<str>, _: &dyn Values) -> ::askama::Result<String> {
        Ok(slug::slugify(s.as_ref()))
    }

    #[askama::filter_fn]
    pub fn lower(s: impl AsRef<str>, _: &dyn Values) -> ::askama::Result<String> {
        Ok(s.as_ref().to_lowercase())
    }

    /// URL YouTube -> URL embed
    #[askama::filter_fn]
    pub fn youtube_embed(url: impl AsRef<str>, _: &dyn Values) -> ::askama::Result<String> {
        let id = u::extract_youtube_id(url.as_ref()).unwrap_or_default();
        Ok(format!("https://www.youtube.com/embed/{}", id))
    }

    #[askama::filter_fn]
    pub fn format_date(
        date: &Option<chrono::NaiveDate>,
        _: &dyn Values,
    ) -> ::askama::Result<String> {
        Ok(date
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_default())
    }

    #[askama::filter_fn]
    pub fn format_date_vn(
        date: &Option<chrono::NaiveDate>,
        _: &dyn Values,
    ) -> ::askama::Result<String> {
        Ok(date
            .map(|d| d.format("%d/%m/%Y").to_string())
            .unwrap_or_default())
    }

    #[askama::filter_fn]
    pub fn format_datetime_vn(
        dt: &chrono::DateTime<chrono::Utc>,
        _: &dyn Values,
    ) -> ::askama::Result<String> {
        Ok(dt.format("%d/%m/%Y").to_string())
    }

    #[askama::filter_fn]
    pub fn join_tags(items: &[String], _: &dyn Values) -> ::askama::Result<String> {
        Ok(items.join(", "))
    }
}
