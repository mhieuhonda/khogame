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
    #[must_use]
    pub fn ua_summary(&self) -> String {
        let ua = self.user_agent.as_deref().unwrap_or("");
        if ua.is_empty() {
            return "—".into();
        }
        // Chuẩn hoá các token UA phổ biến. Thứ tự quan trọng: Edge phải
        // trước Chrome (chuỗi Edg có chứa Chrome token), Chromium phải
        // trước Chrome (Chromium UA có thể không có 'Chrome/' riêng).
        let ua = if ua.contains("Edg/") {
            "Edge"
        } else if ua.contains("Chromium") {
            "Chromium"
        } else if ua.contains("Chrome/") {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn session_with_ua(ua: &str) -> SessionRow {
        SessionRow {
            id: Uuid::nil(),
            user_id: Uuid::nil(),
            username: "tester".into(),
            display_name: "Tester".into(),
            user_agent: if ua.is_empty() { None } else { Some(ua.into()) },
            ip_address: None,
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::days(30),
        }
    }

    #[test]
    fn test_ua_summary_detects_browsers() {
        // Edge phải nhận trước Chrome (chuỗi Edg có chứa Chrome token)
        assert_eq!(
            session_with_ua("Mozilla/5.0 Windows NT 10.0 Chrome/120.0.0.0 Edg/120.0.0.0")
                .ua_summary(),
            "Edge"
        );
        // Chromium nhận trước Chrome (UA có thể có cả 'Chrome/' và 'Chromium')
        assert_eq!(
            session_with_ua("Mozilla/5.0 X11; Linux x86_64 Chromium/120.0.0.0 Safari/537.36")
                .ua_summary(),
            "Chromium"
        );
        assert_eq!(
            session_with_ua("Mozilla/5.0 (X11; Linux x86_64) Chrome/119.0.0.0 Safari/537.36")
                .ua_summary(),
            "Chrome"
        );
        assert_eq!(
            session_with_ua("Mozilla/5.0 Firefox/121.0").ua_summary(),
            "Firefox"
        );
        assert_eq!(
            session_with_ua("Mozilla/5.0 iPhone Safari/604.1 Mobile/15E148").ua_summary(),
            "Safari Mobile"
        );
    }

    #[test]
    fn test_ua_summary_special_clients() {
        assert_eq!(
            session_with_ua("ai-agent-web").ua_summary(),
            "AI Agent (web)"
        );
        assert_eq!(session_with_ua("curl/8.5.0").ua_summary(), "curl");
        assert_eq!(
            session_with_ua("python-requests/2.31.0").ua_summary(),
            "Python client"
        );
        assert_eq!(
            session_with_ua("Go-http-client/2.0").ua_summary(),
            "Go client"
        );
    }

    #[test]
    fn test_ua_summary_empty_and_unknown() {
        // UA rỗng → dấu gạch (session do hệ thống tạo không kèm UA)
        assert_eq!(session_with_ua("").ua_summary(), "—");
        assert_eq!(session_with_ua("SomeWeirdBot/9.9").ua_summary(), "Khác");
    }
}
