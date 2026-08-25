use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[sqlx(type_name = "notification_type", rename_all = "snake_case")]
pub enum NotificationType {
    Comment,
    Reply,
    Like,
    Follow,
    ReportStatus,
    System,
    NewGame,
    Review,
    Rating,
    Mention,
}

impl NotificationType {
    #[must_use]
    pub fn icon(&self) -> &'static str {
        match self {
            NotificationType::Comment => "💬",
            NotificationType::Reply => "↩️",
            NotificationType::Like => "❤️",
            NotificationType::Follow => "👤",
            NotificationType::ReportStatus => "🚩",
            NotificationType::System => "🔔",
            NotificationType::NewGame => "🎮",
            NotificationType::Review => "⭐",
            NotificationType::Rating => "⭐",
            NotificationType::Mention => "@",
        }
    }
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            NotificationType::Comment => "Bình luận mới",
            NotificationType::Reply => "Phản hồi",
            NotificationType::Like => "Lượt thích",
            NotificationType::Follow => "Người theo dõi",
            NotificationType::ReportStatus => "Báo cáo",
            NotificationType::System => "Hệ thống",
            NotificationType::NewGame => "Game mới",
            NotificationType::Review => "Đánh giá",
            NotificationType::Rating => "Đánh giá sao",
            NotificationType::Mention => "Đề cập",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Notification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub r#type: NotificationType,
    pub title: String,
    pub content: Option<String>,
    pub link: Option<String>,
    pub is_read: bool,
    pub created_at: DateTime<Utc>,
}

impl Notification {
    #[must_use]
    pub fn icon(&self) -> &'static str {
        self.r#type.icon()
    }
    #[must_use]
    pub fn type_label(&self) -> &'static str {
        self.r#type.label()
    }
    #[must_use]
    pub fn link_or(&self) -> String {
        self.link.clone().unwrap_or_default()
    }
    #[must_use]
    pub fn content_or(&self) -> String {
        self.content.clone().unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NotificationWithActor {
    pub id: Uuid,
    pub user_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub r#type: NotificationType,
    pub title: String,
    pub content: Option<String>,
    pub link: Option<String>,
    pub is_read: bool,
    pub created_at: DateTime<Utc>,
    pub actor_name: Option<String>,
    pub actor_avatar: Option<String>,
}

impl NotificationWithActor {
    #[must_use]
    pub fn icon(&self) -> &'static str {
        self.r#type.icon()
    }
    #[must_use]
    pub fn type_label(&self) -> &'static str {
        self.r#type.label()
    }
    #[must_use]
    pub fn link_or(&self) -> String {
        self.link.clone().unwrap_or_default()
    }
    #[must_use]
    pub fn content_or(&self) -> String {
        self.content.clone().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mọi variant phải có icon KHÔNG rỗng — icon rỗng sẽ render ô trống
    /// kỳ quặc trong dropdown thông báo.
    #[test]
    fn test_every_notification_type_has_icon_and_label() {
        let all = [
            NotificationType::Comment,
            NotificationType::Reply,
            NotificationType::Like,
            NotificationType::Follow,
            NotificationType::ReportStatus,
            NotificationType::System,
            NotificationType::NewGame,
            NotificationType::Review,
            NotificationType::Rating,
            NotificationType::Mention,
        ];
        for t in &all {
            assert!(!t.icon().is_empty(), "icon rỗng cho {:?}", t);
            assert!(!t.label().is_empty(), "label rỗng cho {:?}", t);
        }
    }

    /// Icon/label của Review và Rating khác nhau thì label cũng phải
    /// khác nhau để admin phân biệt loại thông báo trong UI.
    #[test]
    fn test_review_and_rating_labels_distinct() {
        assert_ne!(
            NotificationType::Review.label(),
            NotificationType::Rating.label()
        );
    }

    /// Helper link_or/content_or trả chuỗi rỗng (không panic) khi None —
    /// template askama dùng trực tiếp giá trị này.
    #[test]
    fn test_link_or_content_or_defaults() {
        let n = Notification {
            id: Uuid::nil(),
            user_id: Uuid::nil(),
            actor_id: None,
            r#type: NotificationType::System,
            title: "Tieu de".into(),
            content: None,
            link: None,
            is_read: false,
            created_at: chrono::Utc::now(),
        };
        assert_eq!(n.link_or(), "");
        assert_eq!(n.content_or(), "");
        assert_eq!(n.icon(), NotificationType::System.icon());
        assert_eq!(n.type_label(), NotificationType::System.label());
    }
}
