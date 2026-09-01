//! Model cho AI Agent account system.
//!
//! Bao gồm:
//! - [`AiAgentProfile`]: hồ sơ 1-1 với `users` (`model_name`, vendor, ...).
//! - [`AiProgressReport`]: báo cáo tiến trình từ AI.
//! - [`AiProgressReportWithAgent`]: báo cáo kèm thông tin AI để hiển thị.
//! - [`AiTaskStatus`]: enum trạng thái task (queued/running/done/failed/cancelled).
//!
//! v3.11.0 — THÔNG SỐ CẤU TRÚC: hệ params key/value tự do (`ai_agent_params`)
//! đã bị XÓA, thay bằng 7 trường spec cố định trên `ai_agent_profiles`:
//! developer / architecture / context_window / max_output / languages /
//! total_params / active_params (+ model_name, vendor, capabilities có sẵn).
//! * `total_params` — toàn bộ số lượng trọng số có trong mô hình AI.
//! * `active_params` — số lượng tham số thực tế được tính toán để xử lý
//!   một đầu vào tại một thời điểm (MoE chỉ kích hoạt subset expert).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

/// Trạng thái một task mà AI Agent báo cáo.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq, Default)]
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
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Queued => "Đã xếp hàng",
            Self::Running => "Đang chạy",
            Self::Done => "Hoàn thành",
            Self::Failed => "Lỗi",
            Self::Cancelled => "Đã huỷ",
        }
    }

    /// Màu sắc (CSS) cho badge trạng thái.
    #[must_use]
    pub const fn color(&self) -> &'static str {
        match self {
            Self::Queued => "#6b7280",
            Self::Running => "#3b82f6",
            Self::Done => "#10b981",
            Self::Failed => "#ef4444",
            Self::Cancelled => "#9ca3af",
        }
    }
}

/// Dữ liệu cập nhật hồ sơ AI Agent (v3.11.0 — cấu trúc hoá thay 17 tham
/// số rời: handlers truyền struct này vào `AiAgentRepo::update_profile`).
///
/// Spec mới đúng 10 trường hiển thị trên hồ sơ: Model (model_name),
/// Vendor, Khả năng (capabilities), Nhà phát triển (developer), Kiến trúc
/// (architecture), Cửa sổ ngữ cảnh (context_window), Output tối đa
/// (max_output), Ngôn ngữ (languages), Tổng tham số (total_params),
/// Tham số kích hoạt (active_params).
#[derive(Debug, Clone, Copy)]
pub struct AiProfileUpdate<'a> {
    pub model_name: &'a str,
    pub vendor: &'a str,
    pub version: &'a str,
    pub capabilities: &'a [String],
    pub privacy_level: &'a str,
    pub accent_color: &'a str,
    pub developer: &'a str,
    pub architecture: &'a str,
    pub context_window: &'a str,
    pub max_output: &'a str,
    pub languages: &'a str,
    pub total_params: &'a str,
    pub active_params: &'a str,
    pub bio: &'a str,
    /// `None`/rỗng = giữ nguyên avatar hiện tại (COALESCE).
    pub avatar_url: Option<&'a str>,
}

/// Hồ sơ AI Agent (1-1 với `users`).
///
/// v3.11.0 — 7 trường spec cấu trúc (thay thế bảng `ai_agent_params` cũ):
/// chỉ hiển thị khi `privacy_level = "public"`; giá trị rỗng = trường đó
/// được ẩn khỏi hồ sơ (AI/admin không bắt buộc khai báo đủ).
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
    /// Nhà phát triển (vd "Z.ai (Zhipu AI)")
    #[serde(default)]
    pub developer: String,
    /// Kiến trúc (vd "Mixture-of-Experts (MoE), 256 experts")
    #[serde(default)]
    pub architecture: String,
    /// Cửa sổ ngữ cảnh (vd "204.800 tokens (~200K)")
    #[serde(default)]
    pub context_window: String,
    /// Output tối đa mỗi lượt (vd "131.072 tokens (~128K)")
    #[serde(default)]
    pub max_output: String,
    /// Ngôn ngữ xử lý tốt (vd "Tiếng Việt, English, 中文")
    #[serde(default)]
    pub languages: String,
    /// Tổng tham số — toàn bộ số lượng trọng số có trong mô hình.
    #[serde(default)]
    pub total_params: String,
    /// Tham số kích hoạt — số tham số thực tế được tính toán để xử lý
    /// MỘT đầu vào tại một thời điểm (MoE active params).
    #[serde(default)]
    pub active_params: String,
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
    pub const fn from_str(s: &str) -> Self {
        if s.eq_ignore_ascii_case("anonymous") {
            Self::Anonymous
        } else {
            Self::Public
        }
    }
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Anonymous => "anonymous",
        }
    }
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Public => "Công khai",
            Self::Anonymous => "Ẩn danh",
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
    // v3.11.0 — structured spec (7 trường mới)
    #[serde(default)]
    pub developer: String,
    #[serde(default)]
    pub architecture: String,
    #[serde(default)]
    pub context_window: String,
    #[serde(default)]
    pub max_output: String,
    #[serde(default)]
    pub languages: String,
    #[serde(default)]
    pub total_params: String,
    #[serde(default)]
    pub active_params: String,
}

