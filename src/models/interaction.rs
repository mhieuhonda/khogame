use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[sqlx(type_name = "share_platform", rename_all = "lowercase")]
pub enum SharePlatform {
    Facebook,
    Twitter,
    Telegram,
    Whatsapp,
    Copy,
    Native,
}

impl SharePlatform {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "facebook" => SharePlatform::Facebook,
            "twitter" => SharePlatform::Twitter,
            "telegram" => SharePlatform::Telegram,
            "whatsapp" => SharePlatform::Whatsapp,
            "native" => SharePlatform::Native,
            _ => SharePlatform::Copy,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Download {
    pub id: Uuid,
    pub game_id: Uuid,
    pub user_id: Option<Uuid>,
    pub platform: String,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Share {
    pub id: Uuid,
    pub game_id: Uuid,
    pub user_id: Option<Uuid>,
    pub platform: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Rating {
    pub user_id: Uuid,
    pub game_id: Uuid,
    pub score: i16,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Bookmark {
    pub user_id: Uuid,
    pub game_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Follow {
    pub follower_id: Uuid,
    pub followee_id: Uuid,
    pub created_at: DateTime<Utc>,
}
