//! Models cho tính năng Kết bạn + Chat riêng + Nhóm chat (v3.14.0).
//!
//! - [`Friendship`]: 1 dòng `friendships` (pending/accepted/declined/blocked).
//! - [`FriendshipWithUser`]: kèm thông tin hiển thị của "người còn lại".
//! - [`Conversation`]: hội thoại DM hoặc nhóm (`chat_conversations`).
//! - [`InboxItem`]: 1 dòng inbox (hội thoại + preview + unread + đối phương).
//! - [`DmMessageWithSender`]: tin nhắn kèm sender (render thread).
//! - [`GroupMemberInfo`]: thành viên nhóm kèm role.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

/// Trạng thái kết bạn (map enum PG `friend_status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FriendStatus {
    Pending,
    Accepted,
    Declined,
    Blocked,
}

impl FriendStatus {
    /// Parse từ text DB (`status::text`). Trả None nếu giá trị lạ.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "accepted" => Some(Self::Accepted),
            "declined" => Some(Self::Declined),
            "blocked" => Some(Self::Blocked),
            _ => None,
        }
    }

    /// Nhãn tiếng Việt cho UI.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pending => "Đang chờ",
            Self::Accepted => "Bạn bè",
            Self::Declined => "Đã từ chối",
            Self::Blocked => "Đã chặn",
        }
    }
}

/// 1 dòng `friendships` (hướng requester → addressee).
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Friendship {
    pub id: Uuid,
    pub requester_id: Uuid,
    pub addressee_id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Friendship {
    /// Quan hệ này có phải bạn bè (accepted) không.
    #[must_use]
    pub fn is_accepted(&self) -> bool {
        self.status == "accepted"
    }

    /// Id của "người còn lại" khi biết mình là `me`.
    #[must_use]
    pub fn other_id(&self, me: Uuid) -> Uuid {
        if self.requester_id == me {
            self.addressee_id
        } else {
            self.requester_id
        }
    }
}

/// Lời mời / bạn bè kèm thông tin hiển thị của người còn lại.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct FriendshipWithUser {
    pub id: Uuid,
    pub requester_id: Uuid,
    pub addressee_id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
    /// true nếu mình là người GỬI lời mời (để hiện nút "Hủy lời mời"
    /// thay vì "Chấp nhận/Từ chối").
    pub is_requester: bool,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

/// Hội thoại (`chat_conversations`).
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Conversation {
    pub id: Uuid,
    pub kind: String,
    pub dm_key: Option<String>,
    pub name: String,
    pub avatar_url: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Conversation {
    /// Tên hiển thị: nhóm → name; DM → do caller điền (tên đối phương).
    #[must_use]
    pub fn is_group(&self) -> bool {
        self.kind == "group"
    }
}

/// 1 dòng inbox: hội thoại + preview tin cuối + số chưa đọc + đối phương.
///
/// `other_username/display_name/avatar_url` chỉ có ý nghĩa với DM
/// (nhóm dùng `name` của hội thoại, các cột này NULL).
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct InboxItem {
    pub id: Uuid,
    pub kind: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub updated_at: DateTime<Utc>,
    pub my_role: String,
    pub member_count: i64,
    pub last_content: Option<String>,
    pub last_image: Option<String>,
    pub last_sender_name: Option<String>,
    pub last_at: Option<DateTime<Utc>>,
    pub unread: i64,
    pub other_username: Option<String>,
    pub other_display_name: Option<String>,
    pub other_avatar_url: Option<String>,
}

impl InboxItem {
    /// Tên hiển thị của hội thoại (DM → tên đối phương, nhóm → tên nhóm).
    #[must_use]
    pub fn display_name(&self) -> &str {
        if self.kind == "dm" {
            self.other_display_name
                .as_deref()
                .filter(|s| !s.is_empty())
                .or(self.other_username.as_deref())
                .unwrap_or("Cuộc trò chuyện")
        } else if self.name.is_empty() {
            "Nhóm chat"
        } else {
            &self.name
        }
    }

    /// Preview tin cuối cho inbox (cắt 80 ký tự — askama không gọi được
    /// turbofish nên tính sẵn ở đây).
    #[must_use]
    pub fn preview(&self) -> String {
        match self.last_content.as_deref() {
            Some(c) if !c.is_empty() => {
                if c.chars().count() > 80 {
                    let short: String = c.chars().take(80).collect();
                    format!("{short}…")
                } else {
                    c.to_string()
                }
            }
            _ => {
                if self.last_image.is_some() {
                    "📷 [Ảnh]".to_string()
                } else {
                    "(chưa có tin nhắn)".to_string()
                }
            }
        }
    }

    /// Tiền tố "Tên: " trước preview (rỗng nếu không rõ người gửi).
    #[must_use]
    pub fn sender_prefix(&self) -> String {
        self.last_sender_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|s| format!("{s}: "))
            .unwrap_or_default()
    }
}

/// Tin nhắn riêng/nhóm kèm sender (render thread).
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct DmMessageWithSender {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub sender_id: Uuid,
    pub content: String,
    pub image_url: Option<String>,
    pub is_deleted: bool,
    pub created_at: DateTime<Utc>,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub role: String,
}

/// Thành viên nhóm kèm thông tin hiển thị.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct GroupMemberInfo {
    pub user_id: Uuid,
    pub role: String,
    pub joined_at: DateTime<Utc>,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}
