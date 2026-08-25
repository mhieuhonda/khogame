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
    AdminSessionsTemplate,
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
    /// Số nền tảng tải game (Platform::all().len()) — hero hiển thị số
    /// thật thay vì hardcode "5" (thêm nền tảng mới là hero tự cập nhật).
    pub platforms_count: usize,
    /// JSON-LD WebSite schema (search action) — cho Google hiển thị
    /// sitelinks searchbox trên kết quả tìm kiếm.
    pub json_ld: String,
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
    /// JSON-LD \<script type="application/ld+json"\> đã được serialize sẵn
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
    /// URL gốc của site (https://...) để dựng canonical tuyệt đối
    pub site_url: String,
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
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
}

/// Trang comment cho load-more (GET /games/{slug}/comments?page=N)
#[derive(Template)]
#[template(path = "partials/comments_page.html")]
pub struct CommentsPageTemplate {
    pub current_user: Option<user::User>,
    pub comments: Vec<comment::CommentWithUser>,
    pub game_slug: String,
    pub page: i64,
    pub has_more: bool,
    pub remaining: i64,
}

/// Notifications
#[derive(Template)]
#[template(path = "notifications/index.html")]
pub struct NotificationsTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub notifications: Vec<notification::NotificationWithActor>,
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
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
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
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
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
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
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
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
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
}

/// Admin: quản lý phiên đăng nhập đang hoạt động
#[derive(Template)]
#[template(path = "admin/sessions.html")]
pub struct AdminSessionsTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub sessions: Vec<settings::SessionRow>,
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
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
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
#[template(path = "partials/report_modal.html")]
pub struct ReportModalPartial<'a> {
    pub slug: &'a str,
}

/// Base URL tuyệt đối của site — set MỘT lần lúc startup (run()) để
/// filter `abs_url` trong template dựng URL đầy đủ cho thẻ meta OG/Twitter
/// (crawler Facebook/Twitter/Telegram không tự resolve URL tương đối).
static SITE_BASE_URL: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Khởi tạo base URL cho các filter template. Gọi một lần trong run().
/// Nếu không gọi (unit test), abs_url trả về path gốc không đổi.
pub fn init_base_url(base: &str) {
    let _ = SITE_BASE_URL.set(base.trim_end_matches('/').to_string());
}

// Helper functions exposed to templates (Askama 0.16 custom filters)
pub mod filters {
    use crate::models::user::User;
    use crate::utils as u;
    use askama::filters::Safe;
    use askama::Values;
    use std::fmt::Display;

    /// Ghép base URL site vào path tương đối → URL tuyệt đối. Dùng cho
    /// og:image / twitter:image (crawler không chấp nhận path tương đối).
    #[askama::filter_fn]
    pub fn abs_url(s: impl AsRef<str>, _: &dyn Values) -> ::askama::Result<String> {
        let base = super::SITE_BASE_URL.get().map(|b| b.as_str()).unwrap_or("");
        Ok(format!("{}{}", base, s.as_ref()))
    }

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

