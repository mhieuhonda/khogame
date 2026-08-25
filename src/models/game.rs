use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[sqlx(type_name = "game_status", rename_all = "snake_case")]
#[derive(Default)]
pub enum GameStatus {
    Draft,
    #[default]
    Published,
    Archived,
    Hidden,
    PendingReview,
}

impl GameStatus {
    pub fn label(&self) -> &'static str {
        match self {
            GameStatus::Draft => "Bản nháp",
            GameStatus::Published => "Đã xuất bản",
            GameStatus::Archived => "Lưu trữ",
            GameStatus::Hidden => "Đã ẩn",
            GameStatus::PendingReview => "Chờ duyệt",
        }
    }
    /// Parse từ chuỗi (DB/JSON). Đặt tên khác `from_str` sẽ phá vỡ nhiều call site;
    /// tạm thời allow clippy::should_implement_trait.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "draft" => GameStatus::Draft,
            "published" => GameStatus::Published,
            "archived" => GameStatus::Archived,
            "hidden" => GameStatus::Hidden,
            "pending_review" => GameStatus::PendingReview,
            _ => GameStatus::Published,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[sqlx(type_name = "platform_type", rename_all = "lowercase")]
pub enum Platform {
    Android,
    Ios,
    Windows,
    Linux,
    Macos,
}

