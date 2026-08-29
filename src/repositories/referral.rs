//! v3.0.0 — Repository chương trình giới thiệu (referral).
//!
//! Mã giới thiệu: sinh 8 ký tự base32 (không ký tự dễ nhầm 0/O/1/I),
//! lưu 1-1. Flow: bạn bè bấm link /r/{code} → cookie 30 ngày → đăng nhập
//! Google lần đầu → handler auth đọc cookie, gọi `record_referral`
//! (chặn tự-giới-thiệu, mỗi user chỉ được giới thiệu 1 lần) → cả hai
//! nhận REFERRAL_XP.

use crate::error::AppResult;
use crate::models::retention::ReferralInfo;
use sqlx::PgPool;
use uuid::Uuid;

/// XP thưởng cho cả người giới thiệu và người mới.
pub const REFERRAL_XP: i32 = 100;

/// Bảng chữ cái mã giới thiệu (bỏ 0,O,1,I để tránh nhìn nhầm).
const CODE_ALPHABET: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";

pub struct ReferralRepo;

impl ReferralRepo {
    /// Lấy mã giới thiệu của user (sinh nếu chưa có).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn get_or_create_code(pool: &PgPool, user_id: Uuid) -> AppResult<String> {
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT code FROM user_referral_codes WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        if let Some(code) = existing {
            return Ok(code);
        }
        // Sinh mã ngẫu nhiên 8 ký tự — thử tối đa 5 lần (collision hiếm)
        use rand::RngExt;
        for _ in 0..5 {
            let code: String = (0..8)
                .map(|_| {
                    let i = rand::rng().random_range(0..CODE_ALPHABET.len());
                    CODE_ALPHABET[i] as char
                })
                .collect();
            let res = sqlx::query(
                "INSERT INTO user_referral_codes (user_id, code) VALUES ($1, $2)
                 ON CONFLICT (user_id) DO NOTHING",
            )
            .bind(user_id)
            .bind(&code)
            .execute(pool)
            .await?;
            if res.rows_affected() > 0 {
                return Ok(code);
            }
            // user đã có mã (race) — đọc lại
            if let Some(c) = sqlx::query_scalar::<_, String>(
                "SELECT code FROM user_referral_codes WHERE user_id = $1",
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            {
                return Ok(c);
            }
        }
        // Lần cuối đọc lại (mã trùng 5 lần liên tiếp gần như không thể)
        let code: String = sqlx::query_scalar(
            "SELECT code FROM user_referral_codes WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| crate::error::AppError::BadRequest("Không sinh được mã giới thiệu".into()))?;
        Ok(code)
    }

    /// Giải mã code → user_id (None nếu sai).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn resolve_code(pool: &PgPool, code: &str) -> AppResult<Option<Uuid>> {
        let id: Option<Uuid> = sqlx::query_scalar(
            "SELECT user_id FROM user_referral_codes WHERE code = $1",
        )
        .bind(code)
        .fetch_optional(pool)
        .await?;
        Ok(id)
    }

    /// Ghi nhận referral sau khi người mới đăng nhập lần đầu.
    /// Trả (referrer_id, is_new). Chặn: tự giới thiệu, người được giới
    /// thiệu đã có referrer. KHÔNG cộng XP ở đây — caller (auth handler)
    /// gọi award_xp cho cả 2 phía.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn record_referral(
        pool: &PgPool,
        referrer_id: Uuid,
        referred_id: Uuid,
    ) -> AppResult<Option<Uuid>> {
        if referrer_id == referred_id {
            return Ok(None);
        }
        let res = sqlx::query(
            "INSERT INTO referrals (referred_id, referrer_id) VALUES ($1, $2)
             ON CONFLICT (referred_id) DO NOTHING",
        )
        .bind(referred_id)
        .bind(referrer_id)
        .execute(pool)
        .await?;
        if res.rows_affected() == 0 {
            return Ok(None);
        }
        Ok(Some(referrer_id))
    }

    /// Thống kê cho trang /referral.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn stats(pool: &PgPool, user_id: Uuid) -> AppResult<ReferralInfo> {
        let code = Self::get_or_create_code(pool, user_id).await?;
        let invited: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM referrals WHERE referrer_id = $1")
                .bind(user_id)
                .fetch_one(pool)
                .await?;
        Ok(ReferralInfo {
            code,
            invited_count: invited,
            xp_earned: i64::from(REFERRAL_XP) * invited,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_alphabet_no_ambiguous_chars() {
        for &b in CODE_ALPHABET {
            assert!(!b"0O1I".contains(&b), "ký tự dễ nhầm trong bảng mã: {b}");
        }
        assert!(CODE_ALPHABET.len() >= 30);
    }
}
