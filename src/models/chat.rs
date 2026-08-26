use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

/// Một dòng trong bảng `chat_messages`.
///
/// `is_deleted=true` đồng nghĩa với việc tin nhắn bị admin ẩn — client
/// nhận được sẽ hiển thị placeholder "tin nhắn đã bị ẩn" thay vì nội dung
/// gốc (giữ context của thread nếu có reply).
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ChatMessage {
    pub id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub author_ip: Option<String>,
    pub author_ua: Option<String>,
    pub is_deleted: bool,
    pub created_at: DateTime<Utc>,
}

/// Chat message kèm thông tin hiển thị của author (username, avatar, display_name).
/// Lấy qua JOIN với bảng `users` — payload trả về cho API history và broadcast
/// qua WebSocket để client render một lần mà không phải fetch thêm user info.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ChatMessageWithUser {
    pub id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub is_deleted: bool,
    pub created_at: DateTime<Utc>,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    /// Role của author — client hiển thị badge "Admin" / "Mod" để user
    /// phân biệt được tin nhắn từ staff vs user thường.
    pub role: String,
}

impl ChatMessageWithUser {
    /// Tên hiển thị ưu tiên `display_name`, fallback `username` (cho client).
    #[must_use]
    pub fn display_label(&self) -> &str {
        if self.display_name.is_empty() {
            &self.username
        } else {
            &self.display_name
        }
    }
}
