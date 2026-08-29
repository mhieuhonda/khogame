use crate::models::{
    ai_agent, category, comment, game, news, notification, repo, report, settings,
    social::SocialPlatform, tag, user, GameCard, NewsStatus, SocialLinks,
};
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
    // News module
    NewsListTemplate,
    NewsShowTemplate,
    NewsNewTemplate,
    NewsEditTemplate,
    MyNewsTemplate,
    AdminNewsPendingTemplate,
    AdminNewsAllTemplate,
    // Admin user detail (chỉ admin, không phải mod)
    AdminUserDetailTemplate,
    // v1.4.0 — admin news categories (CRUD)
    AdminNewsCategoriesTemplate,
    // v2.9.0 — gamification + collections + following + sessions
    LeaderboardTemplate,
    AchievementsTemplate,
    FollowingTemplate,
    MyCollectionsTemplate,
    CollectionShowTemplate,
    ProfileSessionsTemplate,
    AdminAchievementsTemplate,
    // v3.0.0 — retention pages
    QuestsTemplate,
    SpinTemplate,
    TriviaTemplate,
    ShopTemplate,
    ReferralTemplate,
    NotifPrefsTemplate,
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
    /// Số nền tảng tải game (`Platform::all().len()`) — hero hiển thị số
    /// thật thay vì hardcode "5" (thêm nền tảng mới là hero tự cập nhật).
    pub platforms_count: usize,
    /// JSON-LD `WebSite` schema (search action) — cho Google hiển thị
    /// sitelinks searchbox trên kết quả tìm kiếm.
    pub json_ld: String,
    /// 3 tin tức nổi bật mới nhất để hiển thị section "Tin tức" ở homepage
    pub latest_news: Vec<news::NewsWithAuthor>,
    pub total_news: i64,
    /// v2.3.0 — Top GitHub repos (approved, sắp xếp theo stars desc) để
    /// hiển thị section "Repo đề xuất" ở homepage. Lấy 8 repo đầu, nếu
    /// không có repo nào approved → section tự ẩn (template dùng
    /// `{% if !featured_repos.is_empty() %}`).
    pub featured_repos: Vec<repo::GithubRepoCard>,
    /// v2.9.0 — "Tiếp tục xem" (rỗng nếu chưa login / chưa xem game nào).
    pub continue_watching: Vec<GameCard>,
    /// v2.9.0 — "Dành cho bạn" (theo thể loại game đã like/bookmark).
    pub recommended_games: Vec<GameCard>,
    /// v2.9.0 — "Game của tuần" (hot theo daily_stats 7 ngày).
    pub week_games: Vec<GameCard>,
    /// v3.0.0 — "Game của ngày" (deterministic theo ngày VN).
    pub game_of_the_day: Option<crate::models::retention::GameOfDay>,
    /// v3.0.0 — "Sắp ra mắt" (release_date >= hôm nay, gần nhất trước).
    pub upcoming_games: Vec<crate::models::retention::UpcomingGame>,
    /// v3.0.0 — Cảnh báo "sắp mất streak": Some(streak) khi user đã có
    /// chuỗi nhưng CHƯA điểm danh hôm nay. None khi an toàn / chưa login.
    pub streak_warning: Option<i32>,
    /// v3.0.0 — Widget onboarding (None nếu ẩn — user cũ đã xong).
    pub onboarding: Option<OnboardingWidget>,
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

