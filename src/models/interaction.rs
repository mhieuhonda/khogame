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
    /// Chuỗi enum DB (share_platform) — dùng khi bind cast $n::share_platform.
    pub fn as_str(&self) -> &'static str {
        match self {
            SharePlatform::Facebook => "facebook",
            SharePlatform::Twitter => "twitter",
            SharePlatform::Telegram => "telegram",
            SharePlatform::Whatsapp => "whatsapp",
            SharePlatform::Copy => "copy",
            SharePlatform::Native => "native",
        }
    }

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

    pub fn all() -> &'static [SharePlatform] {
        &[
            SharePlatform::Facebook,
            SharePlatform::Twitter,
            SharePlatform::Telegram,
            SharePlatform::Whatsapp,
            SharePlatform::Copy,
            SharePlatform::Native,
        ]
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Roundtrip as_str ↔ from_str cho mọi variant — bất biến mà
    /// record_share dựa vào (bind chuỗi enum phải parse ngược được
    /// trong cast $n::share_platform).
    #[test]
    fn test_share_platform_roundtrip() {
        for p in SharePlatform::all() {
            assert_eq!(SharePlatform::from_str(p.as_str()), *p);
        }
    }

    /// Case-insensitive + giá trị lạ → Copy (default an toàn: share
    /// vẫn được ghi nhận, không mất analytics khi client gửi chuỗi lạ).
    #[test]
    fn test_share_platform_from_str_aliases() {
        assert_eq!(SharePlatform::from_str("FACEBOOK"), SharePlatform::Facebook);
        assert_eq!(SharePlatform::from_str("Telegram"), SharePlatform::Telegram);
        assert_eq!(SharePlatform::from_str("weird"), SharePlatform::Copy);
        assert_eq!(SharePlatform::from_str(""), SharePlatform::Copy);
    }

    /// all() phải đủ 6 platform và as_str là lowercase snake — khớp
    /// rename_all = "lowercase" của sqlx Type derive.
    #[test]
    fn test_share_platform_all_and_str_format() {
        assert_eq!(SharePlatform::all().len(), 6);
        for p in SharePlatform::all() {
            let s = p.as_str();
            assert!(
                s.chars().all(|c| c.is_ascii_lowercase()),
                "as_str phải lowercase snake: {}",
                s
            );
        }
    }
}
