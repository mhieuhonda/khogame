//! v2.9.0 → v3.1.0 — Gamification models: XP, cấp độ, điểm danh, huy hiệu.
//!
//! Kiến trúc:
//! - `user_xp_totals` cache tổng XP (BIGINT — i64, max ~9.2e18) — đọc O(1)
//!   cho chip cấp độ ở comment/chat/profile.
//! - `xp_events` append-only — kiêm activity feed trên hồ sơ.
//! - `daily_checkins` điểm danh + chuỗi ngày liên tiếp.
//! - `achievements` catalog (seed migration 021 + 024 = 125 huy hiệu) +
//!   `user_achievements` (kèm `is_showcased` để ghim tối đa 3 huy hiệu).
//!
//! Cấp độ là HÀM THUẦN của tổng XP (không lưu DB) — đổi ngưỡng chỉ cần
//! đổi bảng LEVELS / hàm level_from_xp, toàn site tự cập nhật.
//!
//! v3.1.0 — TĂNG MAX LEVEL LÊN 500 TỶ (500_000_000_000):
//! - 12 cấp đầu (0..=12000 XP) dùng LEVELS table (tên gọi tĩnh).
//! - Cấp 13+ dùng công thức `level = 12 + (xp - 12000) / 1000`.
//! - Tên danh hiệu cấp cao gán theo tier (13-99: Vô Song, 100-999:
//!   Thiên Hạ Đệ Nhất, 1000+: Bán Thần / Thần Chi Tướng / ... / Vô Biên).
//! - Đạt level 500 tỷ (xp ~5e14) → max level, next_level_xp = None.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Bảng ngưỡng XP → danh hiệu. Index = level - 1.
/// (ngưỡng XP TÍCH LŨY, danh hiệu tiếng Việt) — cho 12 cấp đầu.
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

/// v3.1.0 — Cấp độ tối đa của hệ thống: 500 TỶ.
/// Để đạt: user cần ~12000 + (500e9 - 12) * 1000 ≈ 5e14 XP.
/// (BIGINT max ~9.2e18, dư sức chứa.)
pub const MAX_LEVEL: i64 = 500_000_000_000;

/// Số huy hiệu tối đa được ghim (showcase) lên hồ sơ.
pub const MAX_SHOWCASED_ACHIEVEMENTS: i32 = 3;

/// v3.1.0 — Ngưỡng XP của cấp cuối trong LEVELS table.
/// Dùng cho level_from_xp phân nhánh table-lookup vs formula.
const BASE_TABLE_XP: i64 = 12_000;

/// v3.1.0 — Khoảng XP mỗi cấp (sau khi rời bảng 12 cấp đầu).
/// Công thức: `level = 12 + (xp - 12000) / 1000`.
const XP_PER_LEVEL_TIER2: i64 = 1_000;

/// Thông tin cấp độ tính từ tổng XP.
#[derive(Debug, Clone, Serialize)]
pub struct LevelInfo {
    /// Cấp độ hiện tại (1-based). i64 vì max = 500 tỷ.
    pub level: i64,
    /// Danh hiệu của cấp hiện tại (lookup table hoặc formula tier).
    pub title: &'static str,
    /// Tổng XP hiện tại (BIGINT).
    pub xp: i64,
    /// XP cần để đạt cấp kế (None nếu đã max).
    pub next_level_xp: Option<i64>,
    /// Tiến độ tới cấp kế, 0-100 (100 nếu max).
    pub progress_pct: i32,
}

