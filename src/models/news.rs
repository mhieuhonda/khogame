use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

/// Trạng thái vòng đời của một bài tin tức.
///
/// Workflow:
/// - User đăng tin mới → `Pending` (chờ admin duyệt)
/// - Admin duyệt → `Published` (công khai, hiện trên /news)
/// - Admin từ chối → `Rejected` (kèm review_note lý do)
/// - Admin lưu trữ tin cũ → `Archived` (ẩn khỏi list chính, vẫn truy cập qua direct link)
/// - Draft: status này dành cho tương lai khi có autosave (chưa dùng trong v0.8.0)
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[sqlx(type_name = "news_status", rename_all = "snake_case")]
pub enum NewsStatus {
    Draft,
    #[default]
    Pending,
    Published,
    Archived,
    Rejected,
}

impl Default for NewsStatus {
    fn default() -> Self {
        NewsStatus::Pending
    }
}

impl NewsStatus {
    pub fn label(&self) -> &'static str {
        match self {
            NewsStatus::Draft => "Bản nháp",
            NewsStatus::Pending => "Chờ duyệt",
            NewsStatus::Published => "Đã xuất bản",
            NewsStatus::Archived => "Lưu trữ",
            NewsStatus::Rejected => "Bị từ chối",
        }
    }

    /// True nếu status này có thể hiển thị công khai với mọi người.
    /// Pending/Draft/Rejected chỉ tác giả + admin xem được.
    pub fn is_public(&self) -> bool {
        matches!(self, NewsStatus::Published | NewsStatus::Archived)
    }

    /// True nếu cần admin xử lý (xuất hiện trong queue /admin/news/pending).
    pub fn needs_review(&self) -> bool {
        matches!(self, NewsStatus::Pending)
    }

    /// Badge CSS class cho template — để admin dashboard render màu sắc khác nhau.
    pub fn badge_class(&self) -> &'static str {
        match self {
            NewsStatus::Draft => "badge-neutral",
            NewsStatus::Pending => "badge-warning",
            NewsStatus::Published => "badge-success",
            NewsStatus::Archived => "badge-muted",
            NewsStatus::Rejected => "badge-danger",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "draft" => NewsStatus::Draft,
            "pending" => NewsStatus::Pending,
            "published" => NewsStatus::Published,
            "archived" => NewsStatus::Archived,
            "rejected" => NewsStatus::Rejected,
            _ => NewsStatus::Pending,
        }
    }
}

/// Một bài tin tức do người dùng đăng.
///
/// `author_ip` và `author_ua` chỉ admin xem được (xem `NewsForAdmin`);
/// repository tách hai struct để tránh lộ field nhạy cảm cho moderator.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct News {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub slug: String,
    pub excerpt: String,
    pub content: String,
    pub cover_image: Option<String>,
    pub category: String,
    pub source_url: String,
    pub source_name: String,
    pub status: NewsStatus,
    pub author_ip: Option<String>,
    pub author_ua: Option<String>,
    pub reviewed_by: Option<Uuid>,
    pub review_note: String,
    pub view_count: i32,
    pub like_count: i32,
    pub comment_count: i32,
    pub is_featured: bool,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Danh sách tin tức kèm thông tin tác giả (join users).
/// Dùng cho list trang /news, không chứa trường nhạy cảm.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NewsWithAuthor {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub slug: String,
    pub excerpt: String,
    pub cover_image: Option<String>,
    pub category: String,
    pub source_name: String,
    pub status: NewsStatus,
    pub view_count: i32,
    pub like_count: i32,
    pub comment_count: i32,
    pub is_featured: bool,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub author_name: String,
    pub author_username: String,
    pub author_avatar: Option<String>,
}

/// Tin tức kèm thông tin tác giả đầy đủ + IP/UA (chỉ admin xem được).
/// Moderator không bao giờ thấy struct này.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NewsForAdmin {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub slug: String,
    pub excerpt: String,
    pub content: String,
    pub cover_image: Option<String>,
    pub category: String,
    pub source_url: String,
    pub source_name: String,
    pub status: NewsStatus,
    pub author_ip: Option<String>,
    pub author_ua: Option<String>,
    pub reviewed_by: Option<Uuid>,
    pub review_note: String,
    pub view_count: i32,
    pub like_count: i32,
    pub comment_count: i32,
    pub is_featured: bool,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub author_name: String,
    pub author_username: String,
    pub author_email: String,
    pub author_avatar: Option<String>,
}

/// Comment trên một bài tin tức.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NewsComment {
    pub id: Uuid,
    pub news_id: Uuid,
    pub user_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub content: String,
    pub like_count: i32,
    pub is_pinned: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Comment + thông tin tác giả (cho template).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NewsCommentWithAuthor {
    pub id: Uuid,
    pub news_id: Uuid,
    pub user_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub content: String,
    pub like_count: i32,
    pub is_pinned: bool,
    pub created_at: DateTime<Utc>,
    pub author_name: String,
    pub author_username: String,
    pub author_avatar: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_news_status_default_is_pending() {
        // Default = Pending vì user đăng tin luôn phải qua admin duyệt.
        // Tránh trường hợp user đăng tin tự động xuất bản (tin giả).
        assert_eq!(NewsStatus::default(), NewsStatus::Pending);
        assert!(NewsStatus::default().needs_review());
    }

    #[test]
    fn test_news_status_visibility() {
        // Pending / Draft / Rejected: KHÔNG public (chỉ tác giả + admin)
        assert!(!NewsStatus::Pending.is_public());
        assert!(!NewsStatus::Draft.is_public());
        assert!(!NewsStatus::Rejected.is_public());
        // Published / Archived: public
        assert!(NewsStatus::Published.is_public());
        assert!(NewsStatus::Archived.is_public());
    }

    #[test]
    fn test_needs_review_only_pending() {
        // Chỉ Pending mới xuất hiện trong queue admin duyệt.
        assert!(NewsStatus::Pending.needs_review());
        assert!(!NewsStatus::Draft.needs_review());
        assert!(!NewsStatus::Published.needs_review());
        assert!(!NewsStatus::Archived.needs_review());
        assert!(!NewsStatus::Rejected.needs_review());
    }

    #[test]
    fn test_news_status_label() {
        assert_eq!(NewsStatus::Pending.label(), "Chờ duyệt");
        assert_eq!(NewsStatus::Published.label(), "Đã xuất bản");
        assert_eq!(NewsStatus::Rejected.label(), "Bị từ chối");
        assert_eq!(NewsStatus::Archived.label(), "Lưu trữ");
        assert_eq!(NewsStatus::Draft.label(), "Bản nháp");
    }

    #[test]
    fn test_news_status_badge_class() {
        // Đảm bảo mỗi status có badge riêng để admin dashboard phân biệt trực quan
        assert_eq!(NewsStatus::Pending.badge_class(), "badge-warning");
        assert_eq!(NewsStatus::Published.badge_class(), "badge-success");
        assert_eq!(NewsStatus::Rejected.badge_class(), "badge-danger");
    }

    #[test]
    fn test_news_status_from_str() {
        // Parse từ chuỗi DB / form
        assert_eq!(NewsStatus::from_str("pending"), NewsStatus::Pending);
        assert_eq!(NewsStatus::from_str("published"), NewsStatus::Published);
        assert_eq!(NewsStatus::from_str("rejected"), NewsStatus::Rejected);
        // Unknown → mặc định Pending (fail-safe: tin lạ coi như cần duyệt)
        assert_eq!(NewsStatus::from_str("invalid"), NewsStatus::Pending);
    }
}
