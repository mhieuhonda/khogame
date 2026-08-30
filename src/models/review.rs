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

/// Review kèm thông tin người viết (v2.9.0 wire-up).
///
/// - `is_helpful`: viewer đã vote "hữu ích" chưa (bảng
///   `review_helpful_votes` — migration 021).
/// - `author_xp`: tổng XP người viết — render chip cấp độ cạnh tên.
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
    pub user_username: String,
    pub user_avatar: Option<String>,
    pub is_helpful: bool,
    /// v3.1.0 — i64 (BIGINT, user_xp_totals.total_xp đã chuyển sang BIGINT).
    pub author_xp: i64,
}

impl ReviewWithUser {
    /// Chuỗi sao hiển thị "★★★☆☆" (askama không so sánh được i16 với
    /// integer literal trong vòng for — dựng sẵn trong Rust).
    #[must_use]
    pub fn stars(&self) -> String {
        let mut s = String::new();
        for i in 1..=5_i16 {
            s.push(if self.rating >= i {
                '\u{2605}'
            } else {
                '\u{2606}'
            });
        }
        s
    }

    /// Check radio "N KHÔNG" — dùng cho form sửa review.
    #[must_use]
    pub fn rating_is(&self, v: i32) -> bool {
        self.rating == v as i16
    }
}

impl Review {
    /// Check radio "N sao" cho form sửa review (askama không so sánh
    /// i16 với literal trong template).
    #[must_use]
    pub fn rating_is(&self, v: i32) -> bool {
        self.rating == v as i16
    }

    /// Tiêu đề đã điền sẵn cho form.
    #[must_use]
    pub fn title_or_default(&self) -> String {
        self.title.clone()
    }

    /// Nội dung đã điền sẵn cho form.
    #[must_use]
    pub fn content_or_default(&self) -> String {
        self.content.clone().unwrap_or_default()
    }
}