/// Thông tin mật khẩu đăng nhập của AI Agent (bảng `ai_agent_credentials`,
/// v3.4.0). `password_hash` KHÔNG bao giờ serialize ra template/API —
/// struct view riêng này chỉ mang dữ liệu hiển thị được.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AiAgentCredential {
    pub user_id: Uuid,
    /// Argon2id PHC string — CHỈ dùng nội bộ repo, không render ra UI.
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub password_expires_at: DateTime<Utc>,
    pub failed_attempts: i32,
    pub locked_until: Option<DateTime<Utc>>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Trạng thái mật khẩu AI Agent để hiển thị ở admin (không nhạy cảm).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialStatus {
    /// Còn hạn, chưa bị khoá
    Active,
    /// Đã hết hạn — admin cần đặt lại
    Expired,
    /// Đang bị khoá vì đăng nhập sai nhiều lần
    Locked,
    /// Chưa có mật khẩu (tài khoản cũ tạo qua /auth/ai/register)
    None,
}

impl AiAgentCredential {
    /// Trạng thái hiển thị (active/expired/locked) tính tại thời điểm hiện tại.
    #[must_use]
    pub fn status_at(&self, now: DateTime<Utc>) -> CredentialStatus {
        if let Some(until) = self.locked_until {
            if until > now {
                return CredentialStatus::Locked;
            }
        }
        if self.password_expires_at <= now {
            return CredentialStatus::Expired;
        }
        CredentialStatus::Active
    }
}

impl CredentialStatus {
    /// Nhãn tiếng Việt cho admin UI.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Active => "Còn hiệu lực",
            Self::Expired => "Đã hết hạn",
            Self::Locked => "Tạm khoá",
            Self::None => "Chưa đặt mật khẩu",
        }
    }

    /// Màu badge.
    #[must_use]
    pub const fn color(&self) -> &'static str {
        match self {
            Self::Active => "#10b981",
            Self::Expired => "#f59e0b",
            Self::Locked => "#ef4444",
            Self::None => "#6b7280",
        }
    }
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

    #[test]
    fn test_credential_status_at_variants() {
        use chrono::Duration;
        let now = Utc::now();
        let base = AiAgentCredential {
            user_id: Uuid::new_v4(),
            password_hash: "x".into(),
            password_expires_at: now + Duration::hours(24),
            failed_attempts: 0,
            locked_until: None,
            last_login_at: None,
            updated_by: None,
            created_at: now,
            updated_at: now,
        };
        assert_eq!(base.status_at(now), CredentialStatus::Active);
        // Hết hạn: expires_at trong quá khứ
        let mut expired = base.clone();
        expired.password_expires_at = now - Duration::hours(1);
        assert_eq!(expired.status_at(now), CredentialStatus::Expired);
        // Bị khoá: locked_until trong tương lai (dù mật khẩu còn hạn)
        let mut locked = base.clone();
        locked.locked_until = Some(now + Duration::minutes(10));
        assert_eq!(locked.status_at(now), CredentialStatus::Locked);
        // Khoá đã qua → trở lại active
        let mut unlocked = base.clone();
        unlocked.locked_until = Some(now - Duration::minutes(1));
        assert_eq!(unlocked.status_at(now), CredentialStatus::Active);
    }

    #[test]
    fn test_credential_status_labels() {
        for s in [
            CredentialStatus::Active,
            CredentialStatus::Expired,
            CredentialStatus::Locked,
            CredentialStatus::None,
        ] {
            assert!(!s.label().is_empty());
            let c = s.color();
            assert!(c.starts_with('#') && c.len() == 7, "color={c}");
        }
    }
}
