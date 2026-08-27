use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq, Default)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
pub enum UserRole {
    #[default]
    User,
    Moderator,
    Admin,
    /// Tài khoản đặc biệt dành cho AI Agent (do admin cấp secret để AI
    /// tự đăng ký). Có thể đăng nhập bằng token dài hạn, báo cáo tiến
    /// trình về trang admin. Không phải staff (không có quyền quản trị
    /// site) nhưng có quyền truy cập các endpoint AI nội bộ.
    AiAgent,
}

impl UserRole {
    #[must_use]
    pub const fn is_admin(&self) -> bool {
        matches!(self, Self::Admin)
    }
    #[must_use]
    pub const fn is_staff(&self) -> bool {
        matches!(self, Self::Admin | Self::Moderator)
    }
    /// True nếu đây là tài khoản AI Agent (khác hẳn user thường).
    #[must_use]
    pub const fn is_ai_agent(&self) -> bool {
        matches!(self, Self::AiAgent)
    }
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::User => "Thành viên",
            Self::Moderator => "Điều hành viên",
            Self::Admin => "Quản trị viên",
            Self::AiAgent => "Tác nhân AI",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub google_sub: String,
    pub role: UserRole,
    pub is_banned: bool,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // === Tracking fields (migration 009) ===
    // Chỉ admin xem được; moderator không bao giờ thấy.
    // Lưu để truy vết spam/abuse: ai đăng từ IP nào, dùng thiết bị gì.
    pub signup_ip: Option<String>,
    pub signup_ua: Option<String>,
    pub last_login_ip: Option<String>,
    pub last_login_ua: Option<String>,
    pub last_login_at: Option<DateTime<Utc>>,
}

impl User {
    #[must_use]
    pub fn bio_or(&self) -> String {
        self.bio.clone().unwrap_or_default()
    }
}

/// Trạng thái hiển thị của user cho bảng admin — fix bug v1.3.x:
/// trước đây template chỉ phân biệt `is_banned ? "Bị cấm" : "Hoạt động"`,
/// nên MỌI user không bị cấm đều hiện "Hoạt động" dù thực tế chưa login
/// bao giờ hoặc đã bỏ hoạt động từ lâu. Sai sự thật và gây hiểu nhầm cho
/// admin khi rà soát tài khoản.
///
/// Các trạng thái mới (v1.4.0):
/// - Banned (đỏ) — bị cấm, không cần xét `last_seen_at`
/// - New (xanh dương) — đăng ký trong 7 ngày gần đây (newly registered)
/// - Online (xanh lá) — `last_seen_at` trong 15 phút (đang dùng web)
/// - Active (xanh lá nhạt) — `last_seen_at` trong 24h
/// - Inactive (vàng) — `last_seen_at` trong 30 ngày
/// - Dormant (xám) — `last_seen_at` > 30 ngày hoặc chưa từng đăng nhập
///
/// `last_seen_at` được update tối đa 1 lần/giờ/user qua `touch_last_seen`
/// trong middleware — đủ granularity để phân biệt online vs active vs inactive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserStatusBadge {
    Banned,
    New,
    Online,
    Active,
    Inactive,
    Dormant,
}

