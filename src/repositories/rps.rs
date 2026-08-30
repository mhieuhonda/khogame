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

// ============================================================
// v3.3.0 — PvP MATCHMAKING: ghép 2 người dùng NGẪU NHIÊN.
//
// Luồng (stateless client, state 100% trong PostgreSQL — an toàn với
// nhiều process/restart):
// 1. POST /rps/play {choice}:
//    - Nếu có match `waiting` của NGƯỜI KHÁC (cửa sổ 5 phút) → JOIN,
//      resolve NGAY LẬP TỨC (2 bên nhận kết quả cùng lúc).
//    - Không có → tạo match `waiting`, trả partial "đang tìm người" +
//      HTMX poll /rps/match/{id}/status mỗi 3s.
// 2. GET /rps/match/{id}/status:
//    - `waiting` quá 90s → TỰ GHÉP GLM 5.3 (AI Agent mặc định, migration
//      027) — `is_ai_fallback = TRUE`, không để người chơi treo vô hạn.
//    - `finished` → partial kết quả.
// * Mỗi nước chơi vẫn ghi vào `rps_plays` (stats + huy hiệu giữ nguyên).
// * Chống race: SELECT ... FOR UPDATE SKIP LOCKED khi join match.
// ============================================================

/// Thời gian chờ ghép người chơi thực trước khi fallback sang AI (giây).
pub const RPS_PVP_WAIT_SECS: i64 = 90;
/// Cửa sổ 1 match `waiting` còn có thể bị join (giây) — quá 5 phút coi
/// như người tạo đã rời (poll sẽ huỷ/fallback trước đó).
pub const RPS_PVP_JOIN_WINDOW_SECS: i64 = 300;
/// XP thắng khi PvP (+3 — hơn +2 khi đấu với bot cũ, khuyến khích PvP).
pub const RPS_PVP_XP_WIN: i32 = 3;

/// Bên trong match (vai trò của "me").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpsSide {
    P1,
    P2,
}

/// Hàng `rps_matches` (sqlx FromRow).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RpsMatchRow {
    pub id: i64,
    pub player1_id: Uuid,
    pub player2_id: Option<Uuid>,
    pub player1_choice: String,
    pub player2_choice: Option<String>,
    pub status: String,
    pub winner_id: Option<Uuid>,
    pub is_ai_fallback: bool,
    pub xp1: i32,
    pub xp2: i32,
}

/// Thông tin đối thủ cho UI.
#[derive(Debug, Clone)]
pub struct RpsOpponent {
    pub user_id: Uuid,
    pub username: String,
    pub display_name: String,
    pub is_ai: bool,
}

/// Kết quả PvP từ góc nhìn của người gọi.
#[derive(Debug)]
pub enum RpsPvpStatus {
    /// Match đang chờ người thứ 2 — client poll status.
    Waiting {
        match_id: i64,
        /// Số giây còn lại trước khi fallback AI.
        wait_secs: i64,
    },
    /// Đã xong — có kết quả.
    Resolved {
        my_choice: RpsChoice,
        opponent_choice: RpsChoice,
        outcome: RpsOutcome,
        xp_awarded: i32,
        total_xp: i64,
        level: LevelInfo,
        plays_today: i64,
        wins_lifetime: i64,
        opponent: RpsOpponent,
        /// Đối thủ là AI fallback (GLM 5.3) — UI hiển thị badge.
        is_ai_fallback: bool,
    },
    /// Match đã bị huỷ (người tạo rời) — UI mời chơi lại.
    Cancelled,
}

/// Cộng XP trong tx (upsert user_xp_totals). amount <= 0 → no-op.
async fn award_xp_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    amount: i32,
) -> AppResult<()> {
    if amount <= 0 {
        return Ok(());
    }
    sqlx::query("INSERT INTO xp_events (user_id, reason, amount) VALUES ($1, 'rps_win', $2)")
        .bind(user_id)
        .bind(amount)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        r#"INSERT INTO user_xp_totals (user_id, total_xp)
           VALUES ($1, $2)
           ON CONFLICT (user_id)
           DO UPDATE SET total_xp = user_xp_totals.total_xp + $2, updated_at = NOW()"#,
    )
    .bind(user_id)
    .bind(i64::from(amount))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Ghi 1 hàng rps_plays (cả PvP lẫn AI fallback đều ghi — giữ stats).
