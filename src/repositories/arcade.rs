//! v3.0.0 — Repository arcade: vòng quay may mắn + câu đố hằng ngày.
//!
//! Spin: 1 lượt/ngày (PK user+date), trọng số chọn prize ở models
//! (hàm thuần test được), XP cộng qua cùng tx.
//! Trivia: mỗi ngày chọn N câu chưa trả lời (deterministic theo user+ngày),
//! đáp án chỉ chấm ở server (client không bao giờ nhận correct_index),
//! mỗi câu hỏi 1 user trả lời tối đa 1 lần trong đời (chặn retry farm).

use crate::error::{AppError, AppResult};
use crate::models::gamification::LevelInfo;
use crate::models::retention::{SpinPrize, TriviaAnswerResult, TriviaQuestionPublic};
use sqlx::PgPool;
use uuid::Uuid;

/// Số câu hỏi mỗi ngày.
/// v3.6.0 — tăng 3 → 5 theo yêu cầu "thêm nhiều câu hỏi hằng ngày
/// hơn" (bank cũng mở rộng 16 → 90 câu ở migration 034 — 5 câu/ngày đủ
/// dùng 18 ngày không lặp câu).
pub const TRIVIA_PER_DAY: i64 = 5;
/// XP mỗi câu đúng + thưởng đủ cả ngày.
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
    ) -> AppResult<(i32, i64, LevelInfo)> {
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
        let total: i64 = sqlx::query_scalar(
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
        // v3.4.2 — giới hạn theo quota còn lại hôm nay: đã trả lời đủ 3 câu
        // → không trả câu nào (trước đây vẫn render 3 câu mới + form, submit
        // mới bị chặn “đủ 3 câu” — UX mâu thuẫn với cap mới).
        let answered_sql = format!(
            r#"SELECT COUNT(*) FROM trivia_answers
               WHERE user_id = $1 AND answered_date = {}"#,
            crate::utils::SQL_TODAY_VN
        );
        let answered_today: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(answered_sql.as_str()))
            .bind(user_id)
            .fetch_one(pool)
            .await?;
        let remaining = (TRIVIA_PER_DAY - answered_today).max(0);
        if remaining == 0 {
            return Ok(Vec::new());
        }
        let sql = format!(
            r#"SELECT q.id, q.question, q.options
               FROM trivia_questions q
               WHERE q.is_active = TRUE
                 AND NOT EXISTS (
                   SELECT 1 FROM trivia_answers a
                   WHERE a.question_id = q.id AND a.user_id = $1
                 )
               ORDER BY hashtext($1::text || q.id::text || {}::text)
               LIMIT {remaining}"#,
            crate::utils::SQL_TODAY_VN
        );
        let rows = sqlx::query_as::<_, TriviaQuestionPublic>(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .fetch_all(pool)
            .await?;
        Ok(rows)
    }

    /// Trả lời 1 câu. Chấm đáp án server-side, ghi row (PK chống retry).
    /// Trả kết quả kèm XP cộng thêm nếu đúng.
    ///
    /// v3.4.2 FIX (audit "trivia answer-by-ID farm"): trước đây nhận ANY
    /// question_id đang active — user curl quét toàn bộ ID space, mỗi câu
    /// đúng +10 XP không giới hạn và đốt sạch catalog. Giờ giới hạn đúng
    /// `TRIVIA_PER_DAY` câu trả lời/ngày (đúng luật chơi 3 câu/ngày) —
    /// farming qua curl bị chặn đúng ngưỡng người chơi thật nhận được.
    /// # Errors
    /// Trả lỗi khi câu đã trả lời / không tồn tại / đủ quota ngày / DB fail.
    pub async fn answer(
        pool: &PgPool,
        user_id: Uuid,
        question_id: i32,
        answer_index: i32,
    ) -> AppResult<TriviaAnswerResult> {
        // Quota ngày: tối đa TRIVIA_PER_DAY câu (kể cả sai) mỗi ngày.
        let answered_sql = format!(
            r#"SELECT COUNT(*) FROM trivia_answers
               WHERE user_id = $1 AND answered_date = {}"#,
            crate::utils::SQL_TODAY_VN
        );
        let answered_today: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(answered_sql.as_str()))
            .bind(user_id)
            .fetch_one(pool)
            .await?;
        if answered_today >= TRIVIA_PER_DAY {
            return Err(AppError::BadRequest(
                "Bạn đã trả lời đủ số câu hỏi hôm nay — quay lại vào ngày mai!".into(),
            ));
        }
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
        let mut tx = pool.begin().await?;
        // v3.4.2 (audit vòng 4): advisory lock theo user + đếm lại quota
        // TRONG tx — N request song song trả N câu khác nhau cùng lúc đều
        // vượt pre-check ngoài đời (mỗi câu +10 XP). Lock cấp user serialize
        // các answer của CHÍNH user đó (kẻ farm), không ảnh hưởng user khác.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('trivia:' || $1::text))")
            .bind(user_id.to_string())
            .execute(&mut *tx)
            .await?;
        let answered_sql = format!(
            r#"SELECT COUNT(*) FROM trivia_answers
               WHERE user_id = $1 AND answered_date = {}"#,
            crate::utils::SQL_TODAY_VN
        );
        let answered_today: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(answered_sql.as_str()))
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?;
        if answered_today >= TRIVIA_PER_DAY {
            tx.rollback().await?;
            return Err(AppError::BadRequest(
                "Bạn đã trả lời đủ số câu hỏi hôm nay — quay lại vào ngày mai!".into(),
            ));
        }
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
        .execute(&mut *tx)
        .await?;
        if res.rows_affected() == 0 {
            tx.rollback().await?;
            return Err(AppError::BadRequest(
                "Bạn đã trả lời câu hỏi này rồi".into(),
            ));
        }
        let mut xp_awarded = 0;
        if is_correct {
            xp_awarded = TRIVIA_XP_PER_CORRECT;
            sqlx::query(
                "INSERT INTO xp_events (user_id, reason, amount) VALUES ($1, 'trivia', $2)",
            )
            .bind(user_id)
            .bind(xp_awarded)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"INSERT INTO user_xp_totals (user_id, total_xp)
                   VALUES ($1, $2)
                   ON CONFLICT (user_id)
                   DO UPDATE SET total_xp = user_xp_totals.total_xp + $2, updated_at = NOW()"#,
            )
            .bind(user_id)
            .bind(i64::from(xp_awarded))
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
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
    /// hoàn thành bộ câu hỏi).
    /// v3.4.2 FIX (audit race): trước đây "đếm event hôm nay" và "cộng XP"
    /// là 2 statement riêng — 3 tab trả lời song song cùng thấy
    /// `already == 0` → 2× TRIVIA_ALL_BONUS. Giờ dùng 1 statement
    /// `INSERT ... SELECT WHERE NOT EXISTS` (atomic ở cấp row) — chỉ request
    /// nào insert được event mới được cộng totals.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn maybe_award_all_bonus(pool: &PgPool, user_id: Uuid) -> AppResult<i32> {
        let correct = Self::correct_today_count(pool, user_id).await?;
        if correct < TRIVIA_PER_DAY {
            return Ok(0);
        }
        let sql = format!(
            r#"SELECT COUNT(*) FROM xp_events
               WHERE user_id = $1 AND reason = 'trivia_bonus'
                 AND created_at >= {}"#,
            crate::utils::SQL_TODAY_START_VN
        );
        // v3.4.2 (audit vòng 4): INSERT..SELECT WHERE NOT EXISTS KHÔNG atomic
        // dưới READ COMMITTED khi cột không có unique constraint — 2 tx cùng
        // thấy NOT EXISTS → cùng insert → 2× bonus. Giờ dùng advisory lock
        // theo user (giống answer) serialize check-then-insert trong 1 tx.
        let mut tx = pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('trivia:' || $1::text))")
            .bind(user_id.to_string())
            .execute(&mut *tx)
            .await?;
        let already: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?;
        if already > 0 {
            tx.rollback().await?;
            return Ok(0);
        }
        sqlx::query(
            "INSERT INTO xp_events (user_id, reason, amount) VALUES ($1, 'trivia_bonus', $2)",
        )
        .bind(user_id)
        .bind(TRIVIA_ALL_BONUS)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"INSERT INTO user_xp_totals (user_id, total_xp)
               VALUES ($1, $2)
               ON CONFLICT (user_id)
               DO UPDATE SET total_xp = user_xp_totals.total_xp + $2, updated_at = NOW()"#,
        )
        .bind(user_id)
        .bind(i64::from(TRIVIA_ALL_BONUS))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(TRIVIA_ALL_BONUS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-time guards: constants vô lý → fail build ngay (pattern janitor).
    /// v3.6.0: TRIVIA_PER_DAY 3 → 5 (mở rộng bank 16 → 90 câu ở migration 034).
    const _: () = {
        assert!(TRIVIA_PER_DAY == 5);
        assert!(TRIVIA_PER_DAY > 0 && TRIVIA_PER_DAY <= 20);
        assert!(TRIVIA_XP_PER_CORRECT > 0);
        assert!(TRIVIA_ALL_BONUS >= TRIVIA_XP_PER_CORRECT);
    };
}