impl UserStatusBadge {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Banned => "Bị cấm",
            Self::New => "Thành viên mới",
            Self::Online => "Đang online",
            Self::Active => "Hoạt động",
            Self::Inactive => "Không hoạt động",
            Self::Dormant => "Ngừng hoạt động",
        }
    }

    /// Màu CSS (hex) cho badge — dùng inline style trong admin template
    /// để khỏi phải thêm nhiều class CSS mới. Tương phản tốt với cả light
    /// và dark theme (kiểm thử bằng contrast checker).
    #[must_use]
    pub const fn color(self) -> &'static str {
        match self {
            Self::Banned => "#ef4444",   // red-500
            Self::New => "#3b82f6",      // blue-500
            Self::Online => "#22c55e",   // green-500 (sáng hơn cho "đang online")
            Self::Active => "#10b981",   // emerald-500
            Self::Inactive => "#f59e0b", // amber-500
            Self::Dormant => "#94a3b8",  // slate-400
        }
    }

    /// Tính badge từ `is_banned`, `created_at`, `last_seen_at`.
    /// `now` truyền vào để dễ test (deterministic — không phụ thuộc
    /// system clock của test runner).
    #[must_use]
    pub fn compute(
        is_banned: bool,
        created_at: chrono::DateTime<chrono::Utc>,
        last_seen_at: Option<chrono::DateTime<chrono::Utc>>,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        if is_banned {
            return Self::Banned;
        }
        // User mới đăng ký trong 7 ngày — ưu tiên hiển thị "Thành viên mới"
        // để admin rà soát tài khoản mới (chống bot/spam).
        let age = now.signed_duration_since(created_at);
        if age.num_days() < 7 {
            return Self::New;
        }
        match last_seen_at {
            None => Self::Dormant,
            Some(t) => {
                let elapsed = now.signed_duration_since(t);
                if elapsed.num_minutes() < 15 {
                    Self::Online
                } else if elapsed.num_hours() < 24 {
                    Self::Active
                } else if elapsed.num_days() < 30 {
                    Self::Inactive
                } else {
                    Self::Dormant
                }
            }
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserStats {
    pub games_count: i64,
    pub followers_count: i64,
    pub following_count: i64,
}

/// User + số game đã đăng (cho bảng quản trị)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserWithGameCount {
    pub id: Uuid,
    pub email: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub google_sub: String,
    pub role: UserRole,
    pub is_banned: bool,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub games_count: i64,
    // Tracking fields — chỉ admin xem được
    pub signup_ip: Option<String>,
    pub signup_ua: Option<String>,
    pub last_login_ip: Option<String>,
    pub last_login_ua: Option<String>,
    pub last_login_at: Option<DateTime<Utc>>,
}

impl UserWithGameCount {
    /// Label trạng thái cho badge admin (gọi từ template Askama).
    /// Askama 0.16 chỉ hỗ trợ method call 0-arg → gói `UserStatusBadge::compute`
    /// trong 2 wrapper `status_badge_label()` và `status_badge_color()` trả
    /// về `&'static str` (kiểu primitive Askama render được inline).
    /// Non-deterministic (gọi `Utc::now()` mỗi lần render) — OK cho admin
    /// view, không nên dùng cho cache/audit log.
    #[must_use]
    pub fn status_badge_label(&self) -> &'static str {
        UserStatusBadge::compute(
            self.is_banned,
            self.created_at,
            self.last_seen_at,
            chrono::Utc::now(),
        )
        .label()
    }

    #[must_use]
    pub fn status_badge_color(&self) -> &'static str {
        UserStatusBadge::compute(
            self.is_banned,
            self.created_at,
            self.last_seen_at,
            chrono::Utc::now(),
        )
        .color()
    }

    /// Tính badge với `now` cho trước — cho test deterministic và cho
    /// handler khi cần tính status filter chip count.
    #[must_use]
    pub fn status_badge_at(&self, now: DateTime<Utc>) -> UserStatusBadge {
        UserStatusBadge::compute(self.is_banned, self.created_at, self.last_seen_at, now)
    }
}

/// Phiên bản rút gọn cho moderator — KHÔNG chứa email, IP, UA.
/// Moderator có thể quản lý games/comments của user nhưng không
/// được xem thông tin nhạy cảm (email cá nhân, IP, UA).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserForModerator {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub role: UserRole,
    pub is_banned: bool,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub games_count: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, FromRow)]
