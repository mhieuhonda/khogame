//! v3.0.0 — Repository arcade: vòng quay may mắn + câu đố hằng ngày.
//!
//! Spin: 1 lượt/ngày (PK user+date), trọng số chọn prize ở models
//! (hàm thuần test được), XP cộng qua cùng tx.
//! Trivia: mỗi ngày chọn 3 câu chưa trả lời (deterministic theo user+ngày),
//! đáp án chỉ chấm ở server (client không bao giờ nhận correct_index),
//! mỗi câu hỏi 1 user trả lời tối đa 1 lần trong đời (chặn retry farm).

use crate::error::{AppError, AppResult};
use crate::models::gamification::LevelInfo;
use crate::models::retention::{SpinPrize, TriviaAnswerResult, TriviaQuestionPublic};
use sqlx::PgPool;
use uuid::Uuid;

/// Số câu hỏi mỗi ngày.
pub const TRIVIA_PER_DAY: i64 = 3;
/// XP mỗi câu đúng + thưởng cả 3.
pub const TRIVIA_XP_PER_CORRECT: i32 = 10;
pub const TRIVIA_ALL_BONUS: i32 = 20;

pub struct SpinRepo;

impl SpinRepo {
    /// Thực hiện 1 lượt quay. Trả (prize_xp, total_xp, level).
    /// Lỗi BadRequest nếu đã quay hôm nay.
    /// # Errors
    /// Trả lỗi khi đã quay / DB fail.
    pub async fn spin(
        pool: &PgPool,
        user_id: Uuid,
        rand_val: i32,
    ) -> AppResult<(i32, i32, LevelInfo)> {
        let prize = SpinPrize::pick(rand_val);
        let sql = format!(
            r#"INSERT INTO spins (user_id, spin_date, prize_xp)
               VALUES ($1, {}, $2)
               ON CONFLICT (user_id, spin_date) DO NOTHING
               RETURNING prize_xp"#,
            crate::utils::SQL_TODAY_VN
        );
        let mut tx = pool.begin().await?;
        let inserted: Option<i32> = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .bind(prize.xp)
            .fetch_optional(&mut *tx)
            .await?;
        let Some(xp) = inserted else {
            return Err(AppError::BadRequest(
                "Hôm nay bạn đã quay rồi — quay lại vào ngày mai nhé!".into(),
            ));
        };
        // Cộng XP cùng tx (reason 'spin' — không cap)
        sqlx::query("INSERT INTO xp_events (user_id, reason, amount) VALUES ($1, 'spin', $2)")
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

    /// Đã quay hôm nay chưa + giải gần nhất (cho trang /spin).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn today_spin(pool: &PgPool, user_id: Uuid) -> AppResult<Option<i32>> {
        let sql = format!(
            "SELECT prize_xp FROM spins WHERE user_id = $1 AND spin_date = {}",
            crate::utils::SQL_TODAY_VN
        );
        let row: Option<i32> = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
        Ok(row)
    }
}

pub struct TriviaRepo;

