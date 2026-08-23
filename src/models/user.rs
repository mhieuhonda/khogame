use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Default)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
pub enum UserRole {
    #[default]
    User,
    Moderator,
    Admin,
}

impl UserRole {
    pub fn is_admin(&self) -> bool {
        matches!(self, UserRole::Admin)
    }
    pub fn is_staff(&self) -> bool {
        matches!(self, UserRole::Admin | UserRole::Moderator)
    }
    pub fn label(&self) -> &'static str {
        match self {
            UserRole::User => "Thành viên",
            UserRole::Moderator => "Điều hành viên",
            UserRole::Admin => "Quản trị viên",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub google_sub: String,
    pub role: UserRole,
    pub is_banned: bool,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    pub fn bio_or(&self) -> String {
        self.bio.clone().unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserStats {
    pub games_count: i64,
    pub followers_count: i64,
    pub following_count: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, FromRow)]
pub struct UserPreference {
    pub theme: String,
    pub email_notifications: bool,
    pub show_online: bool,
    pub language: String,
}