/// AI Agent profile edit (`model_name`, vendor, capabilities, ...)
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
    /// Tổng comment GỐC (không gồm replies) — FIX v2.8.1: nút
    /// "Tải thêm" trước đây dùng game.comment_count (trigger đếm cả
    /// replies) → số "còn N" sai + nút treo khi hết comment gốc.
    pub comments_total: i64,
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
    /// filter `json_encode` builtin ở 0.16).
    pub json_ld: String,
    /// v2.9.0 — Reviews của game (kèm is_helpful + level tác giả).
    pub reviews: Vec<crate::models::review::ReviewWithUser>,
    /// v2.9.0 — Tổng số review.
    pub reviews_total: i64,
    /// v2.9.0 — Review của viewer (điền form sửa) — None nếu chưa review.
    pub my_review: Option<crate::models::review::Review>,
    /// v2.9.0 — Bộ sưu tập của viewer kèm cờ đã chứa game này hay chưa.
    pub my_collections: Vec<(uuid::Uuid, String, bool)>,
    /// v3.0.0 — "Người chơi khác cũng thích" (co-occurrence qua likes).
    pub also_liked: Vec<GameCard>,
    /// v3.0.0 — Số ngày còn lại tới release_date (None nếu đã ra mắt /
    /// không có ngày). >0 → hiển thị đếm ngược.
    pub release_countdown_days: Option<i64>,
    /// v3.0.0 — User đã tải nhưng CHƯA đánh giá → hiện prompt mời đánh giá.
    pub invite_rating: bool,
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
    /// Nếu user là AI Agent, đây là hồ sơ AI (`model_name`, vendor, ...).
    /// None nếu user thường.
    pub ai_profile: Option<ai_agent::AiAgentProfile>,
    /// v2.7.0 — Mạng xã hội của user (10 nền tảng, chỉ render link đã đặt).
    pub socials: SocialLinks,
    /// v2.9.0 — Cấp độ/XP của user hồ sơ (chip Lv + thanh tiến độ).
    pub level: crate::models::gamification::LevelInfo,
    /// v2.9.0 — Chuỗi điểm danh hiện tại.
    pub streak: i32,
    /// v2.9.0 — Huy hiệu đã đạt (showcase trước).
    pub achievements: Vec<crate::models::gamification::Achievement>,
    /// v2.9.0 — Huy hiệu đang ghim (is_showcased) — hiện to trên hồ sơ.
    pub showcased: Vec<crate::models::gamification::Achievement>,
    /// v2.9.0 — Tổng số huy hiệu đã đạt / tổng catalog.
    pub achievements_count: (usize, usize),
    /// v2.9.0 — Hoạt động gần đây (activity feed từ xp_events).
    pub activity: Vec<crate::models::gamification::ActivityEvent>,
    /// v2.9.0 — Bộ sưu tập công khai của user.
    pub collections: Vec<crate::repositories::collection::CollectionWithOwner>,
    /// v3.0.0 — Heatmap hoạt động 13 tuần.
    pub heatmap: crate::templates::HeatmapWidget,
    /// v3.0.0 — Độ hoàn thiện hồ sơ 0-100 (avatar+bio+socials).
    pub completeness_pct: i32,
    /// v3.0.0 — Năm tham gia (anniversary context) + số tháng thành viên.
    pub member_months: i64,
}

/// Edit profile
#[derive(Template)]
#[template(path = "profile/edit.html")]
pub struct EditProfileTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub preferences: user::UserPreference,
    /// v2.7.0 — Mạng xã hội hiện tại của user (điền value vào form).
    pub socials: SocialLinks,
    /// Danh sách 10 nền tảng hỗ trợ (id + label) để form edit loop.
    pub platforms: &'static [SocialPlatform],
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
    /// News stats cho admin dashboard
    pub pending_news: i64,
    pub total_news: i64,
    /// v1.4.0 — online users count (last_seen < 15 phút)
    pub online_users: i64,
    /// v1.4.0 — 5 user hoạt động gần đây (sidebar widget)
    pub recent_active_users: Vec<user::UserWithGameCount>,
    /// v1.4.0 — tổng user bị cấm
    pub banned_users: i64,
    /// v1.4.0 — tổng comment (insight)
    pub total_comments: i64,
    /// v1.4.0 — tổng view (SUM view_count trên games)
    pub total_views: i64,
    /// v2.9.0 — retention stats: điểm danh hôm nay
    pub checkins_today: i64,
    /// v2.9.0 — retention stats: huy hiệu trao hôm nay
    pub achievements_today: i64,
    /// v2.9.0 — retention stats: top 5 user theo XP
    pub top_xp_users: Vec<crate::models::gamification::LeaderboardEntry>,
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
    /// (key, label, count) cho chip filter — vd ("online", "Đang online", 5).
    /// Count được tính trong handler, không phải template, để tránh N+1 query.
    pub status_options: Vec<(&'static str, &'static str, i64)>,
    pub status_filter: String,
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