impl TriviaRepo {
    /// 3 câu hỏi hôm nay: chưa trả lời bao giờ, chọn deterministic theo
    /// user+ngày. Trả public model KHÔNG kèm đáp án.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn today_questions(
        pool: &PgPool,
        user_id: Uuid,
    ) -> AppResult<Vec<TriviaQuestionPublic>> {
        let sql = format!(
            r#"SELECT q.id, q.question, q.options
               FROM trivia_questions q
               WHERE q.is_active = TRUE
                 AND NOT EXISTS (
                   SELECT 1 FROM trivia_answers a
                   WHERE a.question_id = q.id AND a.user_id = $1
                 )
               ORDER BY hashtext($1::text || q.id::text || {}::text)
               LIMIT {}"#,
            crate::utils::SQL_TODAY_VN,
            TRIVIA_PER_DAY
        );
        let rows = sqlx::query_as::<_, TriviaQuestionPublic>(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .fetch_all(pool)
            .await?;
        Ok(rows)
    }

    /// Trả lời 1 câu. Chấm đáp án server-side, ghi row (PK chống retry).
    /// Trả kết quả kèm XP cộng thêm nếu đúng.
    /// # Errors
    /// Trả lỗi khi câu đã trả lời / không tồn tại / DB fail.
    pub async fn answer(
        pool: &PgPool,
        user_id: Uuid,
        question_id: i32,
        answer_index: i32,
    ) -> AppResult<TriviaAnswerResult> {
        let row: Option<(i32, String, serde_json::Value)> = sqlx::query_as(
            "SELECT correct_index, explanation, options FROM trivia_questions
             WHERE id = $1 AND is_active = TRUE",
        )
        .bind(question_id)
        .fetch_optional(pool)
        .await?;
        let Some((correct_index, explanation, options)) = row else {
            return Err(AppError::NotFound("Câu hỏi không tồn tại".into()));
        };
        if !(0..=3).contains(&answer_index) {
            return Err(AppError::BadRequest("Đáp án không hợp lệ".into()));
        }
        let is_correct = answer_index == correct_index;
        // PK (user_id, question_id) chống double-answer — DO NOTHING rồi
        // kiểm rows_affected: trượt race → coi như đã trả lời.
        let res = sqlx::query(
            r#"INSERT INTO trivia_answers (user_id, question_id, answer_index, is_correct)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT (user_id, question_id) DO NOTHING"#,
        )
        .bind(user_id)
        .bind(question_id)
        .bind(answer_index)
        .bind(is_correct)
        .execute(pool)
        .await?;
        if res.rows_affected() == 0 {
            return Err(AppError::BadRequest(
                "Bạn đã trả lời câu hỏi này rồi".into(),
            ));
        }
        let mut xp_awarded = 0;
        if is_correct {
            xp_awarded = TRIVIA_XP_PER_CORRECT;
            crate::repositories::GamificationRepo::award_xp(
                pool,
                user_id,
                "trivia",
                TRIVIA_XP_PER_CORRECT,
            )
            .await?;
        }
        Ok(TriviaAnswerResult {
            question_id,
            correct_index,
            is_correct,
            explanation,
            xp_awarded,
            options,
        })
    }

    /// Tổng số câu đúng trong ngày hôm nay (để client hiện thưởng cả 3).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn correct_today_count(pool: &PgPool, user_id: Uuid) -> AppResult<i64> {
        let sql = format!(
            r#"SELECT COUNT(*) FROM trivia_answers
               WHERE user_id = $1 AND is_correct = TRUE AND answered_date = {}"#,
            crate::utils::SQL_TODAY_VN
        );
        let c: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .fetch_one(pool)
            .await?;
        Ok(c)
    }

    /// Thưởng bonus trả lời đúng cả 3 câu trong ngày (gọi sau khi client
    /// hoàn thành bộ câu hỏi; idempotent — chỉ cộng 1 lần vì số câu đúng
    /// đạt 3 đúng 1 lần duy nhất trong ngày).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn maybe_award_all_bonus(pool: &PgPool, user_id: Uuid) -> AppResult<i32> {
        let correct = Self::correct_today_count(pool, user_id).await?;
        if correct < TRIVIA_PER_DAY {
            return Ok(0);
        }
        // Chống cộng 2 lần: đếm event 'trivia_bonus' hôm nay (không cap —
        // đếm thủ công thay vì dựa anti-farm cap của award_xp).
        let sql = format!(
            r#"SELECT COUNT(*) FROM xp_events
               WHERE user_id = $1 AND reason = 'trivia_bonus'
                 AND created_at >= {}"#,
            crate::utils::SQL_TODAY_START_VN
        );
        let already: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .fetch_one(pool)
            .await?;
        if already > 0 {
            return Ok(0);
        }
        let (_, _, _) = crate::repositories::GamificationRepo::award_xp(
            pool,
            user_id,
            "trivia_bonus",
            TRIVIA_ALL_BONUS,
        )
        .await?;
        Ok(TRIVIA_ALL_BONUS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-time guards: constants vô lý → fail build ngay (pattern janitor).
    const _: () = {
        assert!(TRIVIA_PER_DAY == 3);
        assert!(TRIVIA_XP_PER_CORRECT > 0);
        assert!(TRIVIA_ALL_BONUS >= TRIVIA_XP_PER_CORRECT);
    };
}
