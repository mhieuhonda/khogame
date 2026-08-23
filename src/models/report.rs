use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[sqlx(type_name = "report_status", rename_all = "snake_case")]
pub enum ReportStatus {
    Pending,
    Reviewing,
    Resolved,
    Dismissed,
}

impl ReportStatus {
    pub fn label(&self) -> &'static str {
        match self {
            ReportStatus::Pending => "Chờ xử lý",
            ReportStatus::Reviewing => "Đang xem xét",
            ReportStatus::Resolved => "Đã xử lý",
            ReportStatus::Dismissed => "Đã bỏ qua",
        }
    }
    pub fn color(&self) -> &'static str {
        match self {
            ReportStatus::Pending => "#f59e0b",
            ReportStatus::Reviewing => "#3b82f6",
            ReportStatus::Resolved => "#10b981",
            ReportStatus::Dismissed => "#6b7280",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
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
    pub fn label(&self) -> &'static str {
        match self {
            ReportReason::Spam => "Spam / Quảng cáo",
            ReportReason::Malware => "Chứa mã độc / virus",
            ReportReason::Copyright => "Vi phạm bản quyền",
            ReportReason::Inappropriate => "Nội dung không phù hợp",
            ReportReason::BrokenLink => "Link hỏng",
            ReportReason::Misleading => "Thông tin sai lệch",
            ReportReason::Illegal => "Nội dung vi phạm pháp luật",
            ReportReason::Other => "Lý do khác",
        }
    }
    pub fn all() -> Vec<ReportReason> {
        vec![
            ReportReason::Spam,
            ReportReason::Malware,
            ReportReason::Copyright,
            ReportReason::Inappropriate,
            ReportReason::BrokenLink,
            ReportReason::Misleading,
            ReportReason::Illegal,
            ReportReason::Other,
        ]
    }
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "spam" => Some(ReportReason::Spam),
            "malware" => Some(ReportReason::Malware),
            "copyright" => Some(ReportReason::Copyright),
            "inappropriate" => Some(ReportReason::Inappropriate),
            "broken_link" => Some(ReportReason::BrokenLink),
            "misleading" => Some(ReportReason::Misleading),
            "illegal" => Some(ReportReason::Illegal),
            "other" => Some(ReportReason::Other),
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
