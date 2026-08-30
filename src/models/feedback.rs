//! Model cho hệ thống góp ý người dùng (v3.4.0).
//!
//! Người dùng gửi góp ý / báo cáo lỗi / báo cáo bảo mật / đề xuất nâng cấp /
//! đề xuất chức năng. Admin xem xét và xử lý tại `/admin/feedback`.
//!
//! - [`FeedbackCategory`]: loại góp ý (enum `feedback_category` trong DB).
//! - [`FeedbackStatus`]: trạng thái xử lý (enum `feedback_status`).
//! - [`Feedback`]: 1 dòng trong bảng `user_feedback`.
//! - [`FeedbackWithUser`]: feedback kèm thông tin người gửi cho trang admin.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

/// Loại góp ý người dùng gửi.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq, Copy)]
#[sqlx(type_name = "feedback_category", rename_all = "snake_case")]
pub enum FeedbackCategory {
    /// Góp ý chung về nền tảng
    General,
    /// Báo cáo lỗi (bug) gặp trên site
    Bug,
    /// Báo cáo lỗ hổng bảo mật — CHỈ admin được xem
    Security,
    /// Đề xuất nâng cấp hệ thống
    Upgrade,
    /// Đề xuất tính năng mới
    Feature,
}

impl FeedbackCategory {
    /// Tất cả các loại (thứ tự hiển thị form).
    #[must_use]
    pub fn all() -> [Self; 5] {
        [
            Self::General,
            Self::Bug,
            Self::Security,
            Self::Upgrade,
            Self::Feature,
        ]
    }

    /// Nhãn tiếng Việt hiển thị UI.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::General => "Góp ý chung",
            Self::Bug => "Báo cáo lỗi",
            Self::Security => "Bảo mật",
            Self::Upgrade => "Đề xuất nâng cấp",
            Self::Feature => "Đề xuất chức năng",
        }
    }

    /// Mô tả ngắn hiển thị dưới label trong form.
    #[must_use]
    pub const fn hint(&self) -> &'static str {
        match self {
            Self::General => "Ý kiến đóng góp chung để nền tảng tốt hơn",
            Self::Bug => "Gặp lỗi trên web? Mô tả rõ trình tự để mình tái hiện",
            Self::Security => "Phát hiện lỗ hổng bảo mật — chỉ quản trị viên được xem",
            Self::Upgrade => "Đề xuất nâng cấp hiệu năng, giao diện, hạ tầng…",
            Self::Feature => "Tính năng mới bạn muốn thấy trên Louis Space",
        }
    }

    /// Icon (emoji) cho badge danh mục.
    #[must_use]
    pub const fn icon(&self) -> &'static str {
        match self {
            Self::General => "💬",
            Self::Bug => "🐞",
            Self::Security => "🔐",
            Self::Upgrade => "⬆️",
            Self::Feature => "✨",
        }
    }

    /// Màu badge theo danh mục.
    #[must_use]
    pub const fn color(&self) -> &'static str {
        match self {
            Self::General => "#3b82f6",
            Self::Bug => "#ef4444",
            Self::Security => "#f59e0b",
            Self::Upgrade => "#8b5cf6",
            Self::Feature => "#10b981",
        }
    }

    /// Parse từ chuỗi form ("bug", "security"…). Case-insensitive.
    #[allow(clippy::should_implement_trait)]
    #[must_use]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "general" => Some(Self::General),
            "bug" => Some(Self::Bug),
            "security" => Some(Self::Security),
            "upgrade" => Some(Self::Upgrade),
            "feature" => Some(Self::Feature),
            _ => None,
        }
    }

    /// Key dùng làm value trong `<option>` + form submit.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Bug => "bug",
            Self::Security => "security",
            Self::Upgrade => "upgrade",
            Self::Feature => "feature",
        }
    }

    /// Có phải góp ý bảo mật không (askama template dùng — enum path
    /// expression không khả dụng trong askama).
    #[must_use]
    pub const fn is_security(&self) -> bool {
        matches!(self, Self::Security)
    }
}

