//! v2.9.0 — Gamification models: XP, cấp độ, điểm danh, huy hiệu.
//!
//! Kiến trúc:
//! - `user_xp_totals` cache tổng XP (đọc O(1) cho chip cấp độ ở
//!   comment/chat/profile).
//! - `xp_events` append-only — kiêm activity feed trên hồ sơ.
//! - `daily_checkins` điểm danh + chuỗi ngày liên tiếp.
//! - `achievements` catalog (seed migration 021) + `user_achievements`
//!   (kèm `is_showcased` để ghim tối đa 3 huy hiệu lên hồ sơ).
//!
//! Cấp độ là HÀM THUẦN của tổng XP (không lưu DB) — đổi ngưỡng chỉ cần
//! đổi bảng LEVELS, toàn site tự cập nhật.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Bảng ngưỡng XP → danh hiệu. Index = level - 1.
/// (ngưỡng XP TÍCH LŨY, danh hiệu tiếng Việt)
pub const LEVELS: &[(i32, &str)] = &[
    (0, "Tân Binh"),
    (100, "Tập Sự"),
    (250, "Thám Hiểm"),
    (500, "Chiến Binh"),
    (900, "Cao Thủ"),
    (1400, "Đấu Sĩ"),
    (2100, "Kỳ Lão"),
    (3000, "Bậc Thầy"),
    (4200, "Đại Sư"),
    (6000, "Huyền Thoại"),
    (8500, "Vinh Quang"),
    (12000, "Bất Tử"),
];

/// Số huy hiệu tối đa được ghim (showcase) lên hồ sơ.
pub const MAX_SHOWCASED_ACHIEVEMENTS: i32 = 3;

/// Thông tin cấp độ tính từ tổng XP.
#[derive(Debug, Clone, Serialize)]
pub struct LevelInfo {
    /// Cấp độ hiện tại (1-based).
    pub level: i32,
    /// Danh hiệu của cấp hiện tại.
    pub title: &'static str,
    /// Tổng XP hiện tại.
    pub xp: i32,
    /// XP cần để đạt cấp kế (None nếu đã max).
    pub next_level_xp: Option<i32>,
    /// Tiến độ tới cấp kế, 0-100 (100 nếu max).
    pub progress_pct: i32,
}

/// Tính cấp độ + danh hiệu + tiến độ từ tổng XP (hàm thuần, test được).
#[must_use]
pub fn level_from_xp(xp: i32) -> LevelInfo {
    let xp = xp.max(0);
    // Tìm level cao nhất có ngưỡng <= xp
    let mut idx = 0usize;
    for (i, (threshold, _)) in LEVELS.iter().enumerate() {
        if xp >= *threshold {
            idx = i;
        }
    }
    let (cur_threshold, title) = LEVELS[idx];
    let next = LEVELS.get(idx + 1);
    let (next_level_xp, progress_pct) = match next {
        Some((next_threshold, _)) => {
            let span = next_threshold - cur_threshold;
            let done = xp - cur_threshold;
            (
                Some(*next_threshold),
                ((done * 100) / span.max(1)).clamp(0, 100),
            )
        }
        None => (None, 100),
    };
    LevelInfo {
        level: idx as i32 + 1,
        title,
        xp,
        next_level_xp,
        progress_pct,
    }
}

/// Cache tổng XP của 1 user (bảng user_xp_totals).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserXpTotal {
    pub user_id: Uuid,
    pub total_xp: i32,
    pub updated_at: DateTime<Utc>,
}

/// 1 dòng điểm danh (bảng daily_checkins).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DailyCheckin {
    pub user_id: Uuid,
    pub checkin_date: chrono::NaiveDate,
    pub streak: i32,
    pub xp_awarded: i32,
    pub created_at: DateTime<Utc>,
}

/// Catalog huy hiệu (bảng achievements).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Achievement {
    pub id: String,
    pub title: String,
    pub description: String,
    pub icon: String,
    pub xp_reward: i32,
    pub category: String,
    pub sort_order: i32,
    pub is_active: bool,
}

/// Huy hiệu kèm trạng thái của 1 user (earned_at = None → chưa đạt).
#[derive(Debug, Clone, Serialize)]
pub struct AchievementWithStatus {
    pub achievement: Achievement,
    pub earned_at: Option<DateTime<Utc>>,
    pub is_showcased: bool,
}

/// Dòng bảng xếp hạng cấp độ.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LeaderboardEntry {
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub total_xp: i32,
    pub games_count: i64,
    pub streak: i64,
}

/// Entry activity feed hồ sơ — render từ xp_events.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ActivityEvent {
    pub reason: String,
    pub amount: i32,
    pub created_at: DateTime<Utc>,
}