/// v1.4.0 — Admin news categories (CRUD). Tách riêng khỏi `AdminCategoriesTemplate`
/// (thể loại game) để admin có 2 trang khác nhau, 2 bảng khác nhau.
#[derive(Template)]
#[template(path = "admin/news_categories.html")]
pub struct AdminNewsCategoriesTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    /// Category + count số tin thuộc category (cho admin biết xoá có an toàn).
    pub categories: Vec<NewsCategoryWithCountView>,
}

/// Wrapper view cho `NewsCategoryWithCount` thêm `is_active_label` để
/// template không phải gọi method — Askama có gọi được nhưng bọc trong
/// struct dễ debug + format text inline.
#[derive(Debug, Clone)]
pub struct NewsCategoryWithCountView {
    pub id: uuid::Uuid,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub icon: String,
    pub sort_order: i32,
    pub is_active: bool,
    pub is_active_label: &'static str,
    pub news_count: i64,
}

impl From<crate::models::news_category::NewsCategoryWithCount> for NewsCategoryWithCountView {
    fn from(c: crate::models::news_category::NewsCategoryWithCount) -> Self {
        Self {
            id: c.id,
            name: c.name,
            slug: c.slug,
            description: c.description,
            icon: c.icon,
            sort_order: c.sort_order,
            is_active: c.is_active,
            is_active_label: if c.is_active { "Hiển thị" } else { "Ẩn" },
            news_count: c.news_count,
        }
    }
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
    /// Mã sự cố cho lỗi 5xx — user báo admin kèm mã này để tra log nhanh.
    pub request_id: Option<String>,
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
    /// Request ID cho lỗi 5xx — user báo admin kèm ID này để tra log.
    /// None với 4xx (validation) vì không cần log tra cứu.
    pub request_id: Option<String>,
}

#[derive(Template)]
#[template(path = "partials/report_modal.html")]
pub struct ReportModalPartial<'a> {
    pub slug: &'a str,
}