/// v3.1.0 — Tên danh hiệu cho level (chỉ gọi khi level > 12 — ngoài
/// bảng LEVELS). 12 cấp đầu dùng LEVELS table trực tiếp.
#[must_use]
pub fn title_for_level(level: i64) -> &'static str {
    if level <= 0 {
        return LEVELS[0].1;
    }
    if level <= i64::try_from(LEVELS.len()).unwrap_or(12) {
        let idx = (level as usize) - 1;
        return LEVELS[idx].1;
    }
    // Tier 2 — level 13+
    match level {
        13..=99 => "Vô Song",
        100..=999 => "Thiên Hạ Đệ Nhất",
        1_000..=9_999 => "Vô Địch Thiên Hạ",
        10_000..=99_999 => "Bán Thần",
        100_000..=999_999 => "Thần Chi Tướng",
        1_000_000..=9_999_999 => "Thần Vương",
        10_000_000..=99_999_999 => "Thánh Nhân",
        100_000_000..=999_999_999 => "Tiên Nhân",
        1_000_000_000..=9_999_999_999 => "Đế Tôn",
        10_000_000_000..=99_999_999_999 => "Chí Tôn",
        100_000_000_000..=MAX_LEVEL => "Vô Cực",
        _ => "Vô Biên", // > MAX_LEVEL — về lý thuyết không xảy ra do cap.
    }
}

/// Tính cấp độ + danh hiệu + tiến độ từ tổng XP (hàm thuần, test được).
///
/// Quy tắc (v3.1.0):
/// - xp <= 12000 → dùng LEVELS table (12 cấp đầu).
/// - xp > 12000 → level = 12 + (xp - 12000) / 1000.
/// - Cap tại MAX_LEVEL (500 tỷ); cap → next_level_xp = None, pct = 100.
#[must_use]
pub fn level_from_xp(xp: i64) -> LevelInfo {
    let xp = xp.max(0);
    // Tier 1 — table lookup
    if xp < BASE_TABLE_XP {
        // Tìm level cao nhất có ngưỡng <= xp
        let mut idx = 0usize;
        for (i, (threshold, _)) in LEVELS.iter().enumerate() {
            if xp >= i64::from(*threshold) {
                idx = i;
            }
        }
        let (cur_threshold, title) = LEVELS[idx];
        let cur_threshold_i64 = i64::from(cur_threshold);
        let next = LEVELS.get(idx + 1);
        let (next_level_xp, progress_pct) = match next {
            Some((next_threshold, _)) => {
                let span = i64::from(*next_threshold - cur_threshold).max(1);
                let done = xp - cur_threshold_i64;
                (
                    Some(i64::from(*next_threshold)),
                    ((done * 100) / span).clamp(0, 100) as i32,
                )
            }
            None => {
                // xp < 12000 nhưng không có next — không xảy ra vì LEVELS[11] = 12000
                (None, 100)
            }
        };
        return LevelInfo {
            level: idx as i64 + 1,
            title,
            xp,
            next_level_xp,
            progress_pct,
        };
    }
    // Tier 2 — formula: level = 12 + (xp - 12000) / 1000
    let base_level = i64::try_from(LEVELS.len()).unwrap_or(12); // 12
    let extra_levels = (xp - BASE_TABLE_XP) / XP_PER_LEVEL_TIER2;
    let level = (base_level + extra_levels).min(MAX_LEVEL);
    let title = title_for_level(level);
    if level >= MAX_LEVEL {
        return LevelInfo {
            level,
            title,
            xp,
            next_level_xp: None,
            progress_pct: 100,
        };
    }
    // Tiến độ trong chunk 1000 XP hiện tại
    let chunk_done = (xp - BASE_TABLE_XP) - extra_levels * XP_PER_LEVEL_TIER2;
    let next_xp = BASE_TABLE_XP + (extra_levels + 1) * XP_PER_LEVEL_TIER2;
    let pct = ((chunk_done * 100) / XP_PER_LEVEL_TIER2).clamp(0, 100) as i32;
    LevelInfo {
        level,
        title,
        xp,
        next_level_xp: Some(next_xp),
        progress_pct: pct,
    }
}

