//! v3.1.0 — Repository game Oẳn tù tì (Kéo búa bao).
//!
//! Thiết kế:
//! - Mỗi play độc lập, không session state.
//! - Daily cap: 30 plays/ngày (anti-farm — không spam để farm XP).
//! - XP thưởng: +2/win, 0/draw, 0/lose.
//! - Mọi play được ghi vào `rps_plays` (migration 024).
//! - Lifecycle stats (số thắng lifetime) được query trực tiếp trong
//!   GamificationRepo::check_and_award (cho huy hiệu rps_X_wins).

use crate::error::{AppError, AppResult};
use crate::models::gamification::LevelInfo;
use sqlx::PgPool;
use uuid::Uuid;

/// Giới hạn số ván chơi mỗi ngày (anti-farm).
pub const RPS_DAILY_CAP: i64 = 30;
/// XP thưởng mỗi ván thắng (draw/lose: 0 XP).
pub const RPS_XP_PER_WIN: i32 = 2;

/// Lựa chọn của người chơi hoặc bot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpsChoice {
    Rock,
    Paper,
    Scissors,
}

impl RpsChoice {
    /// Parse từ chuỗi client (POST form): "rock" | "paper" | "scissors".
    #[must_use]
    pub fn from_form(s: &str) -> Option<Self> {
        match s {
            "rock" => Some(Self::Rock),
            "paper" => Some(Self::Paper),
            "scissors" => Some(Self::Scissors),
            _ => None,
        }
    }

    /// Chọn ngẫu nhiên (uniform 1/3 each).
    #[must_use]
    pub fn random(rand_val: i32) -> Self {
        match rand_val.rem_euclid(3) {
            0 => Self::Rock,
            1 => Self::Paper,
            _ => Self::Scissors,
        }
    }

    /// Nhãn tiếng Việt cho hiển thị (Búa / Bao / Kéo).
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Rock => "Búa",
            Self::Paper => "Bao",
            Self::Scissors => "Kéo",
        }
    }

    /// ID gọn cho CSS class: rps-rock / rps-paper / rps-scissors.
    #[must_use]
    pub fn id(&self) -> &'static str {
        match self {
            Self::Rock => "rock",
            Self::Paper => "paper",
            Self::Scissors => "scissors",
        }
    }

    /// Emoji hiển thị.
    #[must_use]
    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Rock => "✊",
            Self::Paper => "✋",
            Self::Scissors => "✌️",
        }
    }
}

/// Kết quả 1 ván từ góc nhìn của user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpsOutcome {
    Win,
    Lose,
    Draw,
}

impl RpsOutcome {
    /// Định nghĩa kết quả từ cặp (user, bot) — không dùng match đầy đủ
    /// 3x3 để dễ đọc: draw nếu bằng, không thì so sánh quy tắc thắng.
    #[must_use]
    pub fn determine(user: RpsChoice, bot: RpsChoice) -> Self {
        if user == bot {
            return Self::Draw;
        }
        let user_wins = matches!(
            (user, bot),
            (RpsChoice::Rock, RpsChoice::Scissors)
                | (RpsChoice::Paper, RpsChoice::Rock)
                | (RpsChoice::Scissors, RpsChoice::Paper)
        );
        if user_wins {
            Self::Win
        } else {
            Self::Lose
        }
    }

    /// Nhãn result cho DB ('win' | 'lose' | 'draw').
    #[must_use]
    pub fn db_str(&self) -> &'static str {
        match self {
            Self::Win => "win",
            Self::Lose => "lose",
            Self::Draw => "draw",
        }
    }
}

/// Kết quả đầy đủ 1 ván RPS cho handler/UI.
#[derive(Debug, Clone)]
pub struct RpsPlayResult {
    pub user_choice: RpsChoice,
    pub bot_choice: RpsChoice,
    pub outcome: RpsOutcome,
    pub xp_awarded: i32,
    pub total_xp: i64,
    pub level: LevelInfo,
    pub plays_today: i64,
    pub wins_lifetime: i64,
}

pub struct RpsRepo;

