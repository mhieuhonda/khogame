//! Model cho AI Agent account system.
//!
//! Bao gồm:
//! - [`AiAgentProfile`]: hồ sơ 1-1 với `users` (`model_name`, vendor, ...).
//! - [`AiProgressReport`]: báo cáo tiến trình từ AI.
//! - [`AiProgressReportWithAgent`]: báo cáo kèm thông tin AI để hiển thị.
//! - [`AiTaskStatus`]: enum trạng thái task (queued/running/done/failed/cancelled).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

/// Trạng thái một task mà AI Agent báo cáo.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Default)]
#[sqlx(type_name = "ai_task_status", rename_all = "lowercase")]
pub enum AiTaskStatus {
    #[default]
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
}

impl AiTaskStatus {
    /// Nhãn tiếng Việt hiển thị ra UI.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            AiTaskStatus::Queued => "Đã xếp hàng",
            AiTaskStatus::Running => "Đang chạy",
            AiTaskStatus::Done => "Hoàn thành",
            AiTaskStatus::Failed => "Lỗi",
            AiTaskStatus::Cancelled => "Đã huỷ",
        }
    }

    /// Màu sắc (CSS) cho badge trạng thái.
    #[must_use]
    pub fn color(&self) -> &'static str {
        match self {
            AiTaskStatus::Queued => "#6b7280",
            AiTaskStatus::Running => "#3b82f6",
            AiTaskStatus::Done => "#10b981",
            AiTaskStatus::Failed => "#ef4444",
            AiTaskStatus::Cancelled => "#9ca3af",
        }
    }
}

/// Hồ sơ AI Agent (1-1 với `users`).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AiAgentProfile {
    pub user_id: Uuid,
    pub model_name: String,
    pub vendor: String,
    pub version: String,
    pub capabilities: Vec<String>,
    /// "public" hoặc "anonymous"
    pub privacy_level: String,
    pub accent_color: String,
    pub verified: bool,
    pub last_active_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Hạng riêng tư (privacy) của hồ sơ AI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiPrivacyLevel {
    Public,
    Anonymous,
}

impl AiPrivacyLevel {
    /// Parse từ chuỗi ("public"/"anonymous"). Case-insensitive.
    /// Mặc định Public nếu không nhận diện được.
    #[allow(clippy::should_implement_trait)]
    #[must_use]
    pub fn from_str(s: &str) -> Self {
        if s.eq_ignore_ascii_case("anonymous") {
            AiPrivacyLevel::Anonymous
        } else {
            AiPrivacyLevel::Public
        }
    }
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            AiPrivacyLevel::Public => "public",
            AiPrivacyLevel::Anonymous => "anonymous",
        }
    }
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            AiPrivacyLevel::Public => "Công khai",
            AiPrivacyLevel::Anonymous => "Ẩn danh",
        }
    }
}

/// Một báo cáo tiến trình do AI Agent gửi về.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AiProgressReport {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub task: String,
    pub action: String,
    pub percentage: i16,
    pub status: AiTaskStatus,
    pub message: String,
    pub metadata: serde_json::Value,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Báo cáo tiến trình kèm thông tin AI (để render lên trang admin).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AiProgressReportWithAgent {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub task: String,
    pub action: String,
    pub percentage: i16,
    pub status: AiTaskStatus,
    pub message: String,
    pub metadata: serde_json::Value,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // Cột nối từ users + ai_agent_profiles
    pub agent_username: String,
    pub agent_display_name: String,
    pub agent_avatar_url: Option<String>,
    pub agent_model_name: String,
    pub agent_vendor: String,
}

/// Một token của AI Agent (chỉ lưu hash trong DB).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AiAgentToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub label: String,
    pub revoked: bool,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub ip_address: Option<String>,
    pub user_agent: String,
    pub created_at: DateTime<Utc>,
}

/// AI Agent + profile gộp (cho trang list admin).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AiAgentWithProfile {
    // users
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub is_banned: bool,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: Option<DateTime<Utc>>,
    // ai_agent_profiles
    pub model_name: String,
    pub vendor: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub privacy_level: String,
    pub accent_color: String,
    pub verified: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_privacy_level_roundtrip() {
        for lvl in [AiPrivacyLevel::Public, AiPrivacyLevel::Anonymous] {
            assert_eq!(AiPrivacyLevel::from_str(lvl.as_str()), lvl);
        }
        // Case-insensitive
        assert_eq!(
            AiPrivacyLevel::from_str("ANONYMOUS"),
            AiPrivacyLevel::Anonymous
        );
        assert_eq!(AiPrivacyLevel::from_str("Public"), AiPrivacyLevel::Public);
        // Giá trị lạ → mặc định Public (an toàn cho hiển thị)
        assert_eq!(AiPrivacyLevel::from_str("bất kỳ"), AiPrivacyLevel::Public);
        assert_eq!(AiPrivacyLevel::from_str(""), AiPrivacyLevel::Public);
    }

    #[test]
    fn test_privacy_labels() {
        assert_eq!(AiPrivacyLevel::Public.label(), "Công khai");
        assert_eq!(AiPrivacyLevel::Anonymous.label(), "Ẩn danh");
    }

    #[test]
    fn test_task_status_labels_and_colors() {
        for s in [
            AiTaskStatus::Queued,
            AiTaskStatus::Running,
            AiTaskStatus::Done,
            AiTaskStatus::Failed,
            AiTaskStatus::Cancelled,
        ] {
            assert!(!s.label().is_empty());
            let c = s.color();
            assert!(
                c.starts_with('#') && c.len() == 7,
                "color hex hợp lệ, got {c}"
            );
        }
    }

    #[test]
    fn test_default_task_status_is_queued() {
        assert_eq!(AiTaskStatus::default(), AiTaskStatus::Queued);
    }
}
