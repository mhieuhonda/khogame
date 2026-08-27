use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Một thể loại tin tức — admin CRUD qua trang /admin/news-categories.
/// Khác với `crate::models::category::Category` (thể loại GAME),
/// bảng `news_categories` dành riêng cho tin tức.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NewsCategory {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub icon: String,
    pub sort_order: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// NewsCategory + số tin tức thuộc category (cho bảng admin).
/// Count `i64` — `COUNT(news.id)::bigint` từ SQL.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NewsCategoryWithCount {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub icon: String,
    pub sort_order: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub news_count: i64,
}

impl NewsCategory {
    /// Trả về (key, label) tuple cho template select — match interface
    /// với `NEWS_CATEGORIES` cũ để `news/new.html` không phải đổi pattern.
    #[must_use]
    pub fn key_label(&self) -> (String, String) {
        (self.slug.clone(), self.name.clone())
    }
}