impl ActivityEvent {
    /// Nhãn tiếng Việt cho activity feed theo reason XP.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self.reason.as_str() {
            "daily_checkin" => "đã điểm danh hàng ngày",
            "post_game" => "đã đăng một game mới",
            "post_news" => "có bài tin được duyệt đăng",
            "comment" => "đã bình luận",
            "review" => "đã viết review",
            "repo" => "đã chia sẻ một repo",
            "chat_message" => "đã trò chuyện trong chat",
            "received_like" => "nhận được một lượt thích",
            "received_follow" => "có người theo dõi mới",
            "received_download" => "game nhận một lượt tải",
            "achievement" => "mở khóa huy hiệu",
            "level_up" => "lên cấp độ mới",
            _ => "có hoạt động mới",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_boundaries() {
        // Level 1 tại 0 XP
        let l = level_from_xp(0);
        assert_eq!((l.level, l.title), (1, "Tân Binh"));
        assert_eq!(l.next_level_xp, Some(100));
        assert_eq!(l.progress_pct, 0);

        // 99 XP vẫn level 1
        let l = level_from_xp(99);
        assert_eq!(l.level, 1);
        assert_eq!(l.progress_pct, 99);

        // 100 XP → level 2
        let l = level_from_xp(100);
        assert_eq!((l.level, l.title), (2, "Tập Sự"));

        // 249 → vẫn 2; 250 → 3
        assert_eq!(level_from_xp(249).level, 2);
        assert_eq!(level_from_xp(250).level, 3);

        // Ngưỡng giữa các cấp
        assert_eq!(level_from_xp(899).level, 4);
        assert_eq!(level_from_xp(900).level, 5);

        // Max level: xp >= ngưỡng cuối
        let l = level_from_xp(12000);
        assert_eq!((l.level, l.title), (12, "Bất Tử"));
        assert_eq!(l.next_level_xp, None);
        assert_eq!(l.progress_pct, 100);

        let l = level_from_xp(999_999);
        assert_eq!(l.level, 12);

        // XP âm (không thể xảy ra trong DB do CHECK, phòng hộ) → coi như 0
        assert_eq!(level_from_xp(-5).level, 1);
    }

    #[test]
    fn test_level_progress_pct() {
        // Giữa level 1 (0) và level 2 (100): 50 XP → 50%
        assert_eq!(level_from_xp(50).progress_pct, 50);
        // Giữa level 5 (900) và 6 (1400): 1150 XP → 50%
        assert_eq!(level_from_xp(1150).progress_pct, 50);
        // Giữa level 6 (1400) và 7 (2100): 1750 XP → 50%
        let l = level_from_xp(1750);
        assert_eq!(l.level, 6);
        assert_eq!(l.progress_pct, 50);
        // 1900 nằm giữa 1400 (Lv6) và 2100 (Lv7): 500/700 = 71%
        assert_eq!(level_from_xp(1900).progress_pct, 71);
    }

    #[test]
    fn test_activity_labels() {
        let ev = ActivityEvent {
            reason: "post_game".into(),
            amount: 50,
            created_at: Utc::now(),
        };
        assert_eq!(ev.label(), "đã đăng một game mới");
        let unknown = ActivityEvent {
            reason: "future_thing".into(),
            amount: 1,
            created_at: Utc::now(),
        };
        assert_eq!(unknown.label(), "có hoạt động mới");
    }

    #[test]
    fn test_levels_table_monotonic() {
        // Ngưỡng phải tăng nghiêm ngặt + 12 cấp
        assert_eq!(LEVELS.len(), 12);
        for pair in LEVELS.windows(2) {
            assert!(pair[0].0 < pair[1].0, "ngưỡng phải tăng nghiêm ngặt");
        }
    }
}

/// Flat row: catalog huy hiệu + trạng thái earned/showcase của 1 user
/// (JOIN 1 query — tránh N+1). sqlx tuple cần Type/Decode nên phải struct.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AchievementStatusRow {
    // Catalog fields
    pub id: String,
    pub title: String,
    pub description: String,
    pub icon: String,
    pub xp_reward: i32,
    pub category: String,
    pub sort_order: i32,
    pub is_active: bool,
    // User state (NULL nếu chưa đạt)
    pub earned_at: Option<DateTime<Utc>>,
    pub is_showcased: Option<bool>,
}

impl AchievementStatusRow {
    /// Chuyển sang AchievementWithStatus cho template.
    #[must_use]
    pub fn into_status(self) -> AchievementWithStatus {
        AchievementWithStatus {
            achievement: Achievement {
                id: self.id,
                title: self.title,
                description: self.description,
                icon: self.icon,
                xp_reward: self.xp_reward,
                category: self.category,
                sort_order: self.sort_order,
                is_active: self.is_active,
            },
            earned_at: self.earned_at,
            is_showcased: self.is_showcased.unwrap_or(false),
        }
    }
}

/// Flat row: huy hiệu đã đạt (user_achievements JOIN catalog).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserAchievementRow {
    pub id: String,
    pub title: String,
    pub description: String,
    pub icon: String,
    pub xp_reward: i32,
    pub category: String,
    pub sort_order: i32,
    pub is_active: bool,
    pub earned_at: DateTime<Utc>,
    pub is_showcased: bool,
}

impl UserAchievementRow {
    /// Tách (catalog, earned_at, showcased).
    #[must_use]
    pub fn split(self) -> (Achievement, DateTime<Utc>, bool) {
        (
            Achievement {
                id: self.id,
                title: self.title,
                description: self.description,
                icon: self.icon,
                xp_reward: self.xp_reward,
                category: self.category,
                sort_order: self.sort_order,
                is_active: self.is_active,
            },
            self.earned_at,
            self.is_showcased,
        )
    }
}

/// Flat row: catalog huy hiệu + số người đạt (admin stats).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AchievementStatRow {
    pub id: String,
    pub title: String,
    pub description: String,
    pub icon: String,
    pub xp_reward: i32,
    pub category: String,
    pub sort_order: i32,
    pub is_active: bool,
    pub holders: i64,
}

impl AchievementStatRow {
    /// Tách (catalog, holders).
    #[must_use]
    pub fn split(self) -> (Achievement, i64) {
        (
            Achievement {
                id: self.id,
                title: self.title,
                description: self.description,
                icon: self.icon,
                xp_reward: self.xp_reward,
                category: self.category,
                sort_order: self.sort_order,
                is_active: self.is_active,
            },
            self.holders,
        )
    }
}