async fn insert_play_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    my_choice: &str,
    opponent_choice: &str,
    result: &str,
    xp: i32,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO rps_plays (user_id, user_choice, bot_choice, result, xp_awarded)
           VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(user_id)
    .bind(my_choice)
    .bind(opponent_choice)
    .bind(result)
    .bind(xp)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

impl RpsRepo {
    /// POST /rps/play — PvP: join match chờ của người khác hoặc tạo mới.
    ///
    /// # Errors
    ///
    /// Trả lỗi khi quá daily cap / DB fail.
    pub async fn pvp_play(
        pool: &PgPool,
        user_id: Uuid,
        user_choice: RpsChoice,
    ) -> AppResult<RpsPvpStatus> {
        // Anti-farm: cap daily như bot mode.
        let plays_today = Self::plays_today_count(pool, user_id).await?;
        if plays_today >= RPS_DAILY_CAP {
            return Err(AppError::BadRequest(format!(
                "Bạn đã chơi {RPS_DAILY_CAP} ván hôm nay — quay lại vào ngày mai!"
            )));
        }

        let rand_val: i32 = {
            use rand::RngExt;
            rand::rng().random_range(0..1000)
        };

        // 1) Huỷ các match waiting cũ của tôi (chỉ giữ 1 hàng đợi duy nhất).
        sqlx::query(
            "UPDATE rps_matches SET status = 'cancelled', updated_at = NOW()
             WHERE player1_id = $1 AND status = 'waiting'",
        )
        .bind(user_id)
        .execute(pool)
        .await?;

        // 2) Thử join match waiting của NGƯỜI KHÁC (cũ nhất trước — FIFO).
        for _ in 0..3 {
            let candidate: Option<(i64, Uuid, String)> = sqlx::query_as(
                r#"SELECT id, player1_id, player1_choice FROM rps_matches
                   WHERE status = 'waiting'
                     AND player1_id <> $1
                     AND created_at > NOW() - make_interval(secs => $2)
                   ORDER BY created_at ASC
                   LIMIT 1
                   FOR UPDATE SKIP LOCKED"#,
            )
            .bind(user_id)
            .bind(RPS_PVP_JOIN_WINDOW_SECS)
            .fetch_optional(pool)
            .await?;
            let Some((match_id, p1_id, p1_choice_str)) = candidate else {
                break;
            };
            let p1_choice =
                RpsChoice::from_form(&p1_choice_str).unwrap_or_else(|| RpsChoice::random(rand_val));

            // Resolve trong 1 tx: update match + ghi plays + cộng XP.
            let mut tx = pool.begin().await?;
            let outcome = RpsOutcome::determine(p1_choice, user_choice); // góc nhìn P1
            let (xp1, xp2) = match outcome {
                RpsOutcome::Win => (RPS_PVP_XP_WIN, 0),
                RpsOutcome::Lose => (0, RPS_PVP_XP_WIN),
                RpsOutcome::Draw => (0, 0),
            };
            let winner_id = match outcome {
                RpsOutcome::Win => Some(p1_id),
                RpsOutcome::Lose => Some(user_id),
                RpsOutcome::Draw => None,
            };
            let updated = sqlx::query(
                r#"UPDATE rps_matches SET player2_id = $1, player2_choice = $2,
                       status = 'finished', winner_id = $3, xp1 = $4, xp2 = $5, updated_at = NOW()
                   WHERE id = $6 AND status = 'waiting'"#,
            )
            .bind(user_id)
            .bind(user_choice.id())
            .bind(winner_id)
            .bind(xp1)
            .bind(xp2)
            .bind(match_id)
            .execute(&mut *tx)
            .await?;
            if updated.rows_affected() == 0 {
                // Match vừa bị ai đó join/huỷ — thử hàng chờ khác.
                tx.rollback().await?;
                continue;
            }
            // v3.4.2 FIX (audit "daily cap ngoài tx"): đếm lại TRONG tx sau
            // khi giành được match — N request đồng thời đều vượt pre-check
            // ngoài đời (cùng đọc plays_today < cap) giờ chỉ những request
            // serialize sau INSERT mới thấy count tăng dần; request vượt
            // cap bị rollback. (P1 được resolve kèm theo khi có người join —
            // overshoot tối đa 1 ván/ngày, chấp nhận.)
            let sql_cap = format!(
                "SELECT COUNT(*) FROM rps_plays
                 WHERE user_id = $1 AND created_at >= {}",
                crate::utils::SQL_TODAY_START_VN
            );
            let my_plays: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql_cap.as_str()))
                .bind(user_id)
                .fetch_one(&mut *tx)
                .await?;
            if my_plays >= RPS_DAILY_CAP {
                tx.rollback().await?;
                return Err(AppError::BadRequest(format!(
                    "Bạn đã chơi {RPS_DAILY_CAP} ván hôm nay — quay lại vào ngày mai!"
                )));
            }
            // Plays cho cả 2 (góc nhìn riêng từng bên).
            insert_play_tx(
                &mut tx,
                p1_id,
                p1_choice.id(),
                user_choice.id(),
                outcome.db_str(),
                xp1,
            )
            .await?;
            let my_outcome = match outcome {
                RpsOutcome::Win => RpsOutcome::Lose,
                RpsOutcome::Lose => RpsOutcome::Win,
                RpsOutcome::Draw => RpsOutcome::Draw,
            };
            insert_play_tx(
                &mut tx,
                user_id,
                user_choice.id(),
                p1_choice.id(),
                my_outcome.db_str(),
                xp2,
            )
            .await?;
            award_xp_tx(&mut tx, p1_id, xp1).await?;
            let total_xp = if xp2 > 0 {
                sqlx::query_scalar(
                    r#"INSERT INTO user_xp_totals (user_id, total_xp)
                       VALUES ($1, $2)
                       ON CONFLICT (user_id)
                       DO UPDATE SET total_xp = user_xp_totals.total_xp + $2, updated_at = NOW()
                       RETURNING total_xp"#,
                )
                .bind(user_id)
                .bind(i64::from(xp2))
                .fetch_one(&mut *tx)
                .await?
            } else {
                crate::repositories::GamificationRepo::total_xp(pool, user_id)
                    .await
                    .unwrap_or(0)
            };
            tx.commit().await?;

