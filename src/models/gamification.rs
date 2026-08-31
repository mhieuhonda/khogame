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
/// (ngưỡng XP TÍCH LŨY, danh hiệu tiếng Việt).
///
/// v3.2.0 — MỞ RỘNG từ 12 → 24 cấp: xen 11 bậc mới vào giữa các ngưỡng cũ
/// (giữ NGUYÊN 12 ngưỡng cũ — người chơi cũ không mất danh hiệu "đỉnh",
/// chỉ được thêm tên gọi mới trên đường leo). Cấp 24 (12.000 XP) = "Bất Tử"
/// vẫn là cánh cửa vào tier-2 công thức.
pub const LEVELS: &[(i32, &str)] = &[
    (0, "Tân Binh"),
    (50, "Khởi Đầu"),
    (100, "Tập Sự"),
    (175, "Học Việc"),
    (250, "Thám Hiểm"),
    (375, "Kiếm Khách"),
    (500, "Chiến Binh"),
    (700, "Du Hiệp"),
    (900, "Cao Thủ"),
    (1150, "Anh Hùng"),
    (1400, "Đấu Sĩ"),
    (1750, "Trảm Tướng"),
    (2100, "Kỳ Lão"),
    (2550, "Tông Sư"),
    (3000, "Bậc Thầy"),
    (3600, "Phong Vân"),
    (4200, "Đại Sư"),
    (5100, "Tinh Anh"),
    (6000, "Huyền Thoại"),
    (7200, "Bất Diệt"),
    (8500, "Vinh Quang"),
    (10200, "Thần Tượng"),
    (11100, "Siêu Phàm"),
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

/// v3.2.0 — Tên danh hiệu cho level (chỉ gọi khi level > 24 — ngoài
/// bảng LEVELS). 24 cấp đầu dùng LEVELS table trực tiếp.
///
/// Bậc thang 20 danh hiệu tier-2 được CANH CHỈNH khớp với tên gọi của
/// các huy hiệu `level_N` (migration 021/024) để profile hiển thị đồng
/// nhất: đạt cấp 100 = huy hiệu "Bán Thần" = danh hiệu "Bán Thần".
#[must_use]
pub fn title_for_level(level: i64) -> &'static str {
    if level <= 0 {
        return LEVELS[0].1;
    }
    if level <= i64::try_from(LEVELS.len()).unwrap_or(24) {
        let idx = (level as usize) - 1;
        return LEVELS[idx].1;
    }
    // Tier 2 — level 25+
    match level {
        25..=34 => "Vô Song",
        35..=49 => "Bát Phương Uy Danh",
        50..=74 => "Thiên Hạ Đệ Nhất",
        75..=99 => "Vô Địch",
        100..=149 => "Bán Thần",
        150..=199 => "Thần Chi Tướng",
        200..=299 => "Thần Vương",
        300..=499 => "Thánh Nhân",
        500..=749 => "Tiên Nhân",
        750..=999 => "Đế Tôn",
        1_000..=1_999 => "Chí Tôn",
        2_000..=4_999 => "Vô Cực",
        5_000..=9_999 => "Vô Hạn",
        10_000..=99_999 => "Vô Ảnh",
        100_000..=999_999 => "Vô Hình",
        1_000_000..=9_999_999 => "Thái Cực",
        10_000_000..=99_999_999 => "Hỗn Nguyên",
        100_000_000..=999_999_999 => "Vô Lượng",
        1_000_000_000..=9_999_999_999 => "Đại La",
        10_000_000_000..=99_999_999_999 => "Tạo Hóa",
        100_000_000_000..=MAX_LEVEL => "Vô Thượng",
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
            "shop_spend" => "đã mua vật phẩm cửa hàng",
            "mystery_box" => "đã mở mystery box",
            // v3.6.0 — Admin XP Boost
            "admin_boost" => "nhận XP boost từ quản trị",
            _ => "có hoạt động mới",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_boundaries() {
        // Level 1 tại 0 XP — v3.2.0: next ngưỡng = 50 (Khởi Đầu)
        let l = level_from_xp(0);
        assert_eq!((l.level, l.title), (1, "Tân Binh"));
        assert_eq!(l.next_level_xp, Some(50));
        assert_eq!(l.progress_pct, 0);

        // 49 XP vẫn level 1
        let l = level_from_xp(49);
        assert_eq!(l.level, 1);
        assert_eq!(l.progress_pct, 98);

        // 50 XP → level 2 (Khởi Đầu); 100 XP → level 3 (Tập Sự)
        assert_eq!(level_from_xp(50).level, 2);
        let l = level_from_xp(100);
        assert_eq!((l.level, l.title), (3, "Tập Sự"));

        // 249 → vẫn 4 (Học Việc); 250 → 5 (Thám Hiểm)
        assert_eq!(level_from_xp(249).level, 4);
        assert_eq!(level_from_xp(250).level, 5);

        // Ngưỡng giữa các cấp (giữ nguyên ngưỡng cũ 900 = Cao Thủ)
        assert_eq!(level_from_xp(899).level, 8);
        assert_eq!(level_from_xp(900).level, 9);

        // v3.2.0 — XP 12000 → level 24 (Bất Tử), tier 2 bắt đầu từ level 25.
        // Tier 2 cho level 24: xp=12000..12999 là phạm vi 1000 XP của level 24,
        // next=13000 (level 25 = Vô Song).
        let l = level_from_xp(12_000);
        assert_eq!((l.level, l.title), (24, "Bất Tử"));
        assert_eq!(l.next_level_xp, Some(13_000));
        assert_eq!(l.progress_pct, 0);

        // XP âm (không thể xảy ra trong DB do CHECK, phòng hộ) → coi như 0
        assert_eq!(level_from_xp(-5).level, 1);
    }

    #[test]
    fn test_level_progress_pct() {
        // Giữa level 1 (0) và level 2 (50): 25 XP → 50%
        assert_eq!(level_from_xp(25).progress_pct, 50);
        // Giữa level 10 (1150) và 11 (1400): 1275 XP → 50%
        assert_eq!(level_from_xp(1275).progress_pct, 50);
        // Giữa level 12 (1750) và 13 (2100): 1925 XP → 50%
        let l = level_from_xp(1925);
        assert_eq!(l.level, 12);
        assert_eq!(l.progress_pct, 50);
        // 1900 nằm giữa 1750 (Lv12) và 2100 (Lv13): 150/350 = 42%
        assert_eq!(level_from_xp(1900).progress_pct, 42);
    }

    #[test]
    fn test_levels_table_monotonic() {
        // Ngưỡng phải tăng nghiêm ngặt + 24 cấp (v3.2.0 — mở rộng từ 12)
        assert_eq!(LEVELS.len(), 24);
        for pair in LEVELS.windows(2) {
            assert!(pair[0].0 < pair[1].0, "ngưỡng phải tăng nghiêm ngặt");
        }
        // 12 ngưỡng gốc của v3.1.0 phải vẫn tồn tại trong bảng mới
        for (xp, _) in [
            (0, ""),
            (100, ""),
            (250, ""),
            (500, ""),
            (900, ""),
            (1400, ""),
            (2100, ""),
            (3000, ""),
            (4200, ""),
            (6000, ""),
            (8500, ""),
            (12000, ""),
        ] {
            assert!(
                LEVELS.iter().any(|(t, _)| *t == xp),
                "ngưỡng gốc {xp} phải được giữ nguyên"
            );
        }
    }

    // === v3.2.0 — Tier 2 formula (level 25+, base = LEVELS.len() = 24) ===

    #[test]
    fn test_tier2_boundary_at_12000_xp() {
        // 12000 XP — level 24 (Bất Tử), vào tier 2 nhưng extra=0 nên level=24,
        // chunk 0..1000 với next=13000 (đi lên level 25).
        let l = level_from_xp(12_000);
        assert_eq!(l.level, 24);
        assert_eq!(l.title, "Bất Tử");
        assert_eq!(l.next_level_xp, Some(13_000));
        assert_eq!(l.progress_pct, 0);
        // 12001 XP: vẫn level 24, pct=0 (1/1000 = 0 khi ép i32)
        let l = level_from_xp(12_001);
        assert_eq!(l.level, 24);
        assert_eq!(l.progress_pct, 0);
    }

    #[test]
    fn test_tier2_progress_within_chunk() {
        // 12_500 XP: level 24 (Bất Tử), 500/1000 = 50% đến level 25
        let l = level_from_xp(12_500);
        assert_eq!(l.level, 24);
        assert_eq!(l.progress_pct, 50);
        assert_eq!(l.next_level_xp, Some(13_000));
        // 12_999 XP: 999/1000 = 99% — vẫn level 24
        let l = level_from_xp(12_999);
        assert_eq!(l.level, 24);
        assert_eq!(l.progress_pct, 99);
        // 13_000 XP: level 25 (Vô Song)
        let l = level_from_xp(13_000);
        assert_eq!(l.level, 25);
        assert_eq!(l.title, "Vô Song");
        assert_eq!(l.progress_pct, 0);
    }

    #[test]
    fn test_tier2_high_levels() {
        // level = 24 + (xp - 12_000)/1000
        // 87_000 XP → level 24 + 75 = 99 (Vô Địch — bậc 75..=99)
        let l = level_from_xp(12_000 + 75 * 1000);
        assert_eq!(l.level, 99);
        assert_eq!(l.title, "Vô Địch");
        // 88_000 XP → level 100 (Bán Thần — canh khớp huy hiệu level_100)
        let l = level_from_xp(12_000 + 76 * 1000);
        assert_eq!(l.level, 100);
        assert_eq!(l.title, "Bán Thần");
        // 987_000 XP → level 999 (Đế Tôn — bậc 750..=999)
        let l = level_from_xp(12_000 + 975 * 1000);
        assert_eq!(l.level, 999);
        assert_eq!(l.title, "Đế Tôn");
        // 988_000 XP → level 1000 (Chí Tôn — canh khớp huy hiệu level_1000)
        let l = level_from_xp(12_000 + 976 * 1000);
        assert_eq!(l.level, 1_000);
        assert_eq!(l.title, "Chí Tôn");
    }

    #[test]
    fn test_tier2_max_level_cap() {
        // XP đủ để vượt MAX_LEVEL — phải cap tại 500 tỷ
        // level 500 tỷ = 24 + (xp - 12000)/1000 = 500_000_000_000
        // → xp cần = 12000 + (500e9 - 24) * 1000 ≈ 4.99999999988e14
        let xp_for_max = BASE_TABLE_XP + (MAX_LEVEL - 24) * XP_PER_LEVEL_TIER2;
        let l = level_from_xp(xp_for_max);
        assert_eq!(l.level, MAX_LEVEL);
        assert_eq!(l.title, "Vô Thượng");
        assert_eq!(l.next_level_xp, None);
        assert_eq!(l.progress_pct, 100);
        // XP VƯỢT ngưỡng max → vẫn cap
        let l = level_from_xp(xp_for_max + 1_000_000_000);
        assert_eq!(l.level, MAX_LEVEL);
        assert_eq!(l.next_level_xp, None);
    }

    #[test]
    fn test_tier2_max_level_minus_one() {
        // 1 level trước max → vẫn có next_level_xp (vẫn bậc Vô Thượng)
        let xp_one_below = BASE_TABLE_XP + (MAX_LEVEL - 24 - 1) * XP_PER_LEVEL_TIER2;
        let l = level_from_xp(xp_one_below);
        assert_eq!(l.level, MAX_LEVEL - 1);
        assert!(l.next_level_xp.is_some());
        assert_eq!(l.title, "Vô Thượng");
    }

    #[test]
    fn test_title_for_level_tier_distribution() {
        // Tier 1 — bảng 24 cấp (v3.2.0)
        assert_eq!(title_for_level(1), "Tân Binh");
        assert_eq!(title_for_level(2), "Khởi Đầu");
        assert_eq!(title_for_level(5), "Thám Hiểm");
        assert_eq!(title_for_level(7), "Chiến Binh");
        assert_eq!(title_for_level(19), "Huyền Thoại");
        assert_eq!(title_for_level(24), "Bất Tử");
        // Tier 2 — bậc thang canh khớp huy hiệu level_N
        assert_eq!(title_for_level(25), "Vô Song");
        assert_eq!(title_for_level(34), "Vô Song");
        assert_eq!(title_for_level(35), "Bát Phương Uy Danh");
        assert_eq!(title_for_level(49), "Bát Phương Uy Danh");
        assert_eq!(title_for_level(50), "Thiên Hạ Đệ Nhất");
        assert_eq!(title_for_level(74), "Thiên Hạ Đệ Nhất");
        assert_eq!(title_for_level(75), "Vô Địch");
        assert_eq!(title_for_level(99), "Vô Địch");
        assert_eq!(title_for_level(100), "Bán Thần");
        assert_eq!(title_for_level(149), "Bán Thần");
        assert_eq!(title_for_level(150), "Thần Chi Tướng");
        assert_eq!(title_for_level(199), "Thần Chi Tướng");
        assert_eq!(title_for_level(200), "Thần Vương");
        assert_eq!(title_for_level(299), "Thần Vương");
        assert_eq!(title_for_level(300), "Thánh Nhân");
        assert_eq!(title_for_level(499), "Thánh Nhân");
        assert_eq!(title_for_level(500), "Tiên Nhân");
        assert_eq!(title_for_level(749), "Tiên Nhân");
        assert_eq!(title_for_level(750), "Đế Tôn");
        assert_eq!(title_for_level(999), "Đế Tôn");
        assert_eq!(title_for_level(1_000), "Chí Tôn");
        assert_eq!(title_for_level(1_999), "Chí Tôn");
        assert_eq!(title_for_level(2_000), "Vô Cực");
        assert_eq!(title_for_level(4_999), "Vô Cực");
        assert_eq!(title_for_level(5_000), "Vô Hạn");
        assert_eq!(title_for_level(9_999), "Vô Hạn");
        assert_eq!(title_for_level(10_000), "Vô Ảnh");
        assert_eq!(title_for_level(99_999), "Vô Ảnh");
        assert_eq!(title_for_level(100_000), "Vô Hình");
        assert_eq!(title_for_level(999_999), "Vô Hình");
        assert_eq!(title_for_level(1_000_000), "Thái Cực");
        assert_eq!(title_for_level(9_999_999), "Thái Cực");
        assert_eq!(title_for_level(10_000_000), "Hỗn Nguyên");
        assert_eq!(title_for_level(100_000_000), "Vô Lượng");
        assert_eq!(title_for_level(1_000_000_000), "Đại La");
        assert_eq!(title_for_level(10_000_000_000), "Tạo Hóa");
        assert_eq!(title_for_level(100_000_000_000), "Vô Thượng");
        assert_eq!(title_for_level(MAX_LEVEL), "Vô Thượng");
        assert_eq!(title_for_level(0), "Tân Binh");
    }

    #[test]
    fn test_activity_labels_v310() {
        // v3.1.0 — new reasons cho arcade games
        // (v3.8.0 — rps_win / word_chain đã xóa cùng 2 game mode)
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
