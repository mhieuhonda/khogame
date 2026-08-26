use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[sqlx(type_name = "report_status", rename_all = "snake_case")]
pub enum ReportStatus {
    Pending,
    Reviewing,
    Resolved,
    Dismissed,
}

impl ReportStatus {
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Pending => "Chờ xử lý",
            Self::Reviewing => "Đang xem xét",
            Self::Resolved => "Đã xử lý",
            Self::Dismissed => "Đã bỏ qua",
        }
    }
    #[must_use]
    pub const fn color(&self) -> &'static str {
        match self {
            Self::Pending => "#f59e0b",
            Self::Reviewing => "#3b82f6",
            Self::Resolved => "#10b981",
            Self::Dismissed => "#6b7280",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[sqlx(type_name = "report_reason", rename_all = "snake_case")]
pub enum ReportReason {
    Spam,
    Malware,
    Copyright,
    Inappropriate,
    BrokenLink,
    Misleading,
    Illegal,
    Other,
}

impl ReportReason {
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Spam => "Spam / Quảng cáo",
            Self::Malware => "Chứa mã độc / virus",
            Self::Copyright => "Vi phạm bản quyền",
            Self::Inappropriate => "Nội dung không phù hợp",
            Self::BrokenLink => "Link hỏng",
            Self::Misleading => "Thông tin sai lệch",
            Self::Illegal => "Nội dung vi phạm pháp luật",
            Self::Other => "Lý do khác",
        }
    }
    #[must_use]
    pub fn all() -> Vec<Self> {
        vec![
            Self::Spam,
            Self::Malware,
            Self::Copyright,
            Self::Inappropriate,
            Self::BrokenLink,
            Self::Misleading,
            Self::Illegal,
            Self::Other,
        ]
    }
    #[allow(clippy::should_implement_trait)]
    #[must_use]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "spam" => Some(Self::Spam),
            "malware" => Some(Self::Malware),
            "copyright" => Some(Self::Copyright),
            "inappropriate" => Some(Self::Inappropriate),
            "broken_link" => Some(Self::BrokenLink),
            "misleading" => Some(Self::Misleading),
            "illegal" => Some(Self::Illegal),
            "other" => Some(Self::Other),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Report {
    pub id: Uuid,
    pub game_id: Uuid,
    pub reporter_id: Uuid,
    pub reason: ReportReason,
    pub description: Option<String>,
    pub status: ReportStatus,
    pub resolved_by: Option<Uuid>,
    pub resolution: Option<String>,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ReportWithGame {
    pub id: Uuid,
    pub game_id: Uuid,
    pub game_title: String,
    pub game_slug: String,
    pub reporter_id: Uuid,
    pub reporter_name: String,
    pub reason: ReportReason,
    pub description: Option<String>,
    pub status: ReportStatus,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_reason_from_str_all_variants() {
        for r in ReportReason::all() {
            // Mỗi variant phải parse được từ snake_case của chính nó
            let key = match r {
                ReportReason::Spam => "spam",
                ReportReason::Malware => "malware",
                ReportReason::Copyright => "copyright",
                ReportReason::Inappropriate => "inappropriate",
                ReportReason::BrokenLink => "broken_link",
                ReportReason::Misleading => "misleading",
                ReportReason::Illegal => "illegal",
                ReportReason::Other => "other",
            };
            assert_eq!(ReportReason::from_str(key), Some(r.clone()), "key={key}");
            // Case-insensitive
            assert_eq!(
                ReportReason::from_str(&key.to_uppercase()),
                Some(r.clone()),
                "uppercase key={key}"
            );
        }
    }

    #[test]
    fn test_report_reason_rejects_unknown() {
        assert_eq!(ReportReason::from_str("hacker"), None);
        assert_eq!(ReportReason::from_str(""), None);
        assert_eq!(ReportReason::from_str("brokenlink"), None); // thiếu underscore
    }

    #[test]
    fn test_all_reasons_have_labels() {
        for r in ReportReason::all() {
            assert!(!r.label().is_empty());
        }
        assert_eq!(ReportReason::all().len(), 8);
    }

    #[test]
    fn test_status_colors_valid_hex() {
        for s in [
            ReportStatus::Pending,
            ReportStatus::Reviewing,
            ReportStatus::Resolved,
            ReportStatus::Dismissed,
        ] {
            let c = s.color();
            assert!(c.starts_with('#') && c.len() == 7, "color={c} phải là hex");
            assert!(!s.label().is_empty());
        }
    }
}
