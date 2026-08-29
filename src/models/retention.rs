//! v3.0.0 — Models retention engine: nhiệm vụ, vòng quay, câu đố,
//! cửa hàng XP, referral, heatmap, tùy chọn thông báo, onboarding.
//!
//! Kiến trúc theo đúng convention 021/023: mọi bảng mới tách riêng,
//! không đụng `users`/`user_preferences`; cấp độ vẫn là hàm thuần của XP.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ============================================================
// QUESTS
// ============================================================

/// Định nghĩa 1 nhiệm vụ (bảng quest_catalog).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct QuestDef {
    pub id: String,
    pub title: String,
    pub description: String,
    pub icon: String,
    pub stat_key: String,
    pub target: i32,
    pub xp_reward: i32,
    pub period: String,
    pub is_active: bool,
}

/// Nhiệm vụ + tiến độ của user trong kỳ hiện tại (JOIN catalog + user_quests).
#[derive(Debug, Clone, Serialize)]
pub struct QuestWithProgress {
    pub quest: QuestDef,
    pub progress: i32,
    pub completed: bool,
    pub claimed: bool,
}

/// Một dòng gộp từ DB (progress có thể NULL khi user chưa có row).
#[derive(Debug, FromRow)]
pub struct QuestProgressRow {
    pub id: String,
    pub title: String,
    pub description: String,
    pub icon: String,
    pub stat_key: String,
    pub target: i32,
    pub xp_reward: i32,
    pub period: String,
    pub is_active: bool,
    pub progress: Option<i32>,
    pub completed_at: Option<DateTime<Utc>>,
    pub claimed_at: Option<DateTime<Utc>>,
}

impl QuestProgressRow {
    /// Gộp thành model render (owned — không mượn từ self).
    pub fn into_progress(self) -> QuestWithProgress {
        let completed = self.completed_at.is_some();
        let claimed = self.claimed_at.is_some();
        let quest = QuestDef {
            id: self.id,
            title: self.title,
            description: self.description,
            icon: self.icon,
            stat_key: self.stat_key,
            target: self.target,
            xp_reward: self.xp_reward,
            period: self.period,
            is_active: self.is_active,
        };
        QuestWithProgress {
            quest,
            progress: self.progress.unwrap_or(0).min(self.target),
            completed,
            claimed,
        }
    }
}

// ============================================================
// LUCKY SPIN
// ============================================================

/// Nhóm phần thưởng vòng quay (trọng số + XP).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpinPrize {
    pub xp: i32,
    pub weight: i32,
    /// Nhãn hiển thị trên bánh xe (màu theo tier).
    pub tier: &'static str,
}

impl SpinPrize {
    /// Bảng phần thưởng chuẩn. Tổng trọng số = 1000.
    /// Jackpot 500 XP cực hiếm (5/1000 = 0.5%).
    pub const TABLE: &'static [SpinPrize] = &[
        SpinPrize {
            xp: 5,
            weight: 300,
            tier: "common",
        },
        SpinPrize {
            xp: 10,
            weight: 250,
            tier: "common",
        },
        SpinPrize {
            xp: 15,
            weight: 180,
            tier: "common",
        },
        SpinPrize {
            xp: 20,
            weight: 130,
            tier: "rare",
        },
        SpinPrize {
            xp: 30,
            weight: 80,
            tier: "rare",
        },
        SpinPrize {
            xp: 50,
            weight: 40,
            tier: "epic",
        },
        SpinPrize {
            xp: 100,
            weight: 15,
            tier: "epic",
        },
        SpinPrize {
            xp: 500,
            weight: 5,
            tier: "legendary",
        },
    ];

    /// Chọn phần thưởng theo trọng số. `rand_val` phải trong 0..1000
    /// (hàm thuần — test được, caller sinh từ rand).
    pub fn pick(rand_val: i32) -> &'static SpinPrize {
        let mut acc = 0;
        for p in Self::TABLE {
            acc += p.weight;
            if rand_val < acc {
                return p;
            }
        }
        // rand_val >= tổng trọng số (không xảy ra với 0..1000) — fallback.
        Self::TABLE.first().expect("bảng spin rỗng")
    }
}

// ============================================================
// TRIVIA
// ============================================================

/// Câu hỏi câu đố cho user (KHÔNG kèm correct_index — chống gian lận
/// inspect HTML; đáp án chỉ chấm ở server).
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct TriviaQuestionPublic {
    pub id: i32,
    pub question: String,
    pub options: serde_json::Value,
}

impl TriviaQuestionPublic {
    /// Các lựa chọn dạng `Vec<String>` (askama không lặp được Value).
    #[must_use]
    pub fn options_list(&self) -> Vec<String> {
        self.options
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|v| v.as_str().unwrap_or_default().to_string())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Kết quả trả lời.
#[derive(Debug, Clone, Serialize)]
pub struct TriviaAnswerResult {
    pub question_id: i32,
    pub correct_index: i32,
    pub is_correct: bool,
    pub explanation: String,
    pub xp_awarded: i32,
    /// Mảng lựa chọn (để partial tô xanh đáp án đúng).
    pub options: serde_json::Value,
}

// ============================================================
// SHOP
// ============================================================

/// Vật phẩm cửa hàng (bảng shop_items).
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ShopItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub price: i32,
    pub kind: String,
    pub is_active: bool,
}

/// Vật phẩm + tồn kho của user (render trang shop).
#[derive(Debug, Clone, Serialize)]
pub struct ShopItemWithStock {
    pub item: ShopItem,
    pub owned: i32,
}