    /// URL YouTube -> URL embed. Trả về chuỗi RỖNG nếu không tách được
    /// ID (URL không phải YouTube) — template dùng điều kiện này để
    /// fallback sang link thường thay vì render iframe hỏng
    /// (https://www.youtube.com/embed/ trống trắng).
    #[askama::filter_fn]
    pub fn youtube_embed(url: impl AsRef<str>, _: &dyn Values) -> ::askama::Result<String> {
        let id = u::extract_youtube_id(url.as_ref()).unwrap_or_default();
        if id.is_empty() {
            return Ok(String::new());
        }
        // youtube-nocookie.com: chế độ privacy-enhanced của YouTube —
        // không set cookie tracking cho người xem chưa bấm play.
        Ok(format!("https://www.youtube-nocookie.com/embed/{}", id))
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

#[cfg(test)]
mod filter_tests {
    // #[askama::filter_fn] sinh STRUCT có method execute(input, env)
    // (không phải hàm gọi trực tiếp). Test qua đường này.
    use super::filters;

    #[test]
    fn test_fmt_num_formats() {
        let out = filters::fmt_num::default().execute(999, &()).unwrap();
        assert_eq!(out, "999");
        let out = filters::fmt_num::default().execute(1500, &()).unwrap();
        assert_eq!(out, "1.5K");
        let out = filters::fmt_num::default().execute(2_500_000, &()).unwrap();
        assert_eq!(out, "2.5M");
        // Chuỗi số (Display) cũng được
        let out = filters::fmt_num::default().execute("1200", &()).unwrap();
        assert_eq!(out, "1.2K");
    }

    #[test]
    fn test_fmt_f64_one_decimal() {
        let out = filters::fmt_f64::default().execute(4.33333, &()).unwrap();
        assert_eq!(out, "4.3");
        let out = filters::fmt_f64::default().execute(0, &()).unwrap();
        assert_eq!(out, "0.0");
    }

    #[test]
    fn test_esc_xss() {
        let out = filters::esc::default()
            .execute("<script>alert(1)</script>", &())
            .unwrap();
        assert!(!out.to_string().contains("<script>"));
    }

    #[test]
    fn test_nl2br() {
        let out = filters::nl2br::default()
            .execute("dòng 1\ndòng 2", &())
            .unwrap();
        assert!(out.to_string().contains("dòng 1<br>dòng 2"));
    }

    #[test]
    fn test_initials_filter() {
        let out = filters::initials::default()
            .execute("Nguyễn Văn A", &())
            .unwrap();
        assert_eq!(out, "NA");
        let out = filters::initials::default().execute("", &()).unwrap();
        assert_eq!(out, "?");
    }

    #[test]
    fn test_slugify_filter_vi() {
        let out = filters::slugify::default().execute("Hà Nội", &()).unwrap();
        assert_eq!(out, "ha-noi");
    }

    /// Slug là URL của MỌI game/category/tag — test giới hạn tổng quát hơn:
    /// dấu tiếng Việt bỏ hết, emoji rơi, ký tự đặc biệt thành separator.
    #[test]
    fn test_slugify_filter_vi_full() {
        assert_eq!(
            filters::slugify::default()
                .execute("Trận Chiến Thượng Ny", &())
                .unwrap(),
            "tran-chien-thuong-ny"
        );
        assert_eq!(
            filters::slugify::default()
                .execute("Đế Chế 2026!", &())
                .unwrap(),
            "de-che-2026"
        );
        // Emoji bị loại — slug chỉ [a-z0-9-]
        let s = filters::slugify::default()
            .execute("🎮 Game Siêu Hay 🎮", &())
            .unwrap();
        assert!(
            s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "slug phải ascii: {}",
            s
        );
        // Chuỗi rỗng / chỉ ký tự đặc biệt → rỗng (caller có fallback 'game')
        assert_eq!(filters::slugify::default().execute("   ", &()).unwrap(), "");
        assert_eq!(filters::slugify::default().execute("!!!", &()).unwrap(), "");
    }

    #[test]
    fn test_youtube_embed_valid() {
        let out = filters::youtube_embed::default()
            .execute("https://www.youtube.com/watch?v=abc123", &())
            .unwrap();
        // nocookie domain: privacy-enhanced, phải khớp CSP frame-src
        assert_eq!(out, "https://www.youtube-nocookie.com/embed/abc123");
    }

    #[test]
    fn test_youtube_embed_non_youtube_empty() {
        // URL không phải YouTube → chuỗi rỗng (template fallback link)
        let out = filters::youtube_embed::default()
            .execute("https://example.com/video.mp4", &())
            .unwrap();
        assert_eq!(out, "");
    }

    #[test]
    fn test_lower_filter() {
        let out = filters::lower::default().execute("ABC def", &()).unwrap();
        assert_eq!(out, "abc def");
    }

    #[test]
    fn test_join_tags_filter() {
        let tags = ["action".to_string(), "rpg".to_string()];
        let out = filters::join_tags::default()
            .execute(&tags[..], &())
            .unwrap();
        assert_eq!(out, "action, rpg");
    }

    #[test]
    fn test_html_filter_markdown() {
        let out = filters::html::default().execute("**đậm**", &()).unwrap();
        assert!(out.to_string().contains("<strong>đậm</strong>"));
    }

    #[test]
    fn test_truncate_filter_with_arg() {
        // truncate có required arg `max` — set qua builder .max(5)
        let out = filters::truncate::default()
            .with_max(5)
            .execute("hello world", &())
            .unwrap();
        assert_eq!(out, "hello…");
    }

    #[test]
    fn test_format_date_filters() {
        let d = chrono::NaiveDate::from_ymd_opt(2026, 8, 25).unwrap();
        let out = filters::format_date::default()
            .execute(&Some(d), &())
            .unwrap();
        assert_eq!(out, "2026-08-25");
        let out = filters::format_date_vn::default()
            .execute(&Some(d), &())
            .unwrap();
        assert_eq!(out, "25/08/2026");
        let out = filters::format_date::default().execute(&None, &()).unwrap();
        assert_eq!(out, "");
    }

    #[test]
    fn test_time_ago_filter_parses_rfc3339() {
        // Template luôn truyền .to_rfc3339() — filter phải parse được
        let now = chrono::Utc::now().to_rfc3339();
        let out = filters::time_ago::default()
            .execute(now.as_str(), &())
            .unwrap();
        assert_eq!(out, "vừa xong");
        // 2 giờ trước
        let two_h = (chrono::Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
        let out = filters::time_ago::default()
            .execute(two_h.as_str(), &())
            .unwrap();
        assert!(out.contains("giờ trước"), "got: {}", out);
    }

    #[test]
    fn test_time_ago_filter_rejects_garbage() {
        // Chuỗi không phải RFC3339 → Err (template render 500 — nhưng chí
        // ít không panic; assertion chỉ ghi nhận hành vi)
        let out = filters::time_ago::default().execute("not-a-date", &());
        assert!(out.is_err());
    }

    #[test]
    fn test_avatar_or_fallback() {
        use crate::models::user::User;
        // Không cần DB — dựng struct thủ công qua serde_json cho gọn
        let json = r#"{
            "id": "00000000-0000-0000-0000-000000000000",
            "email": "a@b.c", "username": "u", "display_name": "U",
            "avatar_url": null, "bio": null, "google_sub": "s",
            "role": "User", "is_banned": false, "last_seen_at": null,
            "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"
        }"#;
        let user: User = serde_json::from_str(json).unwrap();
        let out = filters::avatar_or::default().execute(&user, &()).unwrap();
        assert_eq!(out, "/static/img/avatar-placeholder.svg");
    }

    #[test]
    fn test_format_datetime_vn_filter() {
        let dt = chrono::DateTime::parse_from_rfc3339("2026-08-25T10:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let out = filters::format_datetime_vn::default()
            .execute(&dt, &())
            .unwrap();
        assert_eq!(out, "25/08/2026");
    }
}
