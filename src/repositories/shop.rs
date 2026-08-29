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

/// Giới hạn tồn kho streak_freeze (chặn mua ôm hàng).
pub const MAX_STREAK_FREEZE: i32 = 5;

pub struct ShopRepo;

impl ShopRepo {
    /// Danh sách vật phẩm active + tồn kho của user.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn list_for_user(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<ShopItemWithStock>> {
        let items = sqlx::query_as::<_, ShopItem>(
            "SELECT id, name, description, icon, price, kind, is_active
             FROM shop_items WHERE is_active = TRUE ORDER BY price ASC",
        )
        .fetch_all(pool)
        .await?;
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            let owned: i32 = sqlx::query_scalar(
                "SELECT COALESCE(quantity, 0) FROM user_inventory
                 WHERE user_id = $1 AND item_id = $2",
            )
            .bind(user_id)
            .bind(&item.id)
            .fetch_optional(pool)
            .await?
            .unwrap_or(0);
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
            "SELECT id, name, description, icon, price, kind, is_active
             FROM shop_items WHERE id = $1 AND is_active = TRUE",
        )
        .bind(item_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Vật phẩm không tồn tại".into()))?;

        let mut tx = pool.begin().await?;
        // 1) Trừ XP có điều kiện — atomic, thiếu tiền → BadRequest
        let total: Option<i32> = sqlx::query_scalar(
            r#"UPDATE user_xp_totals
               SET total_xp = total_xp - $2, updated_at = NOW()
               WHERE user_id = $1 AND total_xp >= $2
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
        match item.kind.as_str() {
            "streak_freeze" => {
                // Giới hạn tồn kho — đếm TRONG tx để không race vượt cap
                let qty: i32 = sqlx::query_scalar(
                    "SELECT COALESCE(quantity, 0) FROM user_inventory
                     WHERE user_id = $1 AND item_id = 'streak_freeze'",
                )
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await?
                .unwrap_or(0);
                if qty >= MAX_STREAK_FREEZE {
                    return Err(AppError::BadRequest(format!(
                        "Tối đa giữ {MAX_STREAK_FREEZE} Streak Freeze — dùng bớt rồi mua tiếp"
                    )));
                }
                sqlx::query(
                    r#"INSERT INTO user_inventory (user_id, item_id, quantity)
                       VALUES ($1, $2, 1)
                       ON CONFLICT (user_id, item_id)
                       DO UPDATE SET quantity = user_inventory.quantity + 1,
                                     updated_at = NOW()"#,
                )
                .bind(user_id)
                .bind(&item.id)
                .execute(&mut *tx)
                .await?;
            }
            "xp_boost" => {
                sqlx::query(
                    r#"INSERT INTO user_boosts (user_id, xp_boost_until, updated_at)
                       VALUES ($1, NOW() + INTERVAL '24 hours', NOW())
                       ON CONFLICT (user_id) DO UPDATE SET
                         xp_boost_until = GREATEST(
                           COALESCE(user_boosts.xp_boost_until, NOW()), NOW())
                                       + INTERVAL '24 hours',
                         updated_at = NOW()"#,
                )
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
            }
            "name_glow" => {
                sqlx::query(
                    r#"INSERT INTO user_boosts (user_id, name_glow_until, updated_at)
                       VALUES ($1, NOW() + INTERVAL '30 days', NOW())
                       ON CONFLICT (user_id) DO UPDATE SET
                         name_glow_until = GREATEST(
                           COALESCE(user_boosts.name_glow_until, NOW()), NOW())
                                       + INTERVAL '30 days',
                         updated_at = NOW()"#,
                )
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
            }
            "mystery_box" => {
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
            total_xp: total_xp + mystery_xp,
            mystery_xp,
        })
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
}
