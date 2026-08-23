use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[sqlx(type_name = "game_status", rename_all = "snake_case")]
pub enum GameStatus {
    Draft,
    Published,
    Archived,
    Hidden,
    PendingReview,
}

impl Default for GameStatus {
    fn default() -> Self {
        GameStatus::Published
    }
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
pub enum AgeRating {
    Everyone,
    Teen,
    Mature,
    Adult,
}

impl Default for AgeRating {
    fn default() -> Self {
        AgeRating::Everyone
    }
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
        self.cover_image.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| fallback.to_string())
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

impl GameCard {
    pub fn rating_avg_f64(&self) -> f64 {
        use std::str::FromStr;
        f64::from_str(&self.rating_avg.to_string()).unwrap_or(0.0)
    }
    pub fn cover_or(&self, fallback: &str) -> String {
        self.cover_image.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| fallback.to_string())
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GameForm {
    pub title: String,
    pub excerpt: String,
    pub content: String,
    pub status: String,
    pub version: String,
    pub developer: String,
    pub publisher: String,
    pub release_date: Option<String>,
    pub file_size: String,
    pub age_rating: String,
    #[serde(default)]
    pub languages: Vec<String>,
    pub trailer_url: String,
    pub cover_image: String,
    pub category_id: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub screenshots: Vec<String>,
    pub android_link: Option<String>,
    pub ios_link: Option<String>,
    pub windows_link: Option<String>,
    pub linux_link: Option<String>,
    pub macos_link: Option<String>,
}