/// Cache tổng XP của 1 user (bảng user_xp_totals).
/// v3.1.0 — total_xp: BIGINT (i64) để hỗ trợ level tới 500 tỷ.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserXpTotal {
    pub user_id: Uuid,
    pub total_xp: i64,
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
/// v3.1.0 — total_xp: BIGINT (i64).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LeaderboardEntry {
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub total_xp: i64,
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
            // v3.1.0 — arcade reasons
            "spin" => "đã quay vòng may mắn",
            "trivia" => "đã trả lời câu đố đúng",
            "trivia_bonus" => "đã hoàn thành cả 3 câu đố",
            "rps_win" => "đã thắng ván Oẳn tù tì",
            "word_chain" => "đã nối từ hợp lệ",
            "shop_spend" => "đã mua vật phẩm cửa hàng",
            "mystery_box" => "đã mở mystery box",
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

        // v3.1.0 — XP 12000 → level 12 (Bất Tử), tier 2 bắt đầu từ level 13.
        // Tier 2 cho level 12: xp=12000..12999 là phạm vi 1000 XP của level 12,
        // next=13000 (level 13 = Vô Song).
        let l = level_from_xp(12_000);
        assert_eq!((l.level, l.title), (12, "Bất Tử"));
        assert_eq!(l.next_level_xp, Some(13_000));
        assert_eq!(l.progress_pct, 0);

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
    fn test_levels_table_monotonic() {
        // Ngưỡng phải tăng nghiêm ngặt + 12 cấp
        assert_eq!(LEVELS.len(), 12);
        for pair in LEVELS.windows(2) {
            assert!(pair[0].0 < pair[1].0, "ngưỡng phải tăng nghiêm ngặt");
        }
    }

    // === v3.1.0 — NEW TESTS: Tier 2 formula (level 13+) ===

    #[test]
    fn test_tier2_boundary_at_12000_xp() {
        // 12000 XP — level 12 (Bất Tử), vào tier 2 nhưng extra=0 nên level=12,
        // chunk 0..1000 với next=13000 (đi lên level 13).
        let l = level_from_xp(12_000);
        assert_eq!(l.level, 12);
        assert_eq!(l.title, "Bất Tử");
        assert_eq!(l.next_level_xp, Some(13_000));
        assert_eq!(l.progress_pct, 0);
        // 12001 XP: vẫn level 12, pct=0 (1/1000 = 0 khi ép i32)
        let l = level_from_xp(12_001);
        assert_eq!(l.level, 12);
        assert_eq!(l.progress_pct, 0);
    }

    #[test]
    fn test_tier2_progress_within_chunk() {
        // 12_500 XP: level 12 (Bất Tử), 500/1000 = 50% đến level 13
        let l = level_from_xp(12_500);
        assert_eq!(l.level, 12);
        assert_eq!(l.progress_pct, 50);
        assert_eq!(l.next_level_xp, Some(13_000));
        // 12_999 XP: 999/1000 = 99% — vẫn level 12
        let l = level_from_xp(12_999);
        assert_eq!(l.level, 12);
        assert_eq!(l.progress_pct, 99);
        // 13_000 XP: level 13 (Vô Song)
        let l = level_from_xp(13_000);
        assert_eq!(l.level, 13);
        assert_eq!(l.title, "Vô Song");
        assert_eq!(l.progress_pct, 0);
    }

    #[test]
    fn test_tier2_high_levels() {
        // 12_000 + 87 * 1000 = 99_000 XP → level 12 + 87 = 99 (Vô Song)
        let l = level_from_xp(12_000 + 87 * 1000);
        assert_eq!(l.level, 99);
        assert_eq!(l.title, "Vô Song");
        // 12_000 + 88 * 1000 = 100_000 XP → level 100 (Thiên Hạ Đệ Nhất)
        let l = level_from_xp(12_000 + 88 * 1000);
        assert_eq!(l.level, 100);
        assert_eq!(l.title, "Thiên Hạ Đệ Nhất");
        // 12_000 + 987 * 1000 = 999_000 XP → level 999
        let l = level_from_xp(12_000 + 987 * 1000);
        assert_eq!(l.level, 999);
        assert_eq!(l.title, "Thiên Hạ Đệ Nhất");
        // 12_000 + 988 * 1000 = 1_000_000 XP → level 1000 (Vô Địch Thiên Hạ)
        let l = level_from_xp(12_000 + 988 * 1000);
        assert_eq!(l.level, 1_000);
        assert_eq!(l.title, "Vô Địch Thiên Hạ");
    }

    #[test]
    fn test_tier2_max_level_cap() {
        // XP đủ để vượt MAX_LEVEL — phải cap tại 500 tỷ
        // level 500 tỷ = 12 + (xp - 12000)/1000 = 500_000_000_000
        // → xp cần = 12000 + (500e9 - 12) * 1000 ≈ 4.99999999988e14
        let xp_for_max = BASE_TABLE_XP + (MAX_LEVEL - 12) * XP_PER_LEVEL_TIER2;
        let l = level_from_xp(xp_for_max);
        assert_eq!(l.level, MAX_LEVEL);
        assert_eq!(l.title, "Vô Cực");
        assert_eq!(l.next_level_xp, None);
        assert_eq!(l.progress_pct, 100);
        // XP VƯỢT ngưỡng max → vẫn cap
        let l = level_from_xp(xp_for_max + 1_000_000_000);
        assert_eq!(l.level, MAX_LEVEL);
        assert_eq!(l.next_level_xp, None);
    }

    #[test]
    fn test_tier2_max_level_minus_one() {
        // 1 level trước max → vẫn có next_level_xp
        let xp_one_below = BASE_TABLE_XP + (MAX_LEVEL - 12 - 1) * XP_PER_LEVEL_TIER2;
        let l = level_from_xp(xp_one_below);
        assert_eq!(l.level, MAX_LEVEL - 1);
        assert!(l.next_level_xp.is_some());
        assert_eq!(l.title, "Vô Cực");
    }

    #[test]
    fn test_title_for_level_tier_distribution() {
        assert_eq!(title_for_level(1), "Tân Binh");
        assert_eq!(title_for_level(12), "Bất Tử");
        assert_eq!(title_for_level(13), "Vô Song");
        assert_eq!(title_for_level(99), "Vô Song");
        assert_eq!(title_for_level(100), "Thiên Hạ Đệ Nhất");
        assert_eq!(title_for_level(999), "Thiên Hạ Đệ Nhất");
        assert_eq!(title_for_level(1_000), "Vô Địch Thiên Hạ");
        assert_eq!(title_for_level(10_000), "Bán Thần");
        assert_eq!(title_for_level(100_000), "Thần Chi Tướng");
        assert_eq!(title_for_level(1_000_000), "Thần Vương");
        assert_eq!(title_for_level(10_000_000), "Thánh Nhân");
        assert_eq!(title_for_level(100_000_000), "Tiên Nhân");
        assert_eq!(title_for_level(1_000_000_000), "Đế Tôn");
        assert_eq!(title_for_level(10_000_000_000), "Chí Tôn");
        assert_eq!(title_for_level(100_000_000_000), "Vô Cực");
        assert_eq!(title_for_level(MAX_LEVEL), "Vô Cực");
        assert_eq!(title_for_level(0), "Tân Binh");
    }

    #[test]
    fn test_activity_labels_v310() {
        // v3.1.0 — new reasons cho arcade games
        let ev = ActivityEvent {
            reason: "rps_win".into(),
            amount: 2,
            created_at: Utc::now(),
        };
        assert_eq!(ev.label(), "đã thắng ván Oẳn tù tì");
        let ev = ActivityEvent {
            reason: "word_chain".into(),
            amount: 3,
            created_at: Utc::now(),
        };
        assert_eq!(ev.label(), "đã nối từ hợp lệ");
        let ev = ActivityEvent {
            reason: "shop_spend".into(),
            amount: -50,
            created_at: Utc::now(),
        };
        assert_eq!(ev.label(), "đã mua vật phẩm cửa hàng");
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