/// Trạng thái xử lý feedback.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq, Copy)]
#[sqlx(type_name = "feedback_status", rename_all = "snake_case")]
pub enum FeedbackStatus {
    /// Chờ xử lý
    Pending,
    /// Đang xem xét
    Reviewing,
    /// Đã xử lý
    Resolved,
    /// Đã bỏ qua
    Dismissed,
}

impl FeedbackStatus {
    /// Nhãn tiếng Việt.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Pending => "Chờ xử lý",
            Self::Reviewing => "Đang xem xét",
            Self::Resolved => "Đã xử lý",
            Self::Dismissed => "Đã bỏ qua",
        }
    }

    /// Màu badge trạng thái.
    #[must_use]
    pub const fn color(&self) -> &'static str {
        match self {
            Self::Pending => "#f59e0b",
            Self::Reviewing => "#3b82f6",
            Self::Resolved => "#10b981",
            Self::Dismissed => "#6b7280",
        }
    }

    /// Parse từ chuỗi form.
    #[allow(clippy::should_implement_trait)]
    #[must_use]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "pending" => Some(Self::Pending),
            "reviewing" => Some(Self::Reviewing),
            "resolved" => Some(Self::Resolved),
            "dismissed" => Some(Self::Dismissed),
            _ => None,
        }
    }

    /// Key dùng làm value trong form + so sánh ở template askama.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Reviewing => "reviewing",
            Self::Resolved => "resolved",
            Self::Dismissed => "dismissed",
        }
    }

    /// Tất cả trạng thái (form admin).
    #[must_use]
    pub fn all() -> [Self; 4] {
        [
            Self::Pending,
            Self::Reviewing,
            Self::Resolved,
            Self::Dismissed,
        ]
    }
}

/// Một góp ý trong bảng `user_feedback`.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Feedback {
    pub id: Uuid,
    pub user_id: Uuid,
    pub category: FeedbackCategory,
    pub title: String,
    pub body: String,
    pub page_url: Option<String>,
    pub status: FeedbackStatus,
    pub admin_response: Option<String>,
    pub handled_by: Option<Uuid>,
    pub handled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Feedback kèm thông tin người gửi (trang admin + "góp ý của tôi").
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FeedbackWithUser {
    pub id: Uuid,
    pub user_id: Uuid,
    pub category: FeedbackCategory,
    pub title: String,
    pub body: String,
    pub page_url: Option<String>,
    pub status: FeedbackStatus,
    pub admin_response: Option<String>,
    pub handled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    // Nối từ users
    pub user_display_name: String,
    pub user_username: String,
    pub user_avatar_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_roundtrip() {
        for c in FeedbackCategory::all() {
            assert_eq!(FeedbackCategory::from_str(c.key()), Some(c));
            assert_eq!(
                FeedbackCategory::from_str(&c.key().to_uppercase()),
                Some(c),
                "case-insensitive key={}",
                c.key()
            );
        }
    }

    #[test]
    fn test_category_rejects_unknown() {
        assert_eq!(FeedbackCategory::from_str("hack"), None);
        assert_eq!(FeedbackCategory::from_str(""), None);
        assert_eq!(
            FeedbackCategory::from_str("  feature  "),
            Some(FeedbackCategory::Feature)
        );
    }

    #[test]
    fn test_all_categories_have_metadata() {
        for c in FeedbackCategory::all() {
            assert!(!c.label().is_empty());
            assert!(!c.hint().is_empty());
            assert!(!c.icon().is_empty());
            let color = c.color();
            assert!(color.starts_with('#') && color.len() == 7, "color={color}");
        }
    }

    #[test]
    fn test_status_roundtrip_and_metadata() {
        for s in FeedbackStatus::all() {
            assert_eq!(FeedbackStatus::from_str(s.label()), None); // label VN ≠ key
            assert!(!s.label().is_empty());
            let color = s.color();
            assert!(color.starts_with('#') && color.len() == 7, "color={color}");
        }
        assert_eq!(
            FeedbackStatus::from_str("pending"),
            Some(FeedbackStatus::Pending)
        );
        assert_eq!(
            FeedbackStatus::from_str("RESOLVED"),
            Some(FeedbackStatus::Resolved)
        );
        assert_eq!(FeedbackStatus::from_str("unknown"), None);
    }
}
