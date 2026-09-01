//! v3.0.0 — Repository cửa hàng XP: mua vật phẩm, tồn kho, boost.
//!
//! Nguyên tắc an toàn XP: TRỪ XP bằng UPDATE có điều kiện
//! `WHERE total_xp >= price RETURNING` trong cùng transaction với ghi
//! purchase/inventory/boost → không bao giờ âm XP, không race double-buy.

use crate::error::{AppError, AppResult};
use crate::models::retention::{PurchaseOutcome, ShopItem, ShopItemWithStock};
use sqlx::PgPool;
use uuid::Uuid;

/// Giá trị mystery box: min/max XP ngẫu nhiên.
pub const MYSTERY_MIN_XP: i32 = 10;
pub const MYSTERY_MAX_XP: i32 = 150;

/// v3.5.1 FIX (audit 5-e, HIGH — máy in XP): giá mystery box tại seed
/// migration 032. EV payout = (10+150)/2 ≈ 79.5 XP — giá PHẢI > EV thì
/// mở vô hạn mới không sinh XP ròng (45 cũ → +34.5 XP/hộp = in XP vô hạn).
/// Hằng này chỉ để guard compile-time + tài liệu — DB (`shop_items.price`)
/// mới là nguồn sự thật khi mua.
pub const MYSTERY_BOX_PRICE_XP: i32 = 100;

/// v3.5.1 — cap số hộp mở/ngày/user (lớp phòng vệ 2: kể cả nếu admin chỉnh
/// giá DB về thấp lại, farm cũng bị chặn ở 5 hộp/ngày = tối đa +375 XP/ngày
/// nếu trúng jackpots liên tiếp — vô hại với economy).
pub const MYSTERY_BOX_DAILY_CAP: i64 = 5;

/// Giới hạn tồn kho streak_freeze (chặn mua ôm hàng).
pub const MAX_STREAK_FREEZE: i32 = 5;

/// v3.7.0 — duration_hours mặc định khi DB trả giá trị phi lý (<= 0):
/// xp_boost 24h, name_glow / avatar_frame 30 ngày (720h). Layer phòng vệ
/// khi admin lỡ tay chỉnh cột duration_hours về 0/âm — vật phẩm thời gian
/// không bao giờ hết hạn ngay lập tức (trả tiền mà 0h là lừa đảo user).
pub const DEFAULT_XP_BOOST_HOURS: i32 = 24;
pub const DEFAULT_LONG_ITEM_HOURS: i32 = 720;

/// Chuẩn hoá duration_hours từ DB — floor theo kind khi giá trị phi lý.
fn effective_duration_hours(kind: &str, raw: i32) -> i32 {
    let default = if kind == "xp_boost" {
        DEFAULT_XP_BOOST_HOURS
    } else {
        DEFAULT_LONG_ITEM_HOURS
    };
    if raw <= 0 {
        default
    } else {
        raw
    }
}

pub struct ShopRepo;

