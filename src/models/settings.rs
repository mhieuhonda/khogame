use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Key-value site settings
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Setting {
    pub key: String,
    pub value: Option<String>,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Announcement {
    pub text: String,
    pub kind: String,
}

/// Audit log entry cho admin actions
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AdminLog {
    pub id: Uuid,
    pub admin_id: Uuid,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub detail: Option<String>,
    pub ip: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AdminLogWithAdmin {
    pub id: Uuid,
    pub admin_name: String,
    pub admin_username: String,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub detail: Option<String>,
    pub ip: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Dữ liệu chart 7 ngày cho dashboard
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DailyStatRow {
    pub day: chrono::NaiveDate,
    pub views: i64,
    pub downloads: i64,
    pub new_games: i64,
    pub new_users: i64,
}
