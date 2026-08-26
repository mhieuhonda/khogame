use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Review {
    pub id: Uuid,
    pub game_id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub content: Option<String>,
    pub rating: i16,
    pub helpful_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Review kèm thông tin người viết.
///
/// `is_helpful` từng được populate từ bảng `review_helpful` (chưa từng được
/// tạo migration). Hiện dead code — giữ struct để tương lai khi wire-up
/// review UI, query cần thêm JOIN vào `review_helpful` mới tạo. Tránh reference
/// bảng không tồn tại trong production hiện tại.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ReviewWithUser {
    pub id: Uuid,
    pub game_id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub content: Option<String>,
    pub rating: i16,
    pub helpful_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub user_name: String,
    pub user_avatar: Option<String>,
}
