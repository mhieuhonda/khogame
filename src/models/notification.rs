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
    pub fn icon(&self) -> &'static str {
        self.r#type.icon()
    }
    pub fn type_label(&self) -> &'static str {
        self.r#type.label()
    }
    pub fn link_or(&self) -> String {
        self.link.clone().unwrap_or_default()
    }
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
    pub fn icon(&self) -> &'static str {
        self.r#type.icon()
    }
    pub fn type_label(&self) -> &'static str {
        self.r#type.label()
    }
    pub fn link_or(&self) -> String {
        self.link.clone().unwrap_or_default()
    }
    pub fn content_or(&self) -> String {
        self.content.clone().unwrap_or_default()
    }
}