impl ShopRepo {
    /// Danh sách vật phẩm active + tồn kho của user.
    /// # Errors
    /// Trả lỗi khi DB fail.
    ///
    /// v3.12.0 FIX (audit logic pass 1 — M2, N+1): trước đây chạy 1 query
    /// tồn kho RIÊNG cho từng vật phẩm (shop ~12 items → 12 round-trip
    /// tuần tự mỗi lượt mở /shop). Giờ đúng 2 query cố định: 1 load items,
    /// 1 load toàn bộ tồn kho của user rồi map trong Rust bằng HashMap.
    pub async fn list_for_user(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<ShopItemWithStock>> {
        let items = sqlx::query_as::<_, ShopItem>(
            "SELECT id, name, description, icon, price, kind, is_active, duration_hours
             FROM shop_items WHERE is_active = TRUE ORDER BY price ASC",
        )
        .fetch_all(pool)
        .await?;
        let owned_map: std::collections::HashMap<String, i32> = sqlx::query_as::<_, (String, i32)>(
            "SELECT item_id, quantity FROM user_inventory WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect();
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            let owned = owned_map.get(&item.id).copied().unwrap_or(0);
            out.push(ShopItemWithStock { item, owned });
        }
        Ok(out)
    }

    /// XP boost đang active? (dùng bởi GamificationRepo khi cộng XP)
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn xp_boost_active(pool: &PgPool, user_id: Uuid) -> AppResult<bool> {
        let row: Option<chrono::DateTime<chrono::Utc>> =
            sqlx::query_scalar("SELECT xp_boost_until FROM user_boosts WHERE user_id = $1")
                .bind(user_id)
                .fetch_optional(pool)
                .await?;
        Ok(row.map(|t| t > chrono::Utc::now()).unwrap_or(false))
    }

    /// Mua 1 vật phẩm. Trừ XP atomic + ghi đúng loại:
    /// - streak_freeze: +1 inventory (tối đa MAX_STREAK_FREEZE)
    /// - xp_boost: gia hạn 24h (GREATEST(now, hiện tại) + 24h)
    /// - name_glow: gia hạn 30 ngày
    /// - mystery_box: mở ngay → XP ngẫu nhiên min..max
    /// # Errors
    /// Trả lỗi khi thiếu XP / không tồn tại / DB fail.
    pub async fn buy(
        pool: &PgPool,
        user_id: Uuid,
        item_id: &str,
        rand_val: i32,
    ) -> AppResult<PurchaseOutcome> {
        let item = sqlx::query_as::<_, ShopItem>(
            "SELECT id, name, description, icon, price, kind, is_active, duration_hours
             FROM shop_items WHERE id = $1 AND is_active = TRUE",
        )
        .bind(item_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Vật phẩm không tồn tại".into()))?;

        let mut tx = pool.begin().await?;
        // 1) Trừ XP có điều kiện — atomic, thiếu tiền → BadRequest
        // v3.1.0 — total_xp BIGINT (i64) để hỗ trợ level 500 tỷ.
        // v3.12.0 (audit logic L1): thêm guard `$2 > 0` — cột price không có
        // CHECK trong DB, admin lỡ đặt giá 0/âm (data drift/thao tác tay) sẽ
        // biến UPDATE thành MÁY IN XP (total_xp - (-N) = cộng XP, và
        // xp_events.amount = -price ghi dương như thưởng). Guard ở câu SQL
        // chặn tận gốc, không phụ thuộc validation phía handler.
        let total: Option<i64> = sqlx::query_scalar(
            r#"UPDATE user_xp_totals
               SET total_xp = total_xp - $2, updated_at = NOW()
               WHERE user_id = $1 AND total_xp >= $2 AND $2 > 0
               RETURNING total_xp"#,
        )
        .bind(user_id)
        .bind(item.price)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(total_xp) = total else {
            return Err(AppError::BadRequest(format!(
                "Không đủ XP — cần {} XP, bạn có ít hơn vậy",
                item.price
            )));
        };
        // 2) Ghi log chi tiêu (activity feed + audit nhẹ)
        sqlx::query(
            "INSERT INTO xp_events (user_id, reason, amount) VALUES ($1, 'shop_spend', $2)",
        )
        .bind(user_id)
        .bind(-item.price)
        .execute(&mut *tx)
        .await?;

        let mut mystery_xp = 0;
        let mut frame_id: Option<String> = None;
        match item.kind.as_str() {
            "streak_freeze" => {
                // Giới hạn tồn kho — đếm TRONG tx + khoá row (FOR UPDATE)
                // + upsert có guard `quantity < MAX` (audit vòng 8: 2 request
                // đồng thời cùng đọc qty=4 → cùng +1 → vượt cap 5).
                let qty: Option<i32> = sqlx::query_scalar(
                    "SELECT quantity FROM user_inventory
                     WHERE user_id = $1 AND item_id = 'streak_freeze' FOR UPDATE",
                )
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await?;
                if qty.unwrap_or(0) >= MAX_STREAK_FREEZE {
                    return Err(AppError::BadRequest(format!(
                        "Tối đa giữ {MAX_STREAK_FREEZE} Streak Freeze — dùng bớt rồi mua tiếp"
                    )));
                }
                let upserted = sqlx::query(
                    r#"INSERT INTO user_inventory (user_id, item_id, quantity)
                       VALUES ($1, $2, 1)
                       ON CONFLICT (user_id, item_id)
                       DO UPDATE SET quantity = user_inventory.quantity + 1,
                                     updated_at = NOW()
                       WHERE user_inventory.quantity < $3"#,
                )
                .bind(user_id)
                .bind(&item.id)
                .bind(MAX_STREAK_FREEZE)
                .execute(&mut *tx)
                .await?;
                if upserted.rows_affected() == 0 {
                    // Guard chặn đúng khoảnh khắc race → hoàn tác giao dịch
                    // (XP đã trừ ở trên được rollback cùng tx).
                    return Err(AppError::BadRequest(format!(
                        "Tối đa giữ {MAX_STREAK_FREEZE} Streak Freeze — dùng bớt rồi mua tiếp"
                    )));
                }
            }
            "xp_boost" => {
                // v3.7.0 — duration từ DB (migration 036) thay vì hardcode
                // 24h — admin chỉnh duration_hours là đổi được hiệu lực.
                let hours = effective_duration_hours("xp_boost", item.duration_hours);
                sqlx::query(
                    r#"INSERT INTO user_boosts (user_id, xp_boost_until, updated_at)
                       VALUES ($1, NOW() + make_interval(hours => $2), NOW())
                       ON CONFLICT (user_id) DO UPDATE SET
                         xp_boost_until = GREATEST(
                           COALESCE(user_boosts.xp_boost_until, NOW()), NOW())
                                       + make_interval(hours => $2),
                         updated_at = NOW()"#,
                )
                .bind(user_id)
                .bind(hours)
                .execute(&mut *tx)
                .await?;
            }
            "name_glow" => {
                // v3.7.0 — duration từ DB (720h = 30 ngày, 168h = 7 ngày...).
                let hours = effective_duration_hours("name_glow", item.duration_hours);
                sqlx::query(
                    r#"INSERT INTO user_boosts (user_id, name_glow_until, updated_at)
                       VALUES ($1, NOW() + make_interval(hours => $2), NOW())
                       ON CONFLICT (user_id) DO UPDATE SET
                         name_glow_until = GREATEST(
                           COALESCE(user_boosts.name_glow_until, NOW()), NOW())
                                       + make_interval(hours => $2),
                         updated_at = NOW()"#,
                )
                .bind(user_id)
                .bind(hours)
                .execute(&mut *tx)
                .await?;
            }
            "avatar_frame" => {
                // v3.7.0 — KHUNG AVATAR: kích hoạt khung mới (thay thế khung
                // cũ) + gia hạn theo duration_hours. Mua lại cùng khung →
                // GREATEST(now, hiện_tại) + duration (cùng pattern name_glow,
                // không mất phần còn lại). Không dùng user_inventory — state
                // kích hoạt nằm ở user_boosts, 1 khung active/user.
                let hours = effective_duration_hours("avatar_frame", item.duration_hours);
                sqlx::query(
                    r#"INSERT INTO user_boosts (user_id, avatar_frame, avatar_frame_until, updated_at)
                       VALUES ($1, $2, NOW() + make_interval(hours => $3), NOW())
                       ON CONFLICT (user_id) DO UPDATE SET
                         avatar_frame = EXCLUDED.avatar_frame,
                         avatar_frame_until = GREATEST(
                           CASE WHEN user_boosts.avatar_frame = EXCLUDED.avatar_frame
                                THEN COALESCE(user_boosts.avatar_frame_until, NOW())
                                ELSE NOW() END,
                           NOW())
                                       + make_interval(hours => $3),
                         updated_at = NOW()"#,
                )
                .bind(user_id)
                .bind(&item.id)
                .bind(hours)
                .execute(&mut *tx)
                .await?;
                frame_id = Some(item.id.clone());
            }
            "mystery_box" => {
                // v3.5.1 FIX (audit 5-e): cap số hộp/ngày + advisory lock
                // cấp user serialize check-then-act (cùng pattern RPS/
                // Trivia). Đếm event 'mystery_box' hôm nay — mỗi hộp mở luôn
                // chèn 1 event (mystery_xp ≥ 10 > 0).
                sqlx::query("SELECT pg_advisory_xact_lock(hashtext('shop:' || $1::text))")
                    .bind(user_id.to_string())
                    .execute(&mut *tx)
                    .await?;
                let opened_today: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(
                    format!(
                        "SELECT COUNT(*) FROM xp_events
                             WHERE user_id = $1 AND reason = 'mystery_box'
                               AND created_at >= {}",
                        crate::utils::SQL_TODAY_START_VN
                    )
                    .as_str(),
                ))
                .bind(user_id)
                .fetch_one(&mut *tx)
                .await?;
                if opened_today >= MYSTERY_BOX_DAILY_CAP {
                    // return Err trong tx → rollback → XP trừ ở trên được
                    // hoàn lại tự động.
                    return Err(AppError::BadRequest(format!(
                        "Chỉ mở được {MYSTERY_BOX_DAILY_CAP} Hộp Bí Ẩn mỗi ngày — quay lại vào ngày mai!"
                    )));
                }
                // rand_val 0..=9999 → XP min..max (chọn ở service layer,
                // hàm thuần ở dưới dùng chung test được)
                mystery_xp = mystery_xp_for(rand_val);
                sqlx::query("INSERT INTO xp_events (user_id, reason, amount) VALUES ($1, 'mystery_box', $2)")
                    .bind(user_id)
                    .bind(mystery_xp)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query(
                    r#"INSERT INTO user_xp_totals (user_id, total_xp)
                       VALUES ($1, $2)
                       ON CONFLICT (user_id)
                       DO UPDATE SET total_xp = user_xp_totals.total_xp + $2,
                                     updated_at = NOW()
                       RETURNING total_xp"#,
                )
                .bind(user_id)
                .bind(mystery_xp)
                .fetch_one(&mut *tx)
                .await?;
            }
            _ => {
                return Err(AppError::BadRequest("Loại vật phẩm không hỗ trợ".into()));
            }
        }
        tx.commit().await?;
        Ok(PurchaseOutcome {
            item_id: item.id,
            total_xp: total_xp + i64::from(mystery_xp),
            mystery_xp,
            frame_id,
        })
    }
}

#[cfg(test)]
mod v370_tests {
    use super::*;

