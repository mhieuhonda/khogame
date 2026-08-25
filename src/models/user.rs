use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Default)]
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
    pub fn is_admin(&self) -> bool {
        matches!(self, UserRole::Admin)
    }
    #[must_use]
    pub fn is_staff(&self) -> bool {
        matches!(self, UserRole::Admin | UserRole::Moderator)
    }
    /// True nếu đây là tài khoản AI Agent (khác hẳn user thường).
    #[must_use]
    pub fn is_ai_agent(&self) -> bool {
        matches!(self, UserRole::AiAgent)
    }
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            UserRole::User => "Thành viên",
            UserRole::Moderator => "Điều hành viên",
            UserRole::Admin => "Quản trị viên",
            UserRole::AiAgent => "Tác nhân AI",
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
}