pub struct UserPreference {
    pub theme: String,
    pub email_notifications: bool,
    pub show_online: bool,
    pub language: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_permission_matrix() {
        // User thường: không phải staff, không phải admin, không phải AI
        assert!(!UserRole::User.is_staff());
        assert!(!UserRole::User.is_admin());
        assert!(!UserRole::User.is_ai_agent());

        // Moderator: staff nhưng không admin
        assert!(UserRole::Moderator.is_staff());
        assert!(!UserRole::Moderator.is_admin());
        assert!(!UserRole::Moderator.is_ai_agent());

        // Admin: vừa staff vừa admin
        assert!(UserRole::Admin.is_staff());
        assert!(UserRole::Admin.is_admin());
        assert!(!UserRole::Admin.is_ai_agent());

        // AI Agent: KHÔNG phải staff (quan trọng — AI không được đụng admin)
        assert!(!UserRole::AiAgent.is_staff());
        assert!(!UserRole::AiAgent.is_admin());
        assert!(UserRole::AiAgent.is_ai_agent());
    }

    #[test]
    fn test_role_labels() {
        assert_eq!(UserRole::User.label(), "Thành viên");
        assert_eq!(UserRole::Moderator.label(), "Điều hành viên");
        assert_eq!(UserRole::Admin.label(), "Quản trị viên");
        assert_eq!(UserRole::AiAgent.label(), "Tác nhân AI");
    }

    #[test]
    fn test_default_role_is_user() {
        // Default của FromRow khi DB trả NULL → phải là User (an toàn nhất:
        // thiếu quyền tốt hơn thừa quyền)
        assert_eq!(UserRole::default(), UserRole::User);
    }

    #[test]
    fn test_user_tracking_fields_are_optional() {
        // Migration 009 thêm 5 cột tracking. Tất cả đều Option<> để
        // user cũ (tạo trước v0.8.0) không có dữ liệu cũng load được
        // — DB trả NULL → Option::None → template render "—".
        // Verify struct compile và field tồn tại.
        let user = User {
            id: Uuid::new_v4(),
            email: "test@example.com".into(),
            username: "test".into(),
            display_name: "Test".into(),
            avatar_url: None,
            bio: None,
            google_sub: "sub".into(),
            role: UserRole::User,
            is_banned: false,
            last_seen_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            signup_ip: None,
            signup_ua: None,
            last_login_ip: None,
            last_login_ua: None,
            last_login_at: None,
        };
        // IP/UA None khi user chưa login lần nào
        assert!(user.signup_ip.is_none());
        assert!(user.signup_ua.is_none());
        assert!(user.last_login_ip.is_none());
        assert!(user.last_login_ua.is_none());
        assert!(user.last_login_at.is_none());
    }

    #[test]
    fn test_user_with_tracking_fields() {
        // Simulate user đã login — có IP/UA
        let user = User {
            id: Uuid::new_v4(),
            email: "test@example.com".into(),
            username: "test".into(),
            display_name: "Test".into(),
            avatar_url: None,
            bio: None,
            google_sub: "sub".into(),
            role: UserRole::User,
            is_banned: false,
            last_seen_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            signup_ip: Some("203.0.113.42".into()),
            signup_ua: Some("Mozilla/5.0".into()),
            last_login_ip: Some("203.0.113.99".into()),
            last_login_ua: Some("Mozilla/5.0 Chrome".into()),
            last_login_at: Some(chrono::Utc::now()),
        };
        // Admin có thể xem IP signup + last login để truy vết abuse
        assert_eq!(user.signup_ip.as_deref(), Some("203.0.113.42"));
        assert_eq!(user.last_login_ip.as_deref(), Some("203.0.113.99"));
        assert!(user.last_login_at.is_some());
    }

    // ===== Tests v1.4.0: UserStatusBadge::compute =====
    // Verify fix bug "luôn hiện Hoạt động" — trước đây mọi user không
    // bị cấm đều hiện "Hoạt động" dù thực tế chưa đăng nhập.
    #[test]
    fn test_status_badge_banned_overrides_everything() {
        // Banned → luôn Banned, dù vừa mới đăng ký + online 5 phút trước.
        let now = chrono::Utc::now();
        let created = now;
        let last_seen = Some(now);
        assert_eq!(
            UserStatusBadge::compute(true, created, last_seen, now),
            UserStatusBadge::Banned
        );
    }