/// Base URL tuyệt đối của site — set MỘT lần lúc startup (`run()`) để
/// filter `abs_url` trong template dựng URL đầy đủ cho thẻ meta OG/Twitter
/// (crawler Facebook/Twitter/Telegram không tự resolve URL tương đối).
static SITE_BASE_URL: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Khởi tạo base URL cho các filter template. Gọi một lần trong `run()`.
/// Nếu không gọi (unit test), `abs_url` trả về path gốc không đổi.
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
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    /// v3.0.0 — cache-bust version tự động từ CARGO_PKG_VERSION. Dùng trong
    /// layout: `?v={{ ""|ver }}` — bump Cargo.toml là toàn site + Link
    /// preload (middleware) đồng bộ version, hết lỗi quên bump tay
    /// (bug v3.0.0: preload hardcode ?v=2.8.0 trong khi layout ?v=2.9.2).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    #[askama::filter_fn]
    pub fn ver(s: impl Display, _: &dyn Values) -> ::askama::Result<String> {
        // Input bị bỏ qua — chỉ cần 1 cách gọi ổn định từ template.
        let _ = &s;
        Ok(env!("CARGO_PKG_VERSION").to_string())
    }

    #[askama::filter_fn]
    pub fn abs_url(s: impl AsRef<str>, _: &dyn Values) -> ::askama::Result<String> {
        let base = super::SITE_BASE_URL
            .get()
            .map_or("", std::string::String::as_str);
        Ok(format!("{}{}", base, s.as_ref()))
    }

    #[askama::filter_fn]
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub fn time_ago(s: impl AsRef<str>, _: &dyn Values) -> ::askama::Result<String> {
        let s = s.as_ref();
        let dt = chrono::DateTime::parse_from_rfc3339(s)
            .map_err(|e| ::askama::Error::Custom(e.into()))?;
        Ok(u::time_ago(dt.with_timezone(&chrono::Utc)))
    }

    /// v2.2.0 — time_ago cho DateTime<Utc> trực tiếp (cho Option<DateTime>).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    #[askama::filter_fn]
    pub fn time_ago_dt(
        dt: &chrono::DateTime<chrono::Utc>,
        _: &dyn Values,
    ) -> ::askama::Result<String> {
        Ok(u::time_ago(*dt))
    }

    /// Định dạng số lớn: 1200 -> 1.2K, 3400000 -> 3.4M
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    #[askama::filter_fn]
    pub fn fmt_num(n: impl Display, _: &dyn Values) -> ::askama::Result<String> {
        let raw = n.to_string();
        let v: i64 = raw.trim().parse().unwrap_or(0);
        Ok(u::format_number_i64(v))
    }

    /// Số thập phân 1 chữ số: 4.33333 -> 4.3
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    #[askama::filter_fn]
    pub fn fmt_f64(n: impl Display, _: &dyn Values) -> ::askama::Result<String> {
        let raw = n.to_string();
        let v: f64 = raw.trim().parse().unwrap_or(0.0);
        Ok(format!("{v:.1}"))
    }

    /// v2.9.0 — Số cấp độ từ tổng XP (dùng trong template leaderboard/review).
    /// # Errors
    ///
    /// Trả lỗi khi input không parse được số.
    #[askama::filter_fn]
    pub fn level_for(n: impl Display, _: &dyn Values) -> ::askama::Result<String> {
        let raw = n.to_string();
        let v: i32 = raw.trim().parse().unwrap_or(0);
        Ok(crate::models::gamification::level_from_xp(v)
            .level
            .to_string())
    }

    /// v2.9.0 — Danh hiệu từ tổng XP.
    /// # Errors
    ///
    /// Trả lỗi khi input không parse được số.
    #[askama::filter_fn]
    pub fn title_for(n: impl Display, _: &dyn Values) -> ::askama::Result<String> {
        let raw = n.to_string();
        let v: i32 = raw.trim().parse().unwrap_or(0);
        Ok(crate::models::gamification::level_from_xp(v)
            .title
            .to_string())
    }

    /// v2.9.0 — Percent (0-100) của `count` so với `total` (tham số thứ 2)
    /// — thanh tiến độ admin achievements: `{{ a.1|pct(total_users) }}`.
    /// total = 0 → 0%.
    /// # Errors
    ///
    /// Trả lỗi khi thao tác thất bại.
    #[askama::filter_fn]
    pub fn pct(n: impl Display, _: &dyn Values, total: &i64) -> ::askama::Result<String> {
        let raw = n.to_string();
        let v: i64 = raw.trim().parse().unwrap_or(0);
        if *total <= 0 {
            return Ok("0".to_string());
        }
        Ok(((v * 100) / *total).clamp(0, 100).to_string())
    }

    /// Markdown an toàn -> HTML (không escape lần 2)
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    #[askama::filter_fn]
    pub fn html(s: impl AsRef<str>, _: &dyn Values) -> ::askama::Result<Safe<String>> {
        Ok(Safe(u::safe_markdown_to_html(s.as_ref())))
    }

    /// v2.5.0 — Markdown an toàn cho BIO hồ sơ cá nhân (pipeline rút gọn:
    /// không ToC/YouTube/callout/copy-btn — xem services::markdown::render_bio).
    /// Dùng trong templates/profile/show.html + admin/user_detail.html.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    #[askama::filter_fn]
    pub fn bio(s: impl AsRef<str>, _: &dyn Values) -> ::askama::Result<Safe<String>> {
        Ok(Safe(crate::services::markdown::render_bio(s.as_ref())))
    }

    /// v2.4.0 — Ước lượng thời gian đọc (phút) từ markdown source.
    /// Dùng cho badge "X phút đọc" ở đầu bài tin / game detail. Tính 200
    /// từ/phút (conservative cho tiếng Việt có dấu).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    #[askama::filter_fn]
    pub fn reading_time(s: impl AsRef<str>, _: &dyn Values) -> ::askama::Result<String> {
        let mins = crate::services::markdown::reading_time_minutes(s.as_ref());
        Ok(format!("{mins} phút đọc"))
    }

    /// Escape HTML thủ công (không escape lần 2)
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    #[askama::filter_fn]
    pub fn esc(s: impl AsRef<str>, _: &dyn Values) -> ::askama::Result<Safe<String>> {
        Ok(Safe(u::html_escape(s.as_ref())))
    }

    /// Escape HTML + đổi \\n thành <br> (cho bình luận)
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    #[askama::filter_fn]
    pub fn nl2br(s: impl AsRef<str>, _: &dyn Values) -> ::askama::Result<Safe<String>> {
        Ok(Safe(u::html_escape(s.as_ref()).replace('\n', "<br>")))
    }

    /// Chữ cái đầu của tên (avatar fallback)
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    #[askama::filter_fn]
    pub fn initials(name: impl AsRef<str>, _: &dyn Values) -> ::askama::Result<String> {
        Ok(u::initials(name.as_ref()))
    }

    /// Cắt chuỗi kèm "…"
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    #[askama::filter_fn]
    pub fn truncate(s: impl AsRef<str>, _: &dyn Values, max: usize) -> ::askama::Result<String> {
        Ok(u::truncate(s.as_ref(), max))
    }

    /// Avatar của user hoặc placeholder
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    #[askama::filter_fn]
    pub fn avatar_or(user: &User, _: &dyn Values) -> ::askama::Result<String> {
        Ok(user
            .avatar_url
            .clone()
            .unwrap_or_else(|| "/static/img/avatar-placeholder.svg".to_string()))
    }

    #[askama::filter_fn]
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub fn slugify(s: impl AsRef<str>, _: &dyn Values) -> ::askama::Result<String> {
        Ok(slug::slugify(s.as_ref()))
    }

    #[askama::filter_fn]
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub fn lower(s: impl AsRef<str>, _: &dyn Values) -> ::askama::Result<String> {
        Ok(s.as_ref().to_lowercase())
    }

    /// URL YouTube -> URL embed. Trả về chuỗi RỖNG nếu không tách được
    /// ID (URL không phải YouTube) — template dùng điều kiện này để
    /// fallback sang link thường thay vì render iframe hỏng
    /// (https://www.youtube.com/embed/ trống trắng).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    #[askama::filter_fn]
    pub fn youtube_embed(url: impl AsRef<str>, _: &dyn Values) -> ::askama::Result<String> {
        let id = u::extract_youtube_id(url.as_ref()).unwrap_or_default();
        if id.is_empty() {
            return Ok(String::new());
        }
        // youtube-nocookie.com: chế độ privacy-enhanced của YouTube —
        // không set cookie tracking cho người xem chưa bấm play.
        Ok(format!("https://www.youtube-nocookie.com/embed/{id}"))
    }

    #[askama::filter_fn]
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub fn format_date(
        date: &Option<chrono::NaiveDate>,
        _: &dyn Values,
    ) -> ::askama::Result<String> {
        Ok(date
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_default())
    }

    #[askama::filter_fn]
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub fn format_date_vn(
        date: &Option<chrono::NaiveDate>,
        _: &dyn Values,
    ) -> ::askama::Result<String> {
        Ok(date
            .map(|d| d.format("%d/%m/%Y").to_string())
            .unwrap_or_default())
    }

    #[askama::filter_fn]
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub fn format_datetime_vn(
        dt: &chrono::DateTime<chrono::Utc>,
        _: &dyn Values,
    ) -> ::askama::Result<String> {
        Ok(dt.format("%d/%m/%Y").to_string())
    }

    #[askama::filter_fn]
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
            "slug phải ascii: {s}"
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
        assert!(out.contains("giờ trước"), "got: {out}");
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