impl Platform {
    pub fn label(&self) -> &'static str {
        match self {
            Platform::Android => "Android",
            Platform::Ios => "iOS",
            Platform::Windows => "Windows",
            Platform::Linux => "Linux",
            Platform::Macos => "macOS",
        }
    }
    pub fn icon(&self) -> &'static str {
        match self {
            Platform::Android => "android",
            Platform::Ios => "apple",
            Platform::Windows => "windows",
            Platform::Linux => "linux",
            Platform::Macos => "apple",
        }
    }
    pub fn color(&self) -> &'static str {
        match self {
            Platform::Android => "#3DDC84",
            Platform::Ios => "#000000",
            Platform::Windows => "#0078D4",
            Platform::Linux => "#FCC624",
            Platform::Macos => "#555555",
        }
    }
    /// Parse từ chuỗi. Đổi tên `from_str` sẽ phá vỡ nhiều call site;
    /// tạm thời allow clippy::should_implement_trait.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "android" => Some(Platform::Android),
            "ios" => Some(Platform::Ios),
            "windows" => Some(Platform::Windows),
            "linux" => Some(Platform::Linux),
            "macos" | "mac" => Some(Platform::Macos),
            _ => None,
        }
    }
    pub fn all() -> &'static [Platform] {
        &[
            Platform::Android,
            Platform::Ios,
            Platform::Windows,
            Platform::Linux,
            Platform::Macos,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[sqlx(type_name = "age_rating", rename_all = "lowercase")]
#[derive(Default)]
pub enum AgeRating {
    #[default]
    Everyone,
    Teen,
    Mature,
    Adult,
}

impl AgeRating {
    pub fn label(&self) -> &'static str {
        match self {
            AgeRating::Everyone => "E - Mọi lứa tuổi",
            AgeRating::Teen => "T - Thiếu niên (13+)",
            AgeRating::Mature => "M - Trưởng thành (17+)",
            AgeRating::Adult => "A - Người lớn (18+)",
        }
    }
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "teen" => AgeRating::Teen,
            "mature" => AgeRating::Mature,
            "adult" => AgeRating::Adult,
            _ => AgeRating::Everyone,
        }
    }
    pub fn all() -> &'static [AgeRating] {
        &[
            AgeRating::Everyone,
            AgeRating::Teen,
            AgeRating::Mature,
            AgeRating::Adult,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Game {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub slug: String,
    pub excerpt: Option<String>,
    pub content: Option<String>,
    pub status: GameStatus,
    pub version: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub release_date: Option<chrono::NaiveDate>,
    pub file_size: Option<String>,
    pub age_rating: AgeRating,
    pub languages: Vec<String>,
    pub trailer_url: Option<String>,
    pub cover_image: Option<String>,
    pub category_id: Option<Uuid>,
    pub view_count: i32,
    pub download_count: i32,
    pub like_count: i32,
    pub comment_count: i32,
    pub share_count: i32,
    pub rating_avg: bigdecimal::BigDecimal,
    pub rating_count: i32,
    pub is_featured: bool,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Game {
    pub fn excerpt_or(&self) -> String {
        self.excerpt.clone().unwrap_or_default()
    }
    pub fn content_or(&self) -> String {
        self.content.clone().unwrap_or_default()
    }
    pub fn cover_or(&self, fallback: &str) -> String {
        self.cover_image
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| fallback.to_string())
    }
    pub fn rating_avg_f64(&self) -> f64 {
        use std::str::FromStr;
        f64::from_str(&self.rating_avg.to_string()).unwrap_or(0.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GameLink {
    pub id: Uuid,
    pub game_id: Uuid,
    pub platform: Platform,
    pub url: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GameScreenshot {
    pub id: Uuid,
    pub game_id: Uuid,
    pub url: String,
    pub caption: Option<String>,
    pub position: i32,
    pub created_at: DateTime<Utc>,
}

/// Game card with author info - for lists/cards
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GameCard {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub excerpt: Option<String>,
    pub cover_image: Option<String>,
    pub category_name: Option<String>,
    pub category_slug: Option<String>,
    pub author_name: String,
    pub author_avatar: Option<String>,
    pub view_count: i32,
    pub download_count: i32,
    pub like_count: i32,
    pub comment_count: i32,
    pub rating_avg: bigdecimal::BigDecimal,
    pub rating_count: i32,
    pub platforms: Vec<String>,
    pub published_at: Option<DateTime<Utc>>,
}

/// Dòng game cho bảng quản trị / "Game của tôi"
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AdminGameRow {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub status: GameStatus,
    pub view_count: i32,
    pub download_count: i32,
    pub like_count: i32,
    pub comment_count: i32,
    pub is_featured: bool,
    pub created_at: DateTime<Utc>,
    pub author_name: String,
    pub category_name: Option<String>,
}

impl GameCard {
    pub fn rating_avg_f64(&self) -> f64 {
        use std::str::FromStr;
        f64::from_str(&self.rating_avg.to_string()).unwrap_or(0.0)
    }
    pub fn cover_or(&self, fallback: &str) -> String {
        self.cover_image
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| fallback.to_string())
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GameForm {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub excerpt: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub developer: String,
    #[serde(default)]
    pub publisher: String,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub file_size: String,
    #[serde(default)]
    pub age_rating: String,
    #[serde(default)]
    pub languages: String,
    #[serde(default)]
    pub trailer_url: String,
    #[serde(default)]
    pub cover_image: String,
    #[serde(default)]
    pub category_id: Option<String>,
    /// Nhận trực tiếp từ <input name="tags"> (phân cách bằng dấu phẩy)
    #[serde(default)]
    pub tags: String,
    /// Mỗi dòng là 1 URL ảnh
    #[serde(default)]
    pub screenshots: String,
    #[serde(default)]
    pub android_link: Option<String>,
    #[serde(default)]
    pub ios_link: Option<String>,
    #[serde(default)]
    pub windows_link: Option<String>,
    #[serde(default)]
    pub linux_link: Option<String>,
    #[serde(default)]
    pub macos_link: Option<String>,
}

impl GameForm {
    /// Tách chuỗi tags phân cách dấu phẩy thành Vec.
    /// Dedupe case-insensitive và giữ thứ tự xuất hiện đầu — tránh tạo
    /// các bản ghi game_tags trùng lặp khi user nhập "action, Action".
    pub fn tags_vec(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        self.tags
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .filter(|s| seen.insert(s.to_lowercase()))
            .collect()
    }
    /// Tách chuỗi ngôn ngữ phân cách dấu phẩy thành Vec (dedupe như tags_vec)
    pub fn languages_vec(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        self.languages
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .filter(|s| seen.insert(s.to_lowercase()))
            .collect()
    }
    /// Mỗi dòng trong textarea là 1 URL screenshot
    pub fn screenshots_vec(&self) -> Vec<String> {
        self.screenshots
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tags_vec_dedupe_and_trim() {
        let mut form = GameForm::default();
        form.tags = " action , Action, 射击, action, RPG ".into();
        let tags = form.tags_vec();
        assert_eq!(tags, vec!["action".to_string(), "射击".to_string(), "RPG".to_string()]);
    }

    #[test]
    fn test_tags_vec_empty() {
        let form = GameForm::default();
        assert!(form.tags_vec().is_empty());
        let mut form = GameForm::default();
        form.tags = " , , ".into();
        assert!(form.tags_vec().is_empty());
    }

    #[test]
    fn test_languages_vec_dedupe() {
        let mut form = GameForm::default();
        form.languages = "vi, VI, en, vi".into();
        assert_eq!(
            form.languages_vec(),
            vec!["vi".to_string(), "en".to_string()]
        );
    }

    #[test]
    fn test_screenshots_vec_lines() {
        let mut form = GameForm::default();
        form.screenshots = "https://a.com/1.png\n  https://a.com/2.png  \n\n".into();
        assert_eq!(form.screenshots_vec().len(), 2);
    }

    #[test]
    fn test_platform_from_str() {
        assert_eq!(Platform::from_str("android"), Some(Platform::Android));
        assert_eq!(Platform::from_str("iOS"), Some(Platform::Ios));
        assert_eq!(Platform::from_str("WINDOWS"), Some(Platform::Windows));
        assert_eq!(Platform::from_str("mac"), Some(Platform::Macos));
        assert_eq!(Platform::from_str("macos"), Some(Platform::Macos));
        assert_eq!(Platform::from_str("ps5"), None);
        assert_eq!(Platform::from_str(""), None);
    }

    #[test]
    fn test_platform_all_has_5() {
        assert_eq!(Platform::all().len(), 5);
        // Mỗi label phải khác rỗng (dùng cho UI)
        for p in Platform::all() {
            assert!(!p.label().is_empty());
        }
    }

    #[test]
    fn test_game_status_from_str() {
        assert_eq!(GameStatus::from_str("draft"), GameStatus::Draft);
        assert_eq!(GameStatus::from_str("hidden"), GameStatus::Hidden);
        // Giá trị lạ → Published (default an toàn cho link cũ)
        assert_eq!(GameStatus::from_str("bất kỳ"), GameStatus::Published);
    }

    #[test]
    fn test_age_rating_from_str() {
        assert_eq!(AgeRating::from_str("teen"), AgeRating::Teen);
        assert_eq!(AgeRating::from_str("adult"), AgeRating::Adult);
        assert_eq!(AgeRating::from_str("xyz"), AgeRating::Everyone);
    }

    #[test]
    fn test_platform_label_vi_ui() {
        // Label hiển thị trên UI phải đúng chính tả
        assert_eq!(Platform::Ios.label(), "iOS");
        assert_eq!(Platform::Macos.label(), "macOS");
    }
}
