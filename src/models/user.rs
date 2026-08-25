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
    pub fn is_admin(&self) -> bool {
        matches!(self, UserRole::Admin)
    }
    pub fn is_staff(&self) -> bool {
        matches!(self, UserRole::Admin | UserRole::Moderator)
    }
    /// True nếu đây là tài khoản AI Agent (khác hẳn user thường).
    pub fn is_ai_agent(&self) -> bool {
        matches!(self, UserRole::AiAgent)
    }
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
}

impl User {
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
}
