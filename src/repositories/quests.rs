//! v3.0.0 — Repository nhiệm vụ hằng ngày/tuần + onboarding checklist.
//!
//! Thiết kế:
//! - `period_date` với daily = ngày VN hôm nay; weekly = ngày thứ 2 của
//!   tuần hiện tại (giờ VN) — 2 kỳ cùng dùng 1 cột DATE, PK chống trùng.
//! - Nhiệm vụ mỗi kỳ được "sinh" lazily khi user mở trang /quests: chọn
//!   5 nhiệm vụ daily (deterministic theo user+ngày, không mỗi lần refresh
//!   lại đổi) + TẤT CẢ nhiệm vụ weekly. Progress upsert qua `bump`.
//! - Claim XP do user bấm (agency → engagement), hoàn thành tự động khi
//!   đủ target nhưng XP chỉ cộng khi claim.

use crate::error::AppResult;
use crate::models::retention::{QuestProgressRow, QuestWithProgress, ONBOARDING_STEPS};
use sqlx::PgPool;
use uuid::Uuid;

/// Số nhiệm vụ daily chọn mỗi ngày (từ catalog daily).
const DAILY_QUEST_COUNT: i64 = 5;

pub struct QuestRepo;

impl QuestRepo {
    /// Ngày kỳ daily (ngày VN hôm nay) — SQL fragment.
    const PERIOD_DAILY: &'static str = crate::utils::SQL_TODAY_VN;

    /// Ngày kỳ weekly: thứ 2 của tuần hiện tại theo giờ VN.
    /// `date_trunc('week', ts)` trả về thứ 2 trong Postgres (ISO week).
    const PERIOD_WEEKLY: &'static str =
        "(date_trunc('week', NOW() AT TIME ZONE 'Asia/Ho_Chi_Minh'))::date";