/// Kết quả mua hàng.
#[derive(Debug, Clone, Serialize)]
pub struct PurchaseOutcome {
    pub item_id: String,
    pub total_xp: i32,
    /// Mystery box: XP nhận được (0 với vật phẩm khác).
    pub mystery_xp: i32,
}

// ============================================================
// REFERRAL
// ============================================================

/// Thông tin referral cho trang /referral.
#[derive(Debug, Clone, Serialize)]
pub struct ReferralInfo {
    pub code: String,
    pub invited_count: i64,
    pub xp_earned: i64,
}

// ============================================================
// HEATMAP + LỊCH ĐIỂM DANH
// ============================================================

/// 1 ô heatmap hoạt động.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct HeatmapDay {
    pub day: NaiveDate,
    pub activity_count: i32,
}

/// 1 ô lịch điểm danh (ngày trong tháng).
#[derive(Debug, Clone, Serialize)]
pub struct CalendarDay {
    pub day: u32,
    pub checked_in: bool,
    pub is_today: bool,
    pub is_future: bool,
}

// ============================================================
// LEADERBOARD MÙA / TUẦN
// ============================================================

/// Entry bảng xếp hạng theo kỳ (tháng hoặc tuần) từ xp_events.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct SeasonEntry {
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub period_xp: i64,
}

// ============================================================
// NOTIFICATION PREFS
// ============================================================

/// Tùy chọn thông báo per-user (bảng user_notification_prefs).
/// Vắng row → tất cả TRUE trừ weekly_digest (FALSE) — khớp DEFAULT cột.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NotificationPrefs {
    pub user_id: Uuid,
    pub inapp_follow: bool,
    pub inapp_new_game: bool,
    pub inapp_review: bool,
    pub inapp_mention: bool,
    pub weekly_digest: bool,
}

impl Default for NotificationPrefs {
    fn default() -> Self {
        Self {
            user_id: Uuid::nil(),
            inapp_follow: true,
            inapp_new_game: true,
            inapp_review: true,
            inapp_mention: true,
            weekly_digest: false,
        }
    }
}

// ============================================================
// ONBOARDING
// ============================================================

/// 5 bước onboarding (mã step DB).
pub const ONBOARDING_STEPS: &[(&str, &str, &str, i32)] = &[
    ("avatar", "Đặt ảnh đại diện", "📸", 20),
    ("bio", "Viết giới thiệu bản thân", "✍️", 20),
    ("first_comment", "Viết bình luận đầu tiên", "💬", 20),
    ("first_bookmark", "Lưu 1 game vào danh sách", "🔖", 20),
    ("first_rating", "Đánh giá 1 game bằng sao", "⭐", 20),
];

/// Trạng thái 1 bước onboarding cho render.
#[derive(Debug, Clone, Serialize)]
pub struct OnboardingStepStatus {
    pub code: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    pub xp: i32,
    pub done: bool,
}

// ============================================================
// MILESTONE — GAME CỦA NGÀY / ĐẾM NGƯỢC
// ============================================================

/// Game của ngày: chọn deterministic theo ngày VN từ pool game published.
#[derive(Debug, Clone, Serialize)]
pub struct GameOfDay {
    /// Chỉ ngày (YYYY-MM-DD) — render "Game của ngày DD/MM".
    pub date_label: String,
    pub card: crate::models::game::GameCard,
}

/// Game sắp ra mắt (release_date trong tương lai) — đếm ngược.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct UpcomingGame {
    pub slug: String,
    pub title: String,
    pub cover_image: Option<String>,
    pub release_date: chrono::NaiveDate,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spin_pick_within_weights() {
        // Tổng trọng số phải đúng 1000 để pick(0..1000) luôn hợp lệ
        let total: i32 = SpinPrize::TABLE.iter().map(|p| p.weight).sum();
        assert_eq!(total, 1000);
    }

    #[test]
    fn test_spin_pick_distribution_boundaries() {
        // rand 0 → giải thường 5 XP; 999 → rơi vào nhóm nào đó hợp lệ
        assert_eq!(SpinPrize::pick(0).xp, 5);
        assert_eq!(SpinPrize::pick(299).xp, 5);
        assert_eq!(SpinPrize::pick(300).xp, 10);
        // Trên tổng → fallback phần thưởng đầu (an toàn, không panic)
        assert_eq!(SpinPrize::pick(10_000).xp, 5);
        // Jackpot chỉ khi rand chạm vùng cuối cùng
        let jackpot_start: i32 = SpinPrize::TABLE[..7].iter().map(|p| p.weight).sum();
        assert_eq!(SpinPrize::pick(jackpot_start).xp, 500);
        assert_eq!(SpinPrize::pick(jackpot_start - 1).xp, 100);
    }

    #[test]
    fn test_onboarding_steps_shape() {
        assert_eq!(ONBOARDING_STEPS.len(), 5);
        for (code, label, icon, xp) in ONBOARDING_STEPS {
            assert!(!code.is_empty());
            assert!(!label.is_empty());
            assert!(!icon.is_empty());
            assert!(*xp > 0);
        }
    }

    #[test]
    fn test_notification_prefs_default() {
        let p = NotificationPrefs::default();
        assert!(p.inapp_follow);
        assert!(p.inapp_new_game);
        assert!(p.inapp_review);
        assert!(p.inapp_mention);
        assert!(!p.weekly_digest);
    }
}