            let opponent = Self::opponent_info(pool, p1_id).await?;
            let level = crate::models::gamification::level_from_xp(total_xp);
            let wins_lifetime = Self::wins_lifetime(pool, user_id).await.unwrap_or(0);
            return Ok(RpsPvpStatus::Resolved {
                my_choice: user_choice,
                opponent_choice: p1_choice,
                outcome: my_outcome,
                xp_awarded: xp2,
                total_xp,
                level,
                plays_today: plays_today + 1,
                wins_lifetime,
                opponent,
                is_ai_fallback: false,
            });
        }

        // 3) Không ai đang chờ → tạo match waiting mới.
        let match_id: i64 = sqlx::query_scalar(
            r#"INSERT INTO rps_matches (player1_id, player1_choice, status)
               VALUES ($1, $2, 'waiting') RETURNING id"#,
        )
        .bind(user_id)
        .bind(user_choice.id())
        .fetch_one(pool)
        .await?;
        Ok(RpsPvpStatus::Waiting {
            match_id,
            wait_secs: RPS_PVP_WAIT_SECS,
        })
    }

    /// GET /rps/match/{id}/status — poll: trả kết quả / tiếp tục chờ /
    /// fallback AI khi quá 90s / huỷ.
    ///
    /// # Errors
    ///
    /// Trả lỗi khi không phải người trong match / DB fail.
    pub async fn pvp_status(
        pool: &PgPool,
        user_id: Uuid,
        match_id: i64,
    ) -> AppResult<RpsPvpStatus> {
        let row: Option<RpsMatchRow> = sqlx::query_as(
            r#"SELECT id, player1_id, player2_id, player1_choice, player2_choice,
                      status, winner_id, is_ai_fallback, xp1, xp2
               FROM rps_matches WHERE id = $1"#,
        )
        .bind(match_id)
        .fetch_optional(pool)
        .await?;
        let Some(m) = row else {
            return Err(AppError::NotFound("Không tìm thấy match".into()));
        };
        let my_side = if m.player1_id == user_id {
            RpsSide::P1
        } else if m.player2_id == Some(user_id) {
            RpsSide::P2
        } else {
            return Err(AppError::Forbidden("Bạn không thuộc match này".into()));
        };

        match m.status.as_str() {
            "finished" => Self::resolved_from_row(pool, user_id, my_side, m).await,
            "cancelled" => Ok(RpsPvpStatus::Cancelled),
            _ => {
                // waiting — hết 90s → tự ghép GLM 5.3 (AI fallback).
                let age_secs: f64 = sqlx::query_scalar(
                    "SELECT EXTRACT(EPOCH FROM (NOW() - created_at))::float8
                     FROM rps_matches WHERE id = $1",
                )
                .bind(match_id)
                .fetch_one(pool)
                .await?;
                if age_secs < RPS_PVP_WAIT_SECS as f64 {
                    return Ok(RpsPvpStatus::Waiting {
                        match_id,
                        wait_secs: RPS_PVP_WAIT_SECS - age_secs as i64,
                    });
                }
                Self::fallback_to_ai(pool, user_id, my_side, m).await
            }
        }
    }

    /// Chuyển match waiting → đấu với GLM 5.3 và resolve ngay.
    async fn fallback_to_ai(
        pool: &PgPool,
        user_id: Uuid,
        my_side: RpsSide,
        m: RpsMatchRow,
    ) -> AppResult<RpsPvpStatus> {
        let ai_id = crate::repositories::AiAgentRepo::default_agent_user_id(pool).await?;
        let rand_val: i32 = {
            use rand::RngExt;
            rand::rng().random_range(0..1000)
        };
        let ai_choice = RpsChoice::random(rand_val);

        // Match `waiting` chỉ có P1 (player2 NULL khi chờ) — fallback chỉ
        // hợp lệ cho P1. Phòng hộ: P2 poll một match waiting → trả chờ tiếp.
        if my_side != RpsSide::P1 {
            return Ok(RpsPvpStatus::Waiting {
                match_id: m.id,
                wait_secs: RPS_PVP_WAIT_SECS,
            });
        }
        let my_choice_str = m.player1_choice.clone();
        let my_choice =
            RpsChoice::from_form(&my_choice_str).unwrap_or_else(|| RpsChoice::random(rand_val));
        let p2_choice_str = ai_choice.id().to_string();

        let outcome = RpsOutcome::determine(my_choice, ai_choice);
        let my_xp = if outcome == RpsOutcome::Win {
            RPS_PVP_XP_WIN
        } else {
            0
        };
        let winner_id = match outcome {
            RpsOutcome::Win => Some(user_id),
            RpsOutcome::Lose => Some(ai_id),
            RpsOutcome::Draw => None,
        };

        let mut tx = pool.begin().await?;
        let (p1_id, p1_choice, p2_choice, xp1, xp2) = (
            user_id,
            my_choice_str.clone(),
            p2_choice_str.clone(),
            my_xp,
            0,
        );
        // v3.4.2 FIX (audit race A/B): guard `AND status = 'waiting'` —
        // hai tab poll đồng thời sau 90s đều đọc status='waiting' trước khi
        // fallback chạy → cả hai resolve match → double `rps_plays` + double
        // XP; hoặc người chơi thật vừa JOIN giữa lúc đọc và ghi → fallback
        // GHI ĐÈ player2_id, đá người thật khỏi match. Chỉ request nào
        // chuyển được status mới được resolve; request thua → rollback và
        // đọc lại trạng thái thật của match.
        let updated = sqlx::query(
            r#"UPDATE rps_matches SET player2_id = $1, player2_choice = $2,
                   status = 'finished', winner_id = $3, is_ai_fallback = TRUE,
                   xp1 = $4, xp2 = $5, updated_at = NOW()
               WHERE id = $6 AND status = 'waiting'"#,
        )
        .bind(ai_id)
        .bind(&p2_choice)
        .bind(winner_id)
        .bind(xp1)
        .bind(xp2)
        .bind(m.id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            // Match không còn waiting (người thật đã join / tab khác đã
            // resolve) — không cộng XP, không ghi plays; đọc lại match.
            tx.rollback().await?;
            let fresh = sqlx::query_as::<_, RpsMatchRow>(
                r#"SELECT id, player1_id, player2_id, player1_choice, player2_choice,
                          status, winner_id, is_ai_fallback, xp1, xp2
                   FROM rps_matches WHERE id = $1"#,
            )
            .bind(m.id)
            .fetch_optional(pool)
            .await?;
            return match fresh {
                Some(f) if f.status == "finished" => {
                    Self::resolved_from_row(pool, user_id, RpsSide::P1, f).await
                }
                _ => Ok(RpsPvpStatus::Waiting {
                    match_id: m.id,
                    wait_secs: RPS_PVP_WAIT_SECS,
                }),
            };
        }
        // v3.4.2 FIX (audit cap): fallback resolve match kể cả khi user đã
        // chạm cap (match phải có kết thúc), nhưng KHÔNG ghi plays/XP thêm —
        // chống farm XP bằng cách để match timeout rồi poll fallback.
        let sql_cap = format!(
            "SELECT COUNT(*) FROM rps_plays
             WHERE user_id = $1 AND created_at >= {}",
            crate::utils::SQL_TODAY_START_VN
        );
        let under_cap: bool = sqlx::query_scalar(sqlx::AssertSqlSafe(sql_cap.as_str()))
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await
            .map(|c: i64| c < RPS_DAILY_CAP)
            .unwrap_or(false);
        let awarded_xp = if under_cap { my_xp } else { 0 };
        if under_cap {
            insert_play_tx(
                &mut tx,
                p1_id,
                &p1_choice,
                &p2_choice,
                RpsOutcome::determine(
                    RpsChoice::from_form(&p1_choice).unwrap_or(my_choice),
                    RpsChoice::from_form(&p2_choice).unwrap_or(ai_choice),
                )
                .db_str(),
                xp1,
            )
            .await?;
            insert_play_tx(
                &mut tx,
                ai_id,
                &p2_choice,
                &p1_choice,
                RpsOutcome::determine(
                    RpsChoice::from_form(&p2_choice).unwrap_or(ai_choice),
                    RpsChoice::from_form(&p1_choice).unwrap_or(my_choice),
                )
                .db_str(),
                xp2,
            )
            .await?;
            award_xp_tx(&mut tx, user_id, my_xp).await?;
        }
        tx.commit().await?;

        let opponent = RpsOpponent {
            user_id: ai_id,
            username: "glm53".into(),
            display_name: "GLM 5.3".into(),
            is_ai: true,
        };
        let total_xp = crate::repositories::GamificationRepo::total_xp(pool, user_id)
            .await
            .unwrap_or(0);
        let level = crate::models::gamification::level_from_xp(total_xp);
        let plays_today = Self::plays_today_count(pool, user_id).await.unwrap_or(0);
        let wins_lifetime = Self::wins_lifetime(pool, user_id).await.unwrap_or(0);
        Ok(RpsPvpStatus::Resolved {
            my_choice,
            opponent_choice: ai_choice,
            outcome,
            xp_awarded: awarded_xp,
            total_xp,
            level,
            plays_today,
            wins_lifetime,
            opponent,
            is_ai_fallback: true,
        })
    }

    /// Build `Resolved` từ hàng match đã finished (đường poll của P1/P2).
    async fn resolved_from_row(
        pool: &PgPool,
        user_id: Uuid,
        my_side: RpsSide,
        m: RpsMatchRow,
    ) -> AppResult<RpsPvpStatus> {
        let (my_choice_str, opp_choice_str, my_xp, opp_id) = match my_side {
            RpsSide::P1 => (
                m.player1_choice.clone(),
                m.player2_choice.clone().unwrap_or_default(),
                m.xp1,
                m.player2_id,
            ),
            RpsSide::P2 => (
                m.player2_choice.clone().unwrap_or_default(),
                m.player1_choice.clone(),
                m.xp2,
                Some(m.player1_id),
            ),
        };
        let my_choice =
            RpsChoice::from_form(&my_choice_str).unwrap_or_else(|| RpsChoice::random(0));
        let opp_choice =
            RpsChoice::from_form(&opp_choice_str).unwrap_or_else(|| RpsChoice::random(1));
        let outcome = RpsOutcome::determine(my_choice, opp_choice);
        let Some(opp_id) = opp_id else {
            return Err(AppError::NotFound("Match thiếu đối thủ".into()));
        };
        let opponent = Self::opponent_info(pool, opp_id).await?;
        let total_xp = crate::repositories::GamificationRepo::total_xp(pool, user_id)
            .await
            .unwrap_or(0);
        let level = crate::models::gamification::level_from_xp(total_xp);
        let plays_today = Self::plays_today_count(pool, user_id).await.unwrap_or(0);
        let wins_lifetime = Self::wins_lifetime(pool, user_id).await.unwrap_or(0);
        Ok(RpsPvpStatus::Resolved {
            my_choice,
            opponent_choice: opp_choice,
            outcome,
            xp_awarded: my_xp,
            total_xp,
            level,
            plays_today,
            wins_lifetime,
            opponent,
            is_ai_fallback: m.is_ai_fallback,
        })
    }

    /// username / display_name / role của đối thủ.
    async fn opponent_info(pool: &PgPool, user_id: Uuid) -> AppResult<RpsOpponent> {
        let (username, display_name, role): (String, String, String) =
            sqlx::query_as("SELECT username, display_name, role::text FROM users WHERE id = $1")
                .bind(user_id)
                .fetch_one(pool)
                .await?;
        Ok(RpsOpponent {
            user_id,
            username,
            display_name,
            is_ai: role == "ai_agent",
        })
    }
}