    #[test]
    fn test_effective_duration_hours() {
        // Giá trị hợp lệ — giữ nguyên
        assert_eq!(effective_duration_hours("xp_boost", 72), 72);
        assert_eq!(effective_duration_hours("avatar_frame", 720), 720);
        // Giá trị phi lý (0/âm) — fallback theo kind
        assert_eq!(effective_duration_hours("xp_boost", 0), 24);
        assert_eq!(effective_duration_hours("xp_boost", -5), 24);
        assert_eq!(effective_duration_hours("avatar_frame", 0), 720);
        assert_eq!(effective_duration_hours("name_glow", 0), 720);
    }

    #[test]
    fn test_duration_label() {
        use crate::models::retention::duration_label;
        assert_eq!(duration_label(720), "30 ngày");
        assert_eq!(duration_label(168), "7 ngày");
        assert_eq!(duration_label(72), "3 ngày"); // 72h = 3 ngày tròn
        assert_eq!(duration_label(24), "1 ngày");
        assert_eq!(duration_label(36), "36 giờ"); // không tròn ngày → giờ
        assert_eq!(duration_label(0), "—");
    }

    /// Guard: khung Rồng Lửa PHẢI là vật phẩm đắt nhất (yêu cầu product —
    /// "bán cực đắt"). Nếu ai seed item mới đắt hơn, cân nhắc lại thông
    /// điệp "đắt nhất cửa hàng" trước khi bỏ guard này.
    #[test]
    fn test_dragon_frame_is_most_expensive() {
        let dragon = 5000;
        let others = [35, 45, 60, 100, 120, 150, 280, 300, 600, 900, 1500];
        for p in others {
            assert!(dragon > p, "Khung Rồng Lửa phải đắt nhất (giá đối thủ {p})");
        }
    }
}

/// XP mystery box từ rand_val (hàm thuần — test được).
/// rand_val 0..=9999; quy đổi tuyến tính min..max.
pub fn mystery_xp_for(rand_val: i32) -> i32 {
    let v = rand_val.rem_euclid(10_000);
    MYSTERY_MIN_XP + (i64::from(v) * i64::from(MYSTERY_MAX_XP - MYSTERY_MIN_XP) / 10_000) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mystery_xp_range() {
        assert_eq!(mystery_xp_for(0), MYSTERY_MIN_XP);
        assert_eq!(mystery_xp_for(9999), MYSTERY_MAX_XP - 1);
        assert_eq!(mystery_xp_for(-1), MYSTERY_MAX_XP - 1);
        // Luôn trong khoảng
        for v in (0..100).map(|i| i * 137) {
            let xp = mystery_xp_for(v);
            assert!((MYSTERY_MIN_XP..=MYSTERY_MAX_XP).contains(&xp));
        }
    }

    /// Compile-time guard (pattern janitor).
    const _: () = {
        assert!(MAX_STREAK_FREEZE >= 1 && MAX_STREAK_FREEZE <= 10);
    };

    /// v3.5.1 guard (audit 5-e): giá mystery box PHẢI > EV phần thưởng —
    /// nếu không, mở hộp lặp là in XP vô hạn. EV ≈ (min+max)/2 (phân phối
    /// gần đều của rand_val 0..9999).
    const _: () = {
        let ev = (MYSTERY_MIN_XP + MYSTERY_MAX_XP) / 2;
        assert!(
            MYSTERY_BOX_PRICE_XP > ev,
            "MYSTERY_BOX_PRICE_XP phải > EV payout — dùng máy in XP"
        );
        assert!(MYSTERY_BOX_DAILY_CAP >= 1 && MYSTERY_BOX_DAILY_CAP <= 20);
    };
}