// ============================================================
// News templates
// ============================================================

/// Trang danh sách tin tức (public)
#[derive(Template)]
#[template(path = "news/list.html")]
pub struct NewsListTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub items: Vec<news::NewsWithAuthor>,
    pub featured: Vec<news::NewsWithAuthor>,
    pub total: i64,
    pub page: i64,
    pub total_pages: i64,
    pub category: String,
    pub category_label: String,
    pub query: String,
    pub categories: Vec<(String, String)>,
}

/// Trang chi tiết tin tức (public)
#[derive(Template)]
#[template(path = "news/show.html")]
pub struct NewsShowTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub news: news::NewsWithAuthor,
    pub comments: Vec<news::NewsCommentWithAuthor>,
    pub has_liked: bool,
    pub base_url: String,
    /// v2.2.0 — tin liên quan (cùng category, fallback tin mới nhất).
    pub related: Vec<news::NewsWithAuthor>,
}

/// Form đăng tin mới
#[derive(Template)]
#[template(path = "news/new.html")]
pub struct NewsNewTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub categories: Vec<(String, String)>,
    pub errors: Vec<String>,
    pub form: crate::handlers::news::NewsFormPartial,
}

/// Form sửa tin
#[derive(Template)]
#[template(path = "news/edit.html")]
pub struct NewsEditTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub categories: Vec<(String, String)>,
    pub news: news::News,
    pub errors: Vec<String>,
}

