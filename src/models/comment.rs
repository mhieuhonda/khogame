use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Comment {
    pub id: Uuid,
    pub game_id: Uuid,
    pub user_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub content: String,
    pub like_count: i32,
    pub is_pinned: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CommentWithUser {
    pub id: Uuid,
    pub game_id: Uuid,
    pub user_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub content: String,
    pub like_count: i32,
    pub is_pinned: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub user_name: String,
    pub user_avatar: Option<String>,
    pub is_liked: bool,
}

/// Comment cho bảng quản trị — gộp bình luận GAME (bảng `comments`) và
/// bình luận TIN TỨC (bảng `news_comments`) vào một danh sách duy nhất.
///
/// TRƯỚC ĐÂY (bug): admin comments chỉ truy vấn bảng `comments` JOIN
/// `games` → bình luận trên trang tin tức (news_comments) KHÔNG BAO GIỜ
/// xuất hiện ở trang quản lý bình luận — admin không thể xoá/ghim bình
/// luận tin tức dù user vẫn bình luận được.
///
/// `kind` phân biệt nguồn ("game" / "news") để template link đúng trang
/// (`/games/{slug}` hoặc `/news/{slug}`) và handler pin/delete biết bảng
/// nào cần thao tác.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CommentWithGame {
    pub id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub is_pinned: bool,
    pub created_at: DateTime<Utc>,
    pub user_name: String,
    /// "game" (bảng comments) hoặc "news" (bảng news_comments)
    pub kind: String,
    /// Tiêu đề game hoặc tin tức chứa bình luận
    pub item_title: String,
    /// Slug game hoặc tin tức
    pub item_slug: String,
}

impl CommentWithGame {
    /// URL đến trang chứa bình luận — dùng trong template admin thay vì
    /// hard-code `/games/{slug}` (news comment phải link `/news/{slug}`).
    #[must_use]
    pub fn item_url(&self) -> String {
        if self.kind == "news" {
            format!("/news/{}", self.item_slug)
        } else {
            format!("/games/{}", self.item_slug)
        }
    }

    /// Nhãn loại nội dung cho hiển thị (admin nhận diện nhanh nguồn).
    #[must_use]
    pub fn kind_label(&self) -> &'static str {
        if self.kind == "news" {
            "📰 Tin tức"
        } else {
            "🎮 Game"
        }
    }
}
