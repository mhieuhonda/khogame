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

/// Session đang hoạt động kèm user — cho trang quản trị phiên
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SessionRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub display_name: String,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl SessionRow {
    /// User-Agent rút gọn để hiển thị (loại bỏ chuỗi version dài lê thê
    /// của Chrome/Firefox, chỉ giữ nền tảng chính).
    pub fn ua_summary(&self) -> String {
        let ua = self.user_agent.as_deref().unwrap_or("");
        if ua.is_empty() {
            return "—".into();
        }
        // Chuẩn hoá các token UA phổ biến
        let ua = if ua.contains("Edg/") {
            "Edge"
        } else if ua.contains("Chrome/") && !ua.contains("Chromium") {
            "Chrome"
        } else if ua.contains("Firefox/") {
            "Firefox"
        } else if ua.contains("Safari/") && ua.contains("Mobile") {
            "Safari Mobile"
        } else if ua.contains("Safari/") {
            "Safari"
        } else if ua.contains("ai-agent-web") {
            "AI Agent (web)"
        } else if ua.to_lowercase().contains("curl") {
            "curl"
        } else if ua.to_lowercase().contains("python") {
            "Python client"
        } else if ua.to_lowercase().contains("go-http") {
            "Go client"
        } else {
            "Khác"
        };
        ua.to_string()
    }
}