/// Trang "Tin của tôi"
#[derive(Template)]
#[template(path = "news/my_news.html")]
pub struct MyNewsTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub items: Vec<news::News>,
}

/// Admin: hàng đợi duyệt tin
#[derive(Template)]
#[template(path = "admin/news_pending.html")]
pub struct AdminNewsPendingTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub items: Vec<news::NewsForAdmin>,
    pub total: i64,
    pub page: i64,
    pub total_pages: i64,
}

/// Admin: tất cả tin (cho admin duyệt/xem lại)
#[derive(Template)]
#[template(path = "admin/news_all.html")]
pub struct AdminNewsAllTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub items: Vec<news::NewsForAdmin>,
    pub total: i64,
    pub page: i64,
    pub total_pages: i64,
}

/// Admin user detail — chỉ admin xem được, không phải moderator.
/// Hiển thị toàn bộ thông tin: email, IP/UA signup, IP/UA last login,
/// danh sách sessions, số game/news đã đăng.
#[derive(Template)]
#[template(path = "admin/user_detail.html")]
pub struct AdminUserDetailTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub user: user::User,
    pub games_count: i64,
    pub news_count: i64,
    pub active_sessions: i64,
    pub sessions: Vec<crate::models::settings::SessionRow>,
    pub is_self: bool,                      // true nếu admin đang xem chính mình
    pub now: chrono::DateTime<chrono::Utc>, // cho check session expires_at > now
}

// ============================================================
// v2.9.0 — GAMIFICATION + COLLECTIONS + FOLLOWING + SESSIONS
// ============================================================

/// Bảng xếp hạng (/leaderboard)
#[derive(Template)]
#[template(path = "gamification/leaderboard.html")]
pub struct LeaderboardTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub entries: Vec<crate::models::gamification::LeaderboardEntry>,
    pub hot_games: Vec<GameCard>,
    /// v3.0.0 — Top XP tháng hiện tại (season board).
    pub season_entries: Vec<crate::models::retention::SeasonEntry>,
    /// v3.0.0 — Top XP tuần hiện tại (hall of fame tuần).
    pub weekly_entries: Vec<crate::models::retention::SeasonEntry>,
    /// v3.0.0 — Lịch điểm danh tháng hiện tại của viewer (None nếu anon).
    pub calendar: Option<Vec<crate::models::retention::CalendarDay>>,
}

/// Trang huy hiệu cá nhân (/achievements)
#[derive(Template)]
#[template(path = "gamification/achievements.html")]
pub struct AchievementsTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub achievements: Vec<crate::models::gamification::AchievementWithStatus>,
    pub level: crate::models::gamification::LevelInfo,
    pub streak: i32,
    pub earned_count: usize,
}

/// Feed game từ người theo dõi (/following)
#[derive(Template)]
#[template(path = "following/index.html")]
pub struct FollowingTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub games: Vec<GameCard>,
    pub page: i64,
    pub total_pages: i64,
    pub total: i64,
}

/// Bộ sưu tập của tôi (/collections)
#[derive(Template)]
#[template(path = "collections/index.html")]
pub struct MyCollectionsTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub collections: Vec<crate::repositories::collection::Collection>,
}

/// Xem 1 bộ sưu tập (/collections/{id})
#[derive(Template)]
#[template(path = "collections/show.html")]
pub struct CollectionShowTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub collection: crate::repositories::collection::Collection,
    pub games: Vec<GameCard>,
    pub owner_name: String,
    pub owner_username: String,
    pub owner_avatar: Option<String>,
}

/// Phiên đăng nhập của user (/profile/sessions)
#[derive(Template)]
#[template(path = "profile/sessions.html")]
pub struct ProfileSessionsTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub sessions: Vec<MySessionRow>,
}