    #[test]
    fn test_status_badge_new_user_under_7_days() {
        // User đăng ký 3 ngày trước + chưa login → New (vì age < 7 ngày)
        let now = chrono::Utc::now();
        let created = now - chrono::Duration::days(3);
        assert_eq!(
            UserStatusBadge::compute(false, created, None, now),
            UserStatusBadge::New
        );
        // Kể cả last_seen cũ → vẫn New (ưu tiên New để admin rà soát bot)
        let old_seen = Some(now - chrono::Duration::days(2));
        assert_eq!(
            UserStatusBadge::compute(false, created, old_seen, now),
            UserStatusBadge::New
        );
    }

    #[test]
    fn test_status_badge_online_under_15_minutes() {
        // User active trong 15 phút → Online
        let now = chrono::Utc::now();
        let created = now - chrono::Duration::days(30); // đã qua "New" window
        let last_seen = Some(now - chrono::Duration::minutes(5));
        assert_eq!(
            UserStatusBadge::compute(false, created, last_seen, now),
            UserStatusBadge::Online
        );
    }

    #[test]
    fn test_status_badge_active_within_24h() {
        // User last_seen 2 giờ trước → Active (trong 24h)
        let now = chrono::Utc::now();
        let created = now - chrono::Duration::days(30);
        let last_seen = Some(now - chrono::Duration::hours(2));
        assert_eq!(
            UserStatusBadge::compute(false, created, last_seen, now),
            UserStatusBadge::Active
        );
    }

    #[test]
    fn test_status_badge_inactive_within_30_days() {
        // User last_seen 5 ngày trước → Inactive (24h..30d)
        let now = chrono::Utc::now();
        let created = now - chrono::Duration::days(60);
        let last_seen = Some(now - chrono::Duration::days(5));
        assert_eq!(
            UserStatusBadge::compute(false, created, last_seen, now),
            UserStatusBadge::Inactive
        );
    }

    #[test]
    fn test_status_badge_dormant_over_30_days() {
        // User last_seen 60 ngày trước → Dormant
        let now = chrono::Utc::now();
        let created = now - chrono::Duration::days(120);
        let last_seen = Some(now - chrono::Duration::days(60));
        assert_eq!(
            UserStatusBadge::compute(false, created, last_seen, now),
            UserStatusBadge::Dormant
        );
    }

    #[test]
    fn test_status_badge_dormant_when_never_logged_in() {
        // User tạo 60 ngày trước + chưa từng login → Dormant (chứ không phải Active)
        // Đây chính là bug v1.3.x: trước đây hiện "Hoạt động" dù user chưa login.
        let now = chrono::Utc::now();
        let created = now - chrono::Duration::days(60);
        assert_eq!(
            UserStatusBadge::compute(false, created, None, now),
            UserStatusBadge::Dormant
        );
        assert_ne!(
            UserStatusBadge::compute(false, created, None, now),
            UserStatusBadge::Active,
            "User chưa login KHÔNG được hiện là 'Hoạt động'"
        );
    }

    #[test]
    fn test_status_badge_label_and_color_distinct() {
        // Verify mỗi badge có label + color khác nhau để UI phân biệt được.
        use std::collections::HashSet;
        let labels: HashSet<&str> = [
            UserStatusBadge::Banned.label(),
            UserStatusBadge::New.label(),
            UserStatusBadge::Online.label(),
            UserStatusBadge::Active.label(),
            UserStatusBadge::Inactive.label(),
            UserStatusBadge::Dormant.label(),
        ]
        .into_iter()
        .collect();
        assert_eq!(labels.len(), 6, "Mỗi badge phải có label unique");
        let colors: HashSet<&str> = [
            UserStatusBadge::Banned.color(),
            UserStatusBadge::New.color(),
            UserStatusBadge::Online.color(),
            UserStatusBadge::Active.color(),
            UserStatusBadge::Inactive.color(),
            UserStatusBadge::Dormant.color(),
        ]
        .into_iter()
        .collect();
        assert_eq!(colors.len(), 6, "Mỗi badge phải có color unique");
    }
}