impl RpsRepo {
    /// Đếm số ván đã chơi hôm nay (cho cap daily).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn plays_today_count(pool: &PgPool, user_id: Uuid) -> AppResult<i64> {
        let sql = format!(
            "SELECT COUNT(*) FROM rps_plays
             WHERE user_id = $1 AND created_at >= {}",
            crate::utils::SQL_TODAY_START_VN
        );
        let c: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .fetch_one(pool)
            .await?;
        Ok(c)
    }

    /// Tổng số thắng lifetime (cho UI + achievement eval).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn wins_lifetime(pool: &PgPool, user_id: Uuid) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM rps_plays WHERE user_id = $1 AND result = 'win'",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(c)
    }

    /// Chơi 1 ván RPS. Validate cap daily, ghi rps_plays row, cộng XP
    /// nếu thắng. Trả kết quả đầy đủ cho UI.
    /// # Errors
    /// Trả lỗi khi quá cap daily / DB fail.
    pub async fn play(
        pool: &PgPool,
        user_id: Uuid,
        user_choice: RpsChoice,
        bot_choice: RpsChoice,
    ) -> AppResult<RpsPlayResult> {
        let mut tx = pool.begin().await?;
        // Anti-farm: đếm trong tx để không race vượt cap
        let sql = format!(
            "SELECT COUNT(*) FROM rps_plays
             WHERE user_id = $1 AND created_at >= {}",
            crate::utils::SQL_TODAY_START_VN
        );
        let plays_today: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?;
        if plays_today >= RPS_DAILY_CAP {
            return Err(AppError::BadRequest(format!(
                "Bạn đã chơi {RPS_DAILY_CAP} ván hôm nay — quay lại vào ngày mai!"
            )));
        }
        let outcome = RpsOutcome::determine(user_choice, bot_choice);
        let xp_awarded = if outcome == RpsOutcome::Win {
            RPS_XP_PER_WIN
        } else {
            0
        };
        // Ghi play row — tất cả mọi ván đều log (kể cả lose/draw) để có
        // lịch sử chơi, nhưng chỉ thắng mới cộng XP.
        sqlx::query(
            r"INSERT INTO rps_plays (user_id, user_choice, bot_choice, result, xp_awarded)
               VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(user_id)
        .bind(user_choice.id())
        .bind(bot_choice.id())
        .bind(outcome.db_str())
        .bind(xp_awarded)
        .execute(&mut *tx)
        .await?;
        if xp_awarded > 0 {
            sqlx::query(
                "INSERT INTO xp_events (user_id, reason, amount) VALUES ($1, 'rps_win', $2)",
            )
            .bind(user_id)
            .bind(xp_awarded)
            .execute(&mut *tx)
            .await?;
            let total: i64 = sqlx::query_scalar(
                r"INSERT INTO user_xp_totals (user_id, total_xp)
                   VALUES ($1, $2)
                   ON CONFLICT (user_id)
                   DO UPDATE SET total_xp = user_xp_totals.total_xp + $2,
                                 updated_at = NOW()
                   RETURNING total_xp",
            )
            .bind(user_id)
            .bind(xp_awarded)
            .fetch_one(&mut *tx)
            .await?;
            tx.commit().await?;
            let level = crate::models::gamification::level_from_xp(total);
            let wins_lifetime = Self::wins_lifetime(pool, user_id).await.unwrap_or(0);
            Ok(RpsPlayResult {
                user_choice,
                bot_choice,
                outcome,
                xp_awarded,
                total_xp: total,
                level,
                plays_today: plays_today + 1,
                wins_lifetime,
            })
        } else {
            tx.commit().await?;
            let total_xp = crate::repositories::GamificationRepo::total_xp(pool, user_id)
                .await
                .unwrap_or(0);
            let level = crate::models::gamification::level_from_xp(total_xp);
            let wins_lifetime = Self::wins_lifetime(pool, user_id).await.unwrap_or(0);
            Ok(RpsPlayResult {
                user_choice,
                bot_choice,
                outcome,
                xp_awarded,
                total_xp,
                level,
                plays_today: plays_today + 1,
                wins_lifetime,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rps_determine_draw() {
        let r = RpsOutcome::determine(RpsChoice::Rock, RpsChoice::Rock);
        assert_eq!(r, RpsOutcome::Draw);
        let r = RpsOutcome::determine(RpsChoice::Paper, RpsChoice::Paper);
        assert_eq!(r, RpsOutcome::Draw);
        let r = RpsOutcome::determine(RpsChoice::Scissors, RpsChoice::Scissors);
        assert_eq!(r, RpsOutcome::Draw);
    }

    #[test]
    fn test_rps_determine_win() {
        // Rock beats Scissors
        assert_eq!(
            RpsOutcome::determine(RpsChoice::Rock, RpsChoice::Scissors),
            RpsOutcome::Win
        );
        // Paper beats Rock
        assert_eq!(
            RpsOutcome::determine(RpsChoice::Paper, RpsChoice::Rock),
            RpsOutcome::Win
        );
        // Scissors beats Paper
        assert_eq!(
            RpsOutcome::determine(RpsChoice::Scissors, RpsChoice::Paper),
            RpsOutcome::Win
        );
    }

    #[test]
    fn test_rps_determine_lose() {
        // Scissors beats Rock (user loses)
        assert_eq!(
            RpsOutcome::determine(RpsChoice::Rock, RpsChoice::Paper),
            RpsOutcome::Lose
        );
        assert_eq!(
            RpsOutcome::determine(RpsChoice::Paper, RpsChoice::Scissors),
            RpsOutcome::Lose
        );
        assert_eq!(
            RpsOutcome::determine(RpsChoice::Scissors, RpsChoice::Rock),
            RpsOutcome::Lose
        );
    }

    #[test]
    fn test_rps_random_is_valid_choice() {
        for v in 0..10 {
            let c = RpsChoice::random(v);
            assert!(matches!(
                c,
                RpsChoice::Rock | RpsChoice::Paper | RpsChoice::Scissors
            ));
        }
        // Negative also works (rem_euclid)
        let c = RpsChoice::random(-1);
        assert_eq!(c, RpsChoice::Scissors); // -1 % 3 = 2 → Scissors
    }

    #[test]
    fn test_rps_from_form() {
        assert_eq!(RpsChoice::from_form("rock"), Some(RpsChoice::Rock));
        assert_eq!(RpsChoice::from_form("paper"), Some(RpsChoice::Paper));
        assert_eq!(RpsChoice::from_form("scissors"), Some(RpsChoice::Scissors));
        assert_eq!(RpsChoice::from_form("invalid"), None);
    }

    #[test]
    fn test_rps_labels() {
        assert_eq!(RpsChoice::Rock.label(), "Búa");
        assert_eq!(RpsChoice::Paper.label(), "Bao");
        assert_eq!(RpsChoice::Scissors.label(), "Kéo");
        assert_eq!(RpsChoice::Rock.emoji(), "✊");
    }

    /// Compile-time guards.
    const _: () = {
        assert!(RPS_DAILY_CAP > 0);
        assert!(RPS_XP_PER_WIN > 0);
    };
}