/// Dòng session phiên bản user (không lộ token hash).
#[derive(Debug, Clone)]
pub struct MySessionRow {
    pub id: uuid::Uuid,
    pub user_agent: String,
    pub ip: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub current: bool,
}

impl MySessionRow {
    /// IP hiển thị ("—" nếu không có) — askama không render Option trực tiếp.
    #[must_use]
    pub fn ip_display(&self) -> &str {
        self.ip.as_deref().unwrap_or("—")
    }
}

/// Admin: thống kê huy hiệu (/admin/achievements)
#[derive(Template)]
#[template(path = "admin/achievements.html")]
pub struct AdminAchievementsTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub stats: Vec<(crate::models::gamification::Achievement, i64)>,
    pub total_users: i64,
    pub total_holders: i64,
    pub earned_today: i64,
    pub checkins_today: i64,
}

// ============================================================
// v3.0.0 — RETENTION: nhiệm vụ, vòng quay, câu đố, cửa hàng,
// referral, tùy chọn thông báo.
// ============================================================

/// Trang nhiệm vụ hằng ngày/tuần (/quests)
#[derive(Template)]
#[template(path = "gamification/quests.html")]
pub struct QuestsTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub quests: Vec<crate::models::retention::QuestWithProgress>,
    pub level: crate::models::gamification::LevelInfo,
}

/// Trang vòng quay may mắn (/spin)
#[derive(Template)]
#[template(path = "gamification/spin.html")]
pub struct SpinTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    /// Giải đã quay hôm nay (Some = đã quay, nút disabled).
    pub today_prize: Option<i32>,
    pub level: crate::models::gamification::LevelInfo,
}

/// Trang câu đố hằng ngày (/trivia)
#[derive(Template)]
#[template(path = "gamification/trivia.html")]
pub struct TriviaTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub questions: Vec<crate::models::retention::TriviaQuestionPublic>,
    /// Số câu đúng hôm nay (để hiện trạng thái hoàn thành).
    pub correct_today: i64,
}

/// Trang cửa hàng XP (/shop)
#[derive(Template)]
#[template(path = "gamification/shop.html")]
pub struct ShopTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub items: Vec<crate::models::retention::ShopItemWithStock>,
    pub total_xp: i32,
}

/// Trang chương trình giới thiệu (/referral)
#[derive(Template)]
#[template(path = "gamification/referral.html")]
pub struct ReferralTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub info: crate::models::retention::ReferralInfo,
    /// Host công khai (vd louis.vangioitutien.com) — hiện link đầy đủ.
    pub base_url_host: String,
}

/// Trang tùy chọn thông báo (/settings/notifications)
#[derive(Template)]
#[template(path = "pages/notif_prefs.html")]
pub struct NotifPrefsTemplate {
    pub current_user: Option<user::User>,
    pub unread_notifications: i64,
    pub prefs: crate::models::retention::NotificationPrefs,
    /// Hiện banner "đã lưu" sau khi POST redirect ?saved=1.
    pub saved: bool,
}

/// 1 ô heatmap: số hoạt động + mức màu 0-4 (tính sẵn ở Rust).
#[derive(Debug, Clone, serde::Serialize)]
pub struct HeatCell {
    pub count: i32,
    /// 0..=4 — class CSS heat-N.
    pub level: u8,
}

impl HeatCell {
    /// Tính mức màu từ số hoạt động (hàm thuần — test được).
    #[must_use]
    pub fn from_count(count: i32) -> Self {
        let level = if count <= 0 {
            0
        } else if count == 1 {
            1
        } else if count <= 3 {
            2
        } else if count <= 8 {
            3
        } else {
            4
        };
        Self { count, level }
    }
}

/// Dữ liệu heatmap (render qua include trong profile.html).
#[derive(Debug, Clone)]
pub struct HeatmapWidget {
    /// 13 tuần × 7 ngày — cell None = ngoài phạm vi.
    pub weeks: Vec<[Option<HeatCell>; 7]>,
}

/// Dữ liệu onboarding checklist (render qua include trong index.html).
#[derive(Debug, Clone)]
pub struct OnboardingWidget {
    pub steps: Vec<crate::models::retention::OnboardingStepStatus>,
    pub done_count: usize,
}