    /// Sinh (nếu chưa có) + trả danh sách nhiệm vụ hiện tại của user:
    /// 5 daily + toàn bộ weekly. Deterministic pick: ORDER BY
    /// hashtext(user_id::text || id || date) — cùng ngày luôn cùng bộ,
    /// khác user khác ngày khác bộ → cảm giác "riêng cho mình".
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn today_quests(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<QuestWithProgress>> {
        // Upsert các nhiệm vụ được chọn hôm nay (ON CONFLICT DO NOTHING —
        // idempotent, mở lại trang không reset progress).
        let sql = format!(
            r#"INSERT INTO user_quests (user_id, quest_id, period_date)
               SELECT $1, q.id, {}
               FROM quest_catalog q
               WHERE q.is_active = TRUE AND q.period = 'daily'
               ORDER BY hashtext($1::text || q.id || {}::text)
               LIMIT {}
               ON CONFLICT (user_id, quest_id, period_date) DO NOTHING"#,
            Self::PERIOD_DAILY,
            Self::PERIOD_DAILY,
            DAILY_QUEST_COUNT
        );
        sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .execute(pool)
            .await?;

        let sql = format!(
            r#"INSERT INTO user_quests (user_id, quest_id, period_date)
               SELECT $1, q.id, {}
               FROM quest_catalog q
               WHERE q.is_active = TRUE AND q.period = 'weekly'
               ON CONFLICT (user_id, quest_id, period_date) DO NOTHING"#,
            Self::PERIOD_WEEKLY
        );
        sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .execute(pool)
            .await?;

        // Đọc danh sách kèm tiến độ (daily + weekly)
        let sql = format!(
            r#"SELECT q.id, q.title, q.description, q.icon, q.stat_key, q.target,
                      q.xp_reward, q.period, q.is_active,
                      uq.progress, uq.completed_at, uq.claimed_at
               FROM quest_catalog q
               JOIN user_quests uq
                 ON uq.quest_id = q.id
                AND uq.user_id = $1
                AND uq.period_date = CASE WHEN q.period = 'daily'
                     THEN {daily} ELSE {weekly} END
               WHERE q.is_active = TRUE
               ORDER BY q.period DESC, q.xp_reward ASC"#,
            daily = Self::PERIOD_DAILY,
            weekly = Self::PERIOD_WEEKLY
        );
        let rows: Vec<QuestProgressRow> =
            sqlx::query_as(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(user_id)
                .fetch_all(pool)
                .await?;
        Ok(rows.into_iter().map(|r| r.into_progress()).collect())
    }

    /// Tăng tiến độ TẤT CẢ nhiệm vụ active khớp `stat_key` cho user trong
    /// kỳ hiện tại. Tự đánh dấu `completed_at` khi chạm target.
    /// Gọi fire-and-forget từ services/retention (lỗi không fail request).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn bump(
        pool: &PgPool,
        user_id: Uuid,
        stat_key: &str,
        delta: i32,
    ) -> AppResult<u64> {
        if delta <= 0 {
            return Ok(0);
        }
        // Đảm bảo các nhiệm vụ của kỳ đã được sinh (user chưa mở /quests
        // thì chưa có row — bump trước khi mở sẽ mất tiến độ). Gọi rẻ:
        // 2 INSERT ... SELECT ON CONFLICT.
        let sql = format!(
            r#"INSERT INTO user_quests (user_id, quest_id, period_date)
               SELECT $1, q.id, CASE WHEN q.period = 'daily' THEN {daily} ELSE {weekly} END
               FROM quest_catalog q
               WHERE q.is_active = TRUE AND q.stat_key = $2
               ON CONFLICT (user_id, quest_id, period_date) DO NOTHING"#,
            daily = Self::PERIOD_DAILY,
            weekly = Self::PERIOD_WEEKLY
        );
        sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .bind(stat_key)
            .execute(pool)
            .await?;

        let sql = format!(
            r#"UPDATE user_quests uq
               SET progress = LEAST(uq.progress + $3, q.target),
                   completed_at = CASE
                     WHEN uq.completed_at IS NULL
                          AND uq.progress + $3 >= q.target THEN NOW()
                     ELSE uq.completed_at END,
                   updated_at = NOW()
               FROM quest_catalog q
               WHERE q.id = uq.quest_id
                 AND uq.user_id = $1
                 AND q.stat_key = $2
                 AND q.is_active = TRUE
                 AND uq.period_date = CASE WHEN q.period = 'daily'
                      THEN {daily} ELSE {weekly} END
                 AND uq.claimed_at IS NULL"#,
            daily = Self::PERIOD_DAILY,
            weekly = Self::PERIOD_WEEKLY
        );
        let res = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .bind(stat_key)
            .bind(delta)
            .execute(pool)
            .await?;
        Ok(res.rows_affected())
    }

    /// Claim XP 1 nhiệm vụ đã hoàn thành. Trả về (xp_reward, total_xp, level)
    /// — lỗi BadRequest nếu chưa hoàn thành / đã claim / không tồn tại.
    /// # Errors
    /// Trả lỗi khi chưa đủ điều kiện hoặc DB fail.
    pub async fn claim(
        pool: &PgPool,
        user_id: Uuid,
        quest_id: &str,
    ) -> crate::error::AppResult<(i32, i32, crate::models::gamification::LevelInfo)> {
        let mut tx = pool.begin().await?;
        // UPDATE có điều kiện: phải completed + chưa claimed. rows_affected
        // = 0 → sai trạng thái (đưa qua match để phân biệt lỗi).
        let sql = format!(
            r#"UPDATE user_quests uq
               SET claimed_at = NOW(), updated_at = NOW()
               FROM quest_catalog q
               WHERE q.id = uq.quest_id
                 AND uq.user_id = $1 AND uq.quest_id = $2
                 AND uq.completed_at IS NOT NULL
                 AND uq.claimed_at IS NULL
                 AND uq.period_date = CASE WHEN q.period = 'daily'
                      THEN {daily} ELSE {weekly} END
               RETURNING q.xp_reward"#,
            daily = Self::PERIOD_DAILY,
            weekly = Self::PERIOD_WEEKLY
        );
        let xp: Option<i32> =
            sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(user_id)
                .bind(quest_id)
                .fetch_optional(&mut *tx)
                .await?;
        let Some(xp) = xp else {
            return Err(crate::error::AppError::BadRequest(
                "Nhiệm vụ chưa hoàn thành hoặc đã nhận thưởng".into(),
            ));
        };
        // Cộng XP ngay trong tx (reason 'quest' — không cap)
        sqlx::query("INSERT INTO xp_events (user_id, reason, amount) VALUES ($1, 'quest', $2)")
            .bind(user_id)
            .bind(xp)
            .execute(&mut *tx)
            .await?;
        let total: i32 = sqlx::query_scalar(
            r#"INSERT INTO user_xp_totals (user_id, total_xp)
               VALUES ($1, $2)
               ON CONFLICT (user_id)
               DO UPDATE SET total_xp = user_xp_totals.total_xp + $2,
                             updated_at = NOW()
               RETURNING total_xp"#,
        )
        .bind(user_id)
        .bind(xp)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok((xp, total, crate::models::gamification::level_from_xp(total)))
    }

    /// Đếm nhiệm vụ daily đã hoàn thành hôm nay (cho widget homepage).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn daily_progress_summary(
        pool: &PgPool,
        user_id: Uuid,
    ) -> AppResult<(i64, i64)> {
        let sql = format!(
            r#"SELECT
                 COUNT(*) FILTER (WHERE uq.completed_at IS NOT NULL),
                 COUNT(*)
               FROM user_quests uq
               JOIN quest_catalog q ON q.id = uq.quest_id
               WHERE uq.user_id = $1 AND q.period = 'daily'
                 AND uq.period_date = {}"#,
            Self::PERIOD_DAILY
        );
        let (done, total): (i64, i64) =
            sqlx::query_as(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(user_id)
                .fetch_one(pool)
                .await?;
        Ok((done, total))
    }
}

pub struct OnboardingRepo;

impl OnboardingRepo {
    /// Trạng thái 5 bước onboarding của user (done từ bảng onboarding_steps).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn steps_for_user(
        pool: &PgPool,
        user_id: Uuid,
    ) -> AppResult<Vec<crate::models::retention::OnboardingStepStatus>> {
        let done: Vec<String> = sqlx::query_scalar(
            "SELECT step FROM onboarding_steps WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        Ok(ONBOARDING_STEPS
            .iter()
            .map(|(code, label, icon, xp)| crate::models::retention::OnboardingStepStatus {
                code,
                label,
                icon,
                xp: *xp,
                done: done.iter().any(|d| d == code),
            })
            .collect())
    }

    /// Đánh dấu 1 bước hoàn thành (idempotent). Trả true nếu MỚI hoàn thành
    /// (caller cộng thưởng XP đúng 1 lần).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn complete_step(pool: &PgPool, user_id: Uuid, step: &str) -> AppResult<bool> {
        if !ONBOARDING_STEPS.iter().any(|(c, _, _, _)| *c == step) {
            return Ok(false);
        }
        let res = sqlx::query(
            "INSERT INTO onboarding_steps (user_id, step) VALUES ($1, $2)
             ON CONFLICT (user_id, step) DO NOTHING",
        )
        .bind(user_id)
        .bind(step)
        .execute(pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_period_weekly_sql_is_monday_trunc() {
        assert!(QuestRepo::PERIOD_WEEKLY.contains("date_trunc('week'"));
        assert!(QuestRepo::PERIOD_WEEKLY.contains("Asia/Ho_Chi_Minh"));
    }

    /// Compile-time guard (pattern janitor).
    const _: () = {
        assert!(DAILY_QUEST_COUNT > 0 && DAILY_QUEST_COUNT <= 10);
    };
}
