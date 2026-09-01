//! v2.9.0 — Repository gamification: XP, điểm danh chuỗi, huy hiệu,
//! bảng xếp hạng. Theo convention codebase: struct rỗng + static methods,
//! nhận `&PgPool` tường minh.

use crate::error::{AppError, AppResult};
use crate::models::gamification::{
    level_from_xp, Achievement, AchievementWithStatus, ActivityEvent, DailyCheckin,
    LeaderboardEntry, LevelInfo,
};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

/// Định mức XP cho từng hành động (v2.9.0).
pub mod xp {
    /// Điểm danh hàng ngày (chưa kể thưởng chuỗi).
    pub const DAILY_CHECKIN: i32 = 5;
    /// Thưởng chuỗi tối đa mỗi ngày điểm danh (min(streak, 7)).
    pub const MAX_STREAK_BONUS: i32 = 7;
    /// Đăng game mới (khi publish).
    pub const POST_GAME: i32 = 50;
    /// Bài tin được admin duyệt.
    pub const POST_NEWS: i32 = 40;
    /// Viết bình luận (game hoặc tin).
    pub const COMMENT: i32 = 3;
    /// Viết review game.
    pub const REVIEW: i32 = 15;
    /// Chia sẻ repo GitHub.
    pub const REPO: i32 = 20;
    /// Gửi tin nhắn chat.
    pub const CHAT_MESSAGE: i32 = 1;
    /// NHẬN được lượt thích (game của mình được like).
    pub const RECEIVED_LIKE: i32 = 2;
    /// NHẬN được lượt tải game.
    pub const RECEIVED_DOWNLOAD: i32 = 1;
    /// NHẬN được lượt theo dõi mới.
    pub const RECEIVED_FOLLOW: i32 = 10;

    /// Anti-farm: số lượng tối đa event XP mỗi NGÀY cho các reason
    /// dễ spam (bình luận, chat). Vượt ngưỡng → không cộng thêm.
    /// (v3.0.0: bỏ ghi event amount=0 — activity feed chỉ đọc amount>0
    /// nên ghi amount=0 là thuần rác DB, nhân lên với mỗi chat/like.)
    pub const MAX_COMMENT_XP_PER_DAY: i32 = 10;
    pub const MAX_CHAT_XP_PER_DAY: i32 = 20;
    pub const MAX_RECEIVED_LIKE_XP_PER_DAY: i32 = 50;
    pub const MAX_RECEIVED_DOWNLOAD_XP_PER_DAY: i32 = 50;
    /// v3.0.0 FIX (XP farm): `received_follow` trước đây KHÔNG có cap —
    /// unfollow → re-follow lặp vòng (120 lần/phút theo bucket route)
    /// bơm +10 XP/lần vào account mục tiêu. Cap 50 XP/ngày tương tự
    /// `received_like`.
    pub const MAX_RECEIVED_FOLLOW_XP_PER_DAY: i32 = 50;
    /// v3.5.1 FIX (XP farm — audit vòng 10, task 5-b): các reason
    /// "tạo mới nội dung" đều có vòng lặp xoá-rồi-làm-lại mà v3.0.0
    /// chưa cap, vì mỗi lần là 1 event INSERT mới:
    /// - `post_game` (50): farm draft→publish (đã chặn ở `GameRepo::publish`
    ///   bằng cột published_at, cap này là lớp phòng vệ thứ 2) + xoá game
    ///   rồi đăng lại.
    /// - `post_news` (40): xoá news đã duyệt rồi submit lại.
    /// - `review` (15): xoá review (+15) rồi review lại (+15) — ~900 XP/phút.
    /// - `repo` (20): xoá repo đăng ký (+20) rồi register lại (+20).
    ///
    /// Giá trị = SỐ EVENT/ngày (cùng ngữ nghĩa COUNT như các cap trên):
    /// 4 game (200 XP), 4 news (160 XP), 6 review (90 XP), 5 repo (100 XP)
    /// — ngưỡng hợp lý cho user thật, farmer bị chặn sau vài vòng.
    pub const MAX_POST_GAME_XP_PER_DAY: i32 = 4;
    pub const MAX_POST_NEWS_XP_PER_DAY: i32 = 4;
    pub const MAX_REVIEW_XP_PER_DAY: i32 = 6;
    pub const MAX_REPO_XP_PER_DAY: i32 = 5;
}

pub struct GamificationRepo;

impl GamificationRepo {
    /// Đọc tổng XP của user (0 nếu chưa có dòng nào).
    /// v3.1.0 — total_xp: BIGINT (i64) — hỗ trợ level tới 500 tỷ.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn total_xp(pool: &PgPool, user_id: Uuid) -> AppResult<i64> {
        let xp: Option<i64> =
            sqlx::query_scalar("SELECT total_xp FROM user_xp_totals WHERE user_id = $1")
                .bind(user_id)
                .fetch_optional(pool)
                .await?;
        Ok(xp.unwrap_or(0))
    }

    /// Cấp độ hiện tại của user.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn level_of(pool: &PgPool, user_id: Uuid) -> AppResult<LevelInfo> {
        Ok(level_from_xp(Self::total_xp(pool, user_id).await?))
    }

    /// Hệ số XP boost đang active (1 hoặc 2 — mua XP Boost ở cửa hàng).
    /// # Errors
    /// Trả lỗi khi DB fail.
    async fn xp_multiplier(pool: &PgPool, user_id: Uuid) -> AppResult<i32> {
        let active = crate::repositories::ShopRepo::xp_boost_active(pool, user_id).await?;
        Ok(if active { 2 } else { 1 })
    }

    /// Cộng XP + ghi log. Trả về (tổng XP mới BIGINT, XP thực cộng, LevelInfo mới).
    /// `xp_effective` = 0 khi bị chạm trần anti-farm — caller (service) dùng
    /// giá trị này để tính level TRƯỚC khi cộng, tránh báo "Lên cấp" ảo
    /// khi tổng không đổi (v3.0.0 FIX).
    /// v3.1.0 — total_xp: BIGINT (i64) — `amount` vào vẫn i32 (per-event
    /// luôn nhỏ — max ~300 XP/event), nhưng tổng trả ra i64 để caller có
    /// thể render LevelInfo với xp_i64.
    /// Không thông báo ở tầng repo — caller quyết định (tránh lặp
    /// notification khi award_xp được gọi từ nhiều ngữ cảnh).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn award_xp(
        pool: &PgPool,
        user_id: Uuid,
        reason: &str,
        amount: i32,
    ) -> AppResult<(i64, i32, LevelInfo)> {
        let mut tx = pool.begin().await?;
        // Anti-farm cap: đếm số event cùng reason đã có hôm nay
        // v3.12.0 — cap hoist lên ngoài block để tái dùng ở lock re-check
        // phía dưới (trước đây nằm trong block if amount > 0).
        let cap = if amount > 0 {
            match reason {
                "comment" => Some(xp::MAX_COMMENT_XP_PER_DAY),
                "chat_message" => Some(xp::MAX_CHAT_XP_PER_DAY),
                "received_like" => Some(xp::MAX_RECEIVED_LIKE_XP_PER_DAY),
                "received_download" => Some(xp::MAX_RECEIVED_DOWNLOAD_XP_PER_DAY),
                "received_follow" => Some(xp::MAX_RECEIVED_FOLLOW_XP_PER_DAY),
                // v3.5.1 — cap các reason tạo-nội-dùng (xoá-rồi-làm-lại farm)
                "post_game" => Some(xp::MAX_POST_GAME_XP_PER_DAY),
                "post_news" => Some(xp::MAX_POST_NEWS_XP_PER_DAY),
                "review" => Some(xp::MAX_REVIEW_XP_PER_DAY),
                "repo" => Some(xp::MAX_REPO_XP_PER_DAY),
                _ => None,
            }
        } else {
            None
        };
        let effective = if amount > 0 {
            match cap {
                Some(cap) => {
                    // SQL động: chỉ nhét hằng SQL_TODAY_START_VN (không có
                    // input user) — AssertSqlSafe đánh dấu đã audit.
                    let sql = format!(
                        r"SELECT COUNT(*) FROM xp_events
                          WHERE user_id = $1 AND reason = $2
                            AND created_at >= {}",
                        crate::utils::SQL_TODAY_START_VN
                    );
                    let today_count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
                        .bind(user_id)
                        .bind(reason)
                        .fetch_one(&mut *tx)
                        .await?;
                    if today_count >= i64::from(cap) {
                        0 // đã chạm trần ngày — không cộng thêm
                    } else {
                        amount
                    }
                }
                None => amount,
            }
        } else {
            amount
        };
        // v3.12.0 (audit logic M4): anti-farm cap là check-then-act — 2
        // request song song cùng reason (burst comment/chat) đều thấy
        // today_count < cap rồi cùng INSERT → cap ngày bị vượt. Lớp chống
        // farm cùng pattern (trivia/mystery box) đã dùng
        // pg_advisory_xact_lock; lấy lock THEO (user, reason) NGAY SAU khi
        // biết có cap, TRƯỚC COUNT → mọi request dồn hàng xoay vòng, mỗi
        // lần re-COUNT thấy số mới nhất. Lock chỉ tồn tại trong tx này
        // (xact-scoped tự nhả khi commit/rollback) — không deadlock vì
        // single-lock ordering.
        // LƯU Ý: chỉ áp khi effective có thể > 0 (có cap); không-cap path
        // (checkin, shop_spend...) giữ nguyên hành vi để không thêm
        // contention cho event không farm-able.
        let effective = if cap.is_some() && effective > 0 {
            // SQL tĩnh — không format! cần thiết (clippy::useless_format).
            let lock_sql =
                "SELECT pg_advisory_xact_lock(hashtext('xp_cap:' || $1::text || ':' || $2::text));";
            sqlx::query(lock_sql)
                .bind(user_id.to_string())
                .bind(reason)
                .execute(&mut *tx)
                .await?;
            // Re-count sau khi có lock — thấy cả event do request song song
            // vừa commit (READ COMMITTED thấy snapshot mới mỗi statement).
            let sql = format!(
                r"SELECT COUNT(*) FROM xp_events
                  WHERE user_id = $1 AND reason = $2
                    AND created_at >= {}",
                crate::utils::SQL_TODAY_START_VN
            );
            let today_count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(user_id)
                .bind(reason)
                .fetch_one(&mut *tx)
                .await?;
            if today_count >= i64::from(cap.unwrap_or(0)) {
                0
            } else {
                effective
            }
        } else {
            effective
        };
        // v3.0.0 — XP Boost (cửa hàng): nhân đôi XP thực cộng (sau cap).
        // Boost 0→0 vẫn 0, không tạo XP từ hư không.
        let effective = if effective > 0 {
            effective * Self::xp_multiplier(pool, user_id).await? // (đọc ngoài tx — best-effort, lệch 1 request chấp nhận được)
        } else {
            0
        };
        // Log event — CHỈ khi có XP thực (v3.0.0: amount=0 không được
        // activity feed đọc (bộ lọc amount>0) nên không ghi nữa, tránh
        // xp_events phình vô hạn do chat/like spam).
        if effective > 0 {
            sqlx::query("INSERT INTO xp_events (user_id, reason, amount) VALUES ($1, $2, $3)")
                .bind(user_id)
                .bind(reason)
                .bind(effective)
                .execute(&mut *tx)
                .await?;
        }
        // Upsert tổng — v3.1.0: total_xp BIGINT, bind i32 (sqlx tự cast).
        // Trả i64 để level_from_xp(i64) áp dụng được công thức 500 tỷ.
        let total: i64 = sqlx::query_scalar(
            r"INSERT INTO user_xp_totals (user_id, total_xp)
               VALUES ($1, $2)
               ON CONFLICT (user_id)
               DO UPDATE SET total_xp = user_xp_totals.total_xp + $2,
                             updated_at = NOW()
               RETURNING total_xp",
        )
        .bind(user_id)
        .bind(effective)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok((total, effective, level_from_xp(total)))
    }

    /// Tiêu 1 Streak Freeze cho ngày HÔM QUA nếu đủ điều kiện.
    /// Trả Some(streak của hôm-trước-hôm-qua) khi đã tiêu, None khi không.
    /// Gọi trong tx của do_checkin — nhất quán, chống double-consume bằng
    /// PK (user_id, freeze_date).
    /// # Errors
    /// Trả lỗi khi DB fail.
    async fn maybe_consume_streak_freeze(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user_id: Uuid,
    ) -> AppResult<Option<i32>> {
        // Hôm-trước-hôm-qua có checkin không? (nếu không — chuỗi đã đứt
        // trước đó, không có gì để bảo vệ)
        let sql = format!(
            "SELECT streak FROM daily_checkins
             WHERE user_id = $1 AND checkin_date = {} - 2",
            crate::utils::SQL_TODAY_VN
        );
        let Some(prev_streak): Option<i32> = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .fetch_optional(&mut **tx)
            .await?
        else {
            return Ok(None);
        };
        // Đã dùng freeze cho hôm qua chưa? (chỉ 1 lần duy nhất)
        let sql = format!(
            "SELECT 1 FROM streak_freeze_usage
             WHERE user_id = $1 AND freeze_date = {} - 1",
            crate::utils::SQL_TODAY_VN
        );
        let already: Option<i32> = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .fetch_optional(&mut **tx)
            .await?;
        if already.is_some() {
            return Ok(None);
        }
        // Còn freeze trong kho không? (FOR UPDATE chống race tiêu đôi)
        let qty: Option<i32> = sqlx::query_scalar(
            r"SELECT quantity FROM user_inventory
              WHERE user_id = $1 AND item_id = 'streak_freeze' FOR UPDATE",
        )
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?;
        let Some(qty) = qty else {
            return Ok(None);
        };
        if qty <= 0 {
            return Ok(None);
        }
        if qty == 1 {
            sqlx::query(
                "DELETE FROM user_inventory
                 WHERE user_id = $1 AND item_id = 'streak_freeze'",
            )
            .bind(user_id)
            .execute(&mut **tx)
            .await?;
        } else {
            sqlx::query(
                "UPDATE user_inventory SET quantity = quantity - 1, updated_at = NOW()
                 WHERE user_id = $1 AND item_id = 'streak_freeze'",
            )
            .bind(user_id)
            .execute(&mut **tx)
            .await?;
        }
        // Ghi usage — PK chặn double trong race
        let sql = format!(
            "INSERT INTO streak_freeze_usage (user_id, freeze_date)
             VALUES ($1, {} - 1)
             ON CONFLICT (user_id, freeze_date) DO NOTHING",
            crate::utils::SQL_TODAY_VN
        );
        let res = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .execute(&mut **tx)
            .await?;
        if res.rows_affected() == 0 {
            return Ok(None);
        }
        tracing::info!(user = %user_id, "Streak Freeze đã tự kích hoạt cho ngày hôm qua");
        Ok(Some(prev_streak))
    }

    /// Bảng xếp hạng XP theo THÁNG hiện tại (season board) từ xp_events.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn season_leaderboard(
        pool: &PgPool,
        limit: i64,
    ) -> AppResult<Vec<crate::models::retention::SeasonEntry>> {
        let rows = sqlx::query_as::<_, crate::models::retention::SeasonEntry>(
            r"SELECT u.username, u.display_name, u.avatar_url,
                      COALESCE(SUM(e.amount), 0)::bigint AS period_xp
               FROM xp_events e
               JOIN users u ON u.id = e.user_id
               WHERE e.amount > 0
                 AND e.created_at >= date_trunc('month',
                      NOW() AT TIME ZONE 'Asia/Ho_Chi_Minh')
                     AT TIME ZONE 'Asia/Ho_Chi_Minh'
                 AND u.is_banned = FALSE AND u.role <> 'ai_agent'
               GROUP BY u.id, u.username, u.display_name, u.avatar_url
               ORDER BY period_xp DESC, u.created_at ASC
               LIMIT $1",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Bảng xếp hạng XP theo TUẦN hiện tại (thứ 2 → nay, giờ VN).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn weekly_leaderboard(
        pool: &PgPool,
        limit: i64,
    ) -> AppResult<Vec<crate::models::retention::SeasonEntry>> {
        let rows = sqlx::query_as::<_, crate::models::retention::SeasonEntry>(
            r"SELECT u.username, u.display_name, u.avatar_url,
                      COALESCE(SUM(e.amount), 0)::bigint AS period_xp
               FROM xp_events e
               JOIN users u ON u.id = e.user_id
               WHERE e.amount > 0
                 AND e.created_at >= date_trunc('week',
                      NOW() AT TIME ZONE 'Asia/Ho_Chi_Minh')
                     AT TIME ZONE 'Asia/Ho_Chi_Minh'
                 AND u.is_banned = FALSE AND u.role <> 'ai_agent'
               GROUP BY u.id, u.username, u.display_name, u.avatar_url
               ORDER BY period_xp DESC, u.created_at ASC
               LIMIT $1",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Trạng thái điểm danh hôm nay của user.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn today_checkin(pool: &PgPool, user_id: Uuid) -> AppResult<Option<DailyCheckin>> {
        let sql = format!(
            "SELECT user_id, checkin_date, streak, xp_awarded, created_at
             FROM daily_checkins WHERE user_id = $1 AND checkin_date = {}",
            crate::utils::SQL_TODAY_VN
        );
        let row = sqlx::query_as::<_, DailyCheckin>(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
        Ok(row)
    }

    /// Điểm danh hôm nay (idempotent — đã điểm rồi thì trả về streak cũ).
    /// Trả về (streak, xp_awarded, level_after).
    /// XP = DAILY_CHECKIN + min(streak-1, MAX_STREAK_BONUS) thưởng chuỗi.
    ///
    /// v2.9.1 FIX 2 lỗi:
    /// 1. CONTRACT: đã điểm từ trước → trả `xp_awarded = 0` (handler dùng
    ///    `xp_awarded == 0` để render "bạn đã điểm danh rồi" thay vì
    ///    "Điểm danh thành công! +N XP" — trước đây trả xp đã lưu (>=5)
    ///    khiến mọi re-click đều báo thành công +N XP oan).
    /// 2. RACE: 2 tab bấm cùng lúc — cả 2 thấy `existing = None`, một bên
    ///    INSERT daily_checkins thắng (PK user_id+date), bên kia DO NOTHING
    ///    nhưng VẪN cộng xp_events + user_xp_totals → XP x2 cho 1 ngày.
    ///    Giờ check `rows_affected`: INSERT no-op → trả (streak, 0) như
    ///    already-checked-in, KHÔNG đụng vào xp_events/user_xp_totals.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn do_checkin(pool: &PgPool, user_id: Uuid) -> AppResult<(i32, i32, LevelInfo)> {
        let mut tx = pool.begin().await?;
        // Đã điểm hôm nay chưa?
        let sql = format!(
            "SELECT user_id, checkin_date, streak, xp_awarded, created_at
             FROM daily_checkins WHERE user_id = $1 AND checkin_date = {}",
            crate::utils::SQL_TODAY_VN
        );
        let existing = sqlx::query_as::<_, DailyCheckin>(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
        if let Some(c) = existing {
            let level = level_from_xp(Self::total_xp(pool, user_id).await?);
            // FIX 1: xp_awarded = 0 theo contract handler (already = xp == 0)
            return Ok((c.streak, 0, level));
        }
        // Chuỗi: hôm qua có điểm không?
        let sql = format!(
            "SELECT streak FROM daily_checkins
             WHERE user_id = $1 AND checkin_date = {} - 1",
            crate::utils::SQL_TODAY_VN
        );
        let yesterday_streak: Option<i32> = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
        let streak = yesterday_streak.map_or(1, |s| s + 1);
        // v3.0.0 — STREAK FREEZE tự động: hôm qua bị lỡ (hôm trước hôm qua
        // có điểm, hôm qua trống) + user còn Streak Freeze trong kho →
        // tiêu 1 freeze để "bảo vệ" ngày hôm qua, chuỗi tiếp tục (streak
        // = streak của hôm-trước-hôm-qua + 2: 1 ngày đóng băng + hôm nay).
        // Chỉ bảo vệ đúng 1 ngày liền trước — lỡ từ 2 ngày trở lên thì
        // chuỗi đứt thật (thiết kế cố ý để giữ giá trị của streak).
        let streak = if yesterday_streak.is_none() {
            match Self::maybe_consume_streak_freeze(&mut tx, user_id).await? {
                Some(prev_streak) => prev_streak + 2,
                None => streak,
            }
        } else {
            streak
        };
        let bonus = (streak - 1).min(xp::MAX_STREAK_BONUS);
        // v3.0.0 — XP Boost x2 áp dụng cả điểm danh (nhất quán toàn hệ)
        let mult = Self::xp_multiplier(pool, user_id).await?;
        let xp_awarded = (xp::DAILY_CHECKIN + bonus) * mult;
        // FIX 2 (race): DO NOTHING giữa 2 request song song — phải kiểm tra
        // rows_affected trước khi ghi xp, không thì bên thua race vẫn cộng XP.
        let sql = format!(
            r"INSERT INTO daily_checkins (user_id, checkin_date, streak, xp_awarded)
               VALUES ($1, {}, $2, $3)
               ON CONFLICT (user_id, checkin_date) DO NOTHING",
            crate::utils::SQL_TODAY_VN
        );
        let insert_result = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .bind(streak)
            .bind(xp_awarded)
            .execute(&mut *tx)
            .await?;
        if insert_result.rows_affected() == 0 {
            // Request song song khác vừa chiếm slot điểm danh hôm nay.
            // Rollback tx (không ghi gì), trả về state của bản ghi thắng.
            tx.rollback().await?;
            // v3.0.0 FIX: trước đây SELECT lại bằng CURRENT_DATE (timezone
            // server) trong khi insert ghi theo ngày VN — server chạy UTC
            // thì khung 17:00–24:00 UTC fetch_one không thấy row →
            // RowNotFound → double-click thứ hai nhận 400 vô nghĩa.
            let sql = format!(
                "SELECT user_id, checkin_date, streak, xp_awarded, created_at
                 FROM daily_checkins WHERE user_id = $1 AND checkin_date = {}",
                crate::utils::SQL_TODAY_VN
            );
            let winner = sqlx::query_as::<_, DailyCheckin>(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(user_id)
                .fetch_one(pool)
                .await?;
            let level = level_from_xp(Self::total_xp(pool, user_id).await?);
            return Ok((winner.streak, 0, level));
        }
        // XP qua cùng tx để đảm bảo nhất quán
        sqlx::query(
            "INSERT INTO xp_events (user_id, reason, amount) VALUES ($1, 'daily_checkin', $2)",
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
        Ok((streak, xp_awarded, level_from_xp(total)))
    }

    /// Chuỗi hiện tại (dựa trên checkin gần nhất — nếu là hôm qua thì
    /// coi như "đang giữ chuỗi, hôm nay chưa điểm", nếu cũ hơn → đã đứt).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn current_streak(pool: &PgPool, user_id: Uuid) -> AppResult<i32> {
        let row: Option<(chrono::NaiveDate, i32)> = sqlx::query_as(
            "SELECT checkin_date, streak FROM daily_checkins
             WHERE user_id = $1 ORDER BY checkin_date DESC LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        Ok(row.map_or(0, |(d, s)| {
            // v2.9.2 FIX: “hôm nay” theo giờ VN (UTC+7) thay vì UTC —
            // trước đây trong khung 17:00–24:00 UTC (00:00–07:00 VN) so
            // sai ngày với checkin_date (ghi theo giờ VN) → streak hiển
            // thị “giữ” nhầm khi thực tế đã đứt (hoặc ngược lại).
            let today = crate::utils::today_vn();
            let yesterday = today - chrono::Duration::days(1);
            if d >= yesterday {
                s
            } else {
                0 // chuỗi đã đứt
            }
        }))
    }

    /// Toàn bộ catalog huy hiệu (theo sort_order).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn list_achievements(pool: &PgPool) -> AppResult<Vec<Achievement>> {
        let rows = sqlx::query_as::<_, Achievement>(
            "SELECT id, title, description, icon, xp_reward, category, sort_order, is_active
             FROM achievements WHERE is_active = TRUE ORDER BY sort_order ASC",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Catalog + trạng thái của 1 user (earned_at, showcase).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn achievements_with_status(
        pool: &PgPool,
        user_id: Uuid,
    ) -> AppResult<Vec<AchievementWithStatus>> {
        let rows: Vec<crate::models::gamification::AchievementStatusRow> = sqlx::query_as(
            r"SELECT a.id, a.title, a.description, a.icon, a.xp_reward,
                      a.category, a.sort_order, a.is_active,
                      ua.earned_at, ua.is_showcased
               FROM achievements a
               LEFT JOIN user_achievements ua
                 ON ua.achievement_id = a.id AND ua.user_id = $1
               WHERE a.is_active = TRUE
               ORDER BY (ua.earned_at IS NULL), a.sort_order ASC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.into_status()).collect())
    }

    /// Gắn huy hiệu cho user nếu chưa có. Trả về catalog đầy đủ nếu
    /// MỚI gắn (None nếu đã có từ trước).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn grant_achievement(
        pool: &PgPool,
        user_id: Uuid,
        achievement_id: &str,
    ) -> AppResult<Option<Achievement>> {
        let res = sqlx::query(
            r"INSERT INTO user_achievements (user_id, achievement_id)
               VALUES ($1, $2)
               ON CONFLICT (user_id, achievement_id) DO NOTHING",
        )
        .bind(user_id)
        .bind(achievement_id)
        .execute(pool)
        .await?;
        if res.rows_affected() == 0 {
            return Ok(None);
        }
        let a = sqlx::query_as::<_, Achievement>(
            "SELECT id, title, description, icon, xp_reward, category, sort_order, is_active
             FROM achievements WHERE id = $1",
        )
        .bind(achievement_id)
        .fetch_optional(pool)
        .await?;
        Ok(a)
    }

    /// v3.10.0 — user đã sở hữu 1 huy hiệu cụ thể? (admin check trước khi
    /// render nút Cấp/Thu hồi huy hiệu độc quyền AI Agent.)
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn has_achievement(
        pool: &PgPool,
        user_id: Uuid,
        achievement_id: &str,
    ) -> AppResult<bool> {
        let owned: Option<bool> = sqlx::query_scalar(
            "SELECT TRUE FROM user_achievements
             WHERE user_id = $1 AND achievement_id = $2",
        )
        .bind(user_id)
        .bind(achievement_id)
        .fetch_optional(pool)
        .await?;
        Ok(owned.unwrap_or(false))
    }

    /// v3.10.0 — THU HỒI 1 huy hiệu đã trao (chỉ dùng cho huy hiệu
    /// admin-cấp như `ai_agent_core`; huy hiệu hành vi sẽ được engine
    /// trao lại ngay lần check sau nên thu hồi vô nghĩa).
    /// Trả về TRUE nếu thực sự xoá 1 row (FALSE = user chưa có).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn revoke_achievement(
        pool: &PgPool,
        user_id: Uuid,
        achievement_id: &str,
    ) -> AppResult<bool> {
        let res = sqlx::query(
            "DELETE FROM user_achievements
             WHERE user_id = $1 AND achievement_id = $2",
        )
        .bind(user_id)
        .bind(achievement_id)
        .execute(pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    /// Ghim/bỏ ghim huy hiệu showcase (giới hạn MAX_SHOWCASED_ACHIEVEMENTS).
    /// # Errors
    /// Trả lỗi khi DB fail hoặc user cố ghim quá giới hạn.
    pub async fn toggle_showcase(
        pool: &PgPool,
        user_id: Uuid,
        achievement_id: &str,
    ) -> AppResult<bool> {
        let mut tx = pool.begin().await?;
        let current: Option<bool> = sqlx::query_scalar::<_, bool>(
            "SELECT is_showcased FROM user_achievements
                 WHERE user_id = $1 AND achievement_id = $2",
        )
        .bind(user_id)
        .bind(achievement_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(current) = current else {
            return Err(AppError::BadRequest("Bạn chưa sở hữu huy hiệu này".into()));
        };
        if current {
            // Bỏ ghim — luôn được
            sqlx::query(
                "UPDATE user_achievements SET is_showcased = FALSE
                 WHERE user_id = $1 AND achievement_id = $2",
            )
            .bind(user_id)
            .bind(achievement_id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            return Ok(false);
        }
        // Ghim — check quota
        // v3.12.0 (audit logic L2): COUNT rồi UPDATE là check-then-act —
        // 2 tab ghim đồng thời cùng đọc count=2 rồi cùng UPDATE → vượt
        // MAX_SHOWCASED_ACHIEVEMENTS. Khoá các row showcase của user
        // (FOR UPDATE) trước khi đếm: request sau chờ request trước
        // commit rồi mới thấy count mới — quota bất biến dưới race.
        sqlx::query(
            "SELECT achievement_id FROM user_achievements
             WHERE user_id = $1 AND is_showcased = TRUE
             FOR UPDATE",
        )
        .bind(user_id)
        .fetch_all(&mut *tx)
        .await?;
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM user_achievements
             WHERE user_id = $1 AND is_showcased = TRUE",
        )
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;
        if count >= i64::from(crate::models::gamification::MAX_SHOWCASED_ACHIEVEMENTS) {
            return Err(AppError::BadRequest(format!(
                "Chỉ ghim tối đa {} huy hiệu — bỏ ghim một huy hiệu khác trước",
                crate::models::gamification::MAX_SHOWCASED_ACHIEVEMENTS
            )));
        }
        sqlx::query(
            "UPDATE user_achievements SET is_showcased = TRUE
             WHERE user_id = $1 AND achievement_id = $2",
        )
        .bind(user_id)
        .bind(achievement_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Huy hiệu đã đạt của user (mới nhất trước) — cho profile + showcase.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn user_achievements(
        pool: &PgPool,
        user_id: Uuid,
    ) -> AppResult<Vec<(Achievement, chrono::DateTime<Utc>, bool)>> {
        let rows: Vec<crate::models::gamification::UserAchievementRow> = sqlx::query_as(
            r"SELECT a.id, a.title, a.description, a.icon, a.xp_reward,
                      a.category, a.sort_order, a.is_active,
                      ua.earned_at, ua.is_showcased
               FROM user_achievements ua
               JOIN achievements a ON a.id = ua.achievement_id
               WHERE ua.user_id = $1
               ORDER BY ua.is_showcased DESC, ua.earned_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.split()).collect())
    }

    /// Kiểm tra + trao TẤT CẢ huy hiệu đạt điều kiện cho user.
    /// Trả về danh sách huy hiệu MỚI trao (để caller gửi notification
    /// + cộng XP thưởng).
    ///
    /// Toàn bộ điều kiện được tính bằng 1 query UNION-style duy nhất
    /// (không N+1) rồi so khớp catalog trong Rust.
    ///
    /// v3.1.0 FIX (bug tự động cấp danh hiệu): trước đây hàm `met()`
    /// dùng match với 25 ID cố định. Mọi danh hiệu mới seed vào
    /// `achievements` catalog có ID lạ → `match _ => false` → KHÔNG BAO
    /// GIỜ được trao dù user đã đạt điều kiện. Giờ mở rộng `met()` cho
    /// 100 danh hiệu mới (migration 024).
    ///
    /// v3.8.0 FIX (bug "đạt điều kiện nhưng huy hiệu vĩnh viễn không được
    /// cấp"): thống kê rps_wins / word_chain_valid đã xoá cùng 2 game mode
    /// (migration 037 drop bảng rps_plays / word_chain_plays — nếu giữ
    /// subselect vào bảng đã drop thì TOÀN BỘ query stats fail →
    /// check_and_award trả Err → KHÔNG huy hiệu nào được trao cho ai,
    /// vĩnh viễn, chỉ thấy warn log).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn check_and_award(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<Achievement>> {
        #[derive(sqlx::FromRow)]
        struct Stats {
            games_published: i64,
            comments_count: i64,
            likes_given: i64,
            bookmarks_count: i64,
            reviews_count: i64,
            followers_count: i64,
            downloads_total: i64,
            likes_received_total: i64,
            news_published: i64,
            has_social: i64,
            has_avatar: i64,
            has_bio: i64,
            chat_messages: i64,
            repos_count: i64,
            max_streak: i64,
            total_xp: i64,
            // v3.1.0 — new stats for 100 added achievements
            social_links_count: i64,
            collections_count: i64,
            total_checkins: i64,
        }
        let s = sqlx::query_as::<_, Stats>(
            r#"SELECT
                (SELECT COUNT(*) FROM games WHERE user_id = $1 AND status = 'published') AS games_published,
                (SELECT COUNT(*) FROM comments WHERE user_id = $1)
                  + (SELECT COUNT(*) FROM news_comments WHERE user_id = $1) AS comments_count,
                (SELECT COUNT(*) FROM likes WHERE user_id = $1) AS likes_given,
                (SELECT COUNT(*) FROM bookmarks WHERE user_id = $1) AS bookmarks_count,
                (SELECT COUNT(*) FROM reviews WHERE user_id = $1) AS reviews_count,
                (SELECT COUNT(*) FROM follows WHERE followee_id = $1) AS followers_count,
                (SELECT COALESCE(SUM(download_count), 0) FROM games WHERE user_id = $1) AS downloads_total,
                (SELECT COALESCE(SUM(like_count), 0) FROM games WHERE user_id = $1) AS likes_received_total,
                (SELECT COUNT(*) FROM news WHERE user_id = $1 AND status = 'published') AS news_published,
                (SELECT COUNT(*) FROM user_social_links
                  WHERE user_id = $1 AND links::text NOT IN ('{}', 'null')) AS has_social,
                (SELECT COUNT(*) FROM users
                  WHERE id = $1 AND avatar_url IS NOT NULL AND avatar_url <> '') AS has_avatar,
                (SELECT COUNT(*) FROM users
                  WHERE id = $1 AND bio IS NOT NULL AND length(bio) > 0) AS has_bio,
                (SELECT COUNT(*) FROM chat_messages WHERE user_id = $1 AND is_deleted = FALSE) AS chat_messages,
                (SELECT COUNT(*) FROM github_repos WHERE user_id = $1) AS repos_count,
                (SELECT COALESCE(MAX(streak), 0)::bigint FROM daily_checkins WHERE user_id = $1) AS max_streak,
                -- v3.8.0 FIX (bug "huy hiệu vĩnh viễn không được cấp"):
                -- COALESCE phải nằm NGOÀI subselect. Bản cũ
                -- (SELECT COALESCE(total_xp,0) FROM user_xp_totals WHERE ...)
                -- trả 0 DÒNG khi user chưa có row (chưa từng được cộng XP)
                -- → subselect = NULL → sqlx decode i64 fail →
                -- check_and_award trả Err → KHÔNG huy hiệu nào được trao,
                -- vĩnh viễn, im lặng (chỉ warn log). Người dùng like game
                -- (điều kiện first_like_given đạt) mà không hành vi nào
                -- tạo user_xp_totals → badge không bao giờ đến tay.
                COALESCE((SELECT total_xp FROM user_xp_totals WHERE user_id = $1), 0)::bigint AS total_xp,
                -- v3.1.0: count of social link platforms. links is a JSON object
                -- (platform_id -> url map) — count keys via LATERAL jsonb_object_keys.
                -- COALESCE outside subquery returns 0 if user has no row.
                COALESCE((
                  SELECT COUNT(*) FROM user_social_links usl
                  CROSS JOIN LATERAL jsonb_object_keys(usl.links) AS k
                  WHERE usl.user_id = $1
                ), 0) AS social_links_count,
                -- v3.1.0: count of user collections (for collections_X tiers)
                (SELECT COUNT(*) FROM collections WHERE user_id = $1) AS collections_count,
                -- v3.8.0: rps_wins / word_chain_valid subselects removed
                -- (2 game modes deleted; migration 037 drops rps_plays /
                -- word_chain_plays; keeping these subselects would break
                -- the WHOLE stats query on every check_and_award call).
                -- v3.1.0: total checkin rows (for streak_champion — total 365 days)
                (SELECT COUNT(*) FROM daily_checkins WHERE user_id = $1) AS total_checkins"#,
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        // v3.1.0 — level_from_xp giờ nhận i64 (BIGINT) + cap 500 tỷ.
        let level = level_from_xp(s.total_xp).level;
        let met = |id: &str| -> bool {
            match id {
                // === Original 25 (seed migration 021) ===
                "first_login" => true, // đăng nhập rồi mới chạy được hàm này
                "profile_avatar" => s.has_avatar > 0,
                "profile_bio" => s.has_bio > 0,
                "social_link" => s.has_social > 0,
                "first_comment" => s.comments_count >= 1,
                "comments_10" => s.comments_count >= 10,
                "comments_50" => s.comments_count >= 50,
                "first_review" => s.reviews_count >= 1,
                "first_game" => s.games_published >= 1,
                "games_5" => s.games_published >= 5,
                "repo_first" => s.repos_count >= 1,
                "news_first" => s.news_published >= 1,
                "likes_received_50" => s.likes_received_total >= 50,
                "downloads_100" => s.downloads_total >= 100,
                "first_like_given" => s.likes_given >= 1,
                "first_bookmark" => s.bookmarks_count >= 1,
                "bookmarks_10" => s.bookmarks_count >= 10,
                "first_follower" => s.followers_count >= 1,
                "followers_10" => s.followers_count >= 10,
                "chat_first" => s.chat_messages >= 1,
                "streak_3" => s.max_streak >= 3,
                "streak_7" => s.max_streak >= 7,
                "streak_30" => s.max_streak >= 30,
                "level_5" => level >= 5,
                "level_10" => level >= 10,
                // === v3.1.0 — 100 NEW (migration 024) ===
                // -- LEVEL tiers (20) — level là i64, compare với i64 literal
                "level_15" => level >= 15,
                "level_20" => level >= 20,
                "level_25" => level >= 25,
                "level_30" => level >= 30,
                "level_40" => level >= 40,
                "level_50" => level >= 50,
                "level_75" => level >= 75,
                "level_100" => level >= 100,
                "level_150" => level >= 150,
                "level_200" => level >= 200,
                "level_300" => level >= 300,
                "level_500" => level >= 500,
                "level_750" => level >= 750,
                "level_1000" => level >= 1_000,
                "level_2000" => level >= 2_000,
                "level_5000" => level >= 5_000,
                "level_10000" => level >= 10_000,
                "level_100000" => level >= 100_000,
                "level_1m" => level >= 1_000_000,
                "level_max" => level >= crate::models::gamification::MAX_LEVEL,
                // -- STREAK tiers (5)
                "streak_50" => s.max_streak >= 50,
                "streak_100" => s.max_streak >= 100,
                "streak_365" => s.max_streak >= 365,
                "streak_1000" => s.max_streak >= 1_000,
                "streak_champion" => s.total_checkins >= 365,
                // -- COMMENTS tiers (5)
                "comments_100" => s.comments_count >= 100,
                "comments_250" => s.comments_count >= 250,
                "comments_500" => s.comments_count >= 500,
                "comments_1000" => s.comments_count >= 1_000,
                "comments_5000" => s.comments_count >= 5_000,
                // -- GAMES PUBLISHED tiers (10)
                "games_10" => s.games_published >= 10,
                "games_25" => s.games_published >= 25,
                "games_50" => s.games_published >= 50,
                "games_100" => s.games_published >= 100,
                "games_250" => s.games_published >= 250,
                "games_500" => s.games_published >= 500,
                "games_1000" => s.games_published >= 1_000,
                "games_2500" => s.games_published >= 2_500,
                "games_5000" => s.games_published >= 5_000,
                "games_10000" => s.games_published >= 10_000,
                // -- LIKES RECEIVED tiers (5)
                "likes_received_100" => s.likes_received_total >= 100,
                "likes_received_250" => s.likes_received_total >= 250,
                "likes_received_500" => s.likes_received_total >= 500,
                "likes_received_1000" => s.likes_received_total >= 1_000,
                "likes_received_5000" => s.likes_received_total >= 5_000,
                // -- DOWNLOADS tiers (5)
                "downloads_250" => s.downloads_total >= 250,
                "downloads_500" => s.downloads_total >= 500,
                "downloads_1000" => s.downloads_total >= 1_000,
                "downloads_5000" => s.downloads_total >= 5_000,
                "downloads_10000" => s.downloads_total >= 10_000,
                // -- FOLLOWERS tiers (5)
                "followers_50" => s.followers_count >= 50,
                "followers_100" => s.followers_count >= 100,
                "followers_250" => s.followers_count >= 250,
                "followers_500" => s.followers_count >= 500,
                "followers_1000" => s.followers_count >= 1_000,
                // -- REVIEWS tiers (5)
                "reviews_5" => s.reviews_count >= 5,
                "reviews_10" => s.reviews_count >= 10,
                "reviews_25" => s.reviews_count >= 25,
                "reviews_50" => s.reviews_count >= 50,
                "reviews_100" => s.reviews_count >= 100,
                // -- BOOKMARKS tiers (5)
                "bookmarks_25" => s.bookmarks_count >= 25,
                "bookmarks_50" => s.bookmarks_count >= 50,
                "bookmarks_100" => s.bookmarks_count >= 100,
                "bookmarks_250" => s.bookmarks_count >= 250,
                "bookmarks_500" => s.bookmarks_count >= 500,
                // -- REPOS tiers (5)
                "repos_5" => s.repos_count >= 5,
                "repos_10" => s.repos_count >= 10,
                "repos_25" => s.repos_count >= 25,
                "repos_50" => s.repos_count >= 50,
                "repos_100" => s.repos_count >= 100,
                // -- NEWS tiers (5)
                "news_5" => s.news_published >= 5,
                "news_10" => s.news_published >= 10,
                "news_25" => s.news_published >= 25,
                "news_50" => s.news_published >= 50,
                "news_100" => s.news_published >= 100,
                // -- CHAT tiers (5)
                "chat_10" => s.chat_messages >= 10,
                "chat_50" => s.chat_messages >= 50,
                "chat_100" => s.chat_messages >= 100,
                "chat_500" => s.chat_messages >= 500,
                "chat_1000" => s.chat_messages >= 1_000,
                // -- COLLECTIONS tiers (5)
                "collections_3" => s.collections_count >= 3,
                "collections_5" => s.collections_count >= 5,
                "collections_10" => s.collections_count >= 10,
                "collections_25" => s.collections_count >= 25,
                "collections_50" => s.collections_count >= 50,
                // -- SOCIAL LINKS tiers (5) — count of platforms
                "social_2" => s.social_links_count >= 2,
                "social_3" => s.social_links_count >= 3,
                "social_4" => s.social_links_count >= 4,
                "social_5" => s.social_links_count >= 5,
                "social_master" => s.social_links_count >= 7,
                // v3.8.0 — rps_* / word_chain_* achievements deleted together
                // with the two game modes (migration 037 removes catalog
                // rows, so met() will never see these IDs again).
                // v3.2.0 — Fallback GENERIC cho huy hiệu cấp độ mới dạng
                // `level_N` (N là số — migration 027 seed thêm ~45 ngưỡng
                // mịn: level_2, level_3, ..., level_1500, ...). Không cần
                // thêm arm tường minh cho từng ID nữa — pattern
                // `id.strip_prefix("level_")` parse N rồi so sánh.
                // Đặt SAU tất cả arm tường minh (kể cả level_1m / level_max
                // có hậu tố chữ) và TRƯỚC `_ => false`.
                _ if id
                    .strip_prefix("level_")
                    .and_then(|n| n.parse::<i64>().ok())
                    .is_some_and(|n| level >= n) =>
                {
                    true
                }
                // (Future IDs từ migration sau → return false — rõ ràng chưa hỗ trợ)
                _ => false,
            }
        };

        let catalog = Self::list_achievements(pool).await?;
        // v3.12.0 (audit logic M3 — N+1 grant): trước đây loop gọi
        // grant_achievement() TỪNG huy hiệu đạt điều kiện — mỗi lần = 1
        // INSERT + 1 SELECT catalog (2 query × ~130 id) chạy trên MỌI
        // comment/chat/login/like của user có nhiều huy hiệu. Giờ đúng
        // 1 query duy nhất: batch INSERT ... SELECT ... WHERE id = ANY(...)
        // ON CONFLICT DO NOTHING RETURNING achievement_id (RETURNING chỉ
        // trả row MỚI — semantics trùng khớp rows_affected cũ). Dữ liệu
        // Achievement lấy từ catalog đã load sẵn trong bộ nhớ, không
        // query lại. Met-check thuần in-memory nên không mất gì.
        let met_ids: Vec<String> = catalog
            .iter()
            .filter(|a| met(&a.id))
            .map(|a| a.id.clone())
            .collect();
        if met_ids.is_empty() {
            return Ok(Vec::new());
        }
        let newly_ids: Vec<String> = sqlx::query_scalar(
            r"INSERT INTO user_achievements (user_id, achievement_id)
              SELECT $1, id FROM achievements WHERE id = ANY($2)
              ON CONFLICT (user_id, achievement_id) DO NOTHING
              RETURNING achievement_id",
        )
        .bind(user_id)
        .bind(&met_ids)
        .fetch_all(pool)
        .await?;
        Ok(catalog
            .iter()
            .filter(|a| newly_ids.iter().any(|id| id == &a.id))
            .cloned()
            .collect())
    }

    /// Bảng xếp hạng top XP (kèm số game + chuỗi hiện tại).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn leaderboard_top_xp(pool: &PgPool, limit: i64) -> AppResult<Vec<LeaderboardEntry>> {
        // v3.0.0 FIX: streak lấy từ checkin gần nhất KHÔNG có điều kiện
        // thời gian → user đứt chuỗi từ 2 tháng trước vẫn hiện "🔥 30".
        // Giờ chỉ nhận streak khi checkin gần nhất là hôm nay hoặc hôm qua
        // (đúng quy tắc current_streak) — cũ hơn → 0.
        let sql = format!(
            r"SELECT u.username, u.display_name, u.avatar_url,
                      x.total_xp,
                      (SELECT COUNT(*) FROM games g
                        WHERE g.user_id = u.id AND g.status = 'published') AS games_count,
                      COALESCE((SELECT c.streak FROM daily_checkins c
                        WHERE c.user_id = u.id
                          AND c.checkin_date >= {} - 1
                        ORDER BY c.checkin_date DESC LIMIT 1), 0)::bigint AS streak
               FROM user_xp_totals x
               JOIN users u ON u.id = x.user_id
               WHERE u.is_banned = FALSE AND u.role <> 'ai_agent'
               ORDER BY x.total_xp DESC, u.created_at ASC
               LIMIT $1",
            crate::utils::SQL_TODAY_VN
        );
        let rows = sqlx::query_as::<_, LeaderboardEntry>(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(limit)
            .fetch_all(pool)
            .await?;
        Ok(rows)
    }

    /// Activity feed hồ sơ — từ xp_events (bỏ event amount=0 tránh nhiễu).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn recent_activity(
        pool: &PgPool,
        user_id: Uuid,
        limit: i64,
    ) -> AppResult<Vec<ActivityEvent>> {
        let rows = sqlx::query_as::<_, ActivityEvent>(
            r"SELECT reason, amount, created_at FROM xp_events
               WHERE user_id = $1 AND amount > 0
               ORDER BY created_at DESC LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Số điểm danh hôm nay (admin dashboard).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn checkins_today_count(pool: &PgPool) -> AppResult<i64> {
        let sql = format!(
            "SELECT COUNT(*) FROM daily_checkins WHERE checkin_date = {}",
            crate::utils::SQL_TODAY_VN
        );
        let c: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .fetch_one(pool)
            .await?;
        Ok(c)
    }

    /// Số huy hiệu được trao hôm nay (admin dashboard).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn achievements_today_count(pool: &PgPool) -> AppResult<i64> {
        let sql = format!(
            "SELECT COUNT(*) FROM user_achievements
             WHERE earned_at >= {}",
            crate::utils::SQL_TODAY_START_VN
        );
        let c: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .fetch_one(pool)
            .await?;
        Ok(c)
    }

    /// Thống kê huy hiệu cho admin: catalog + số người đạt.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn achievement_stats(pool: &PgPool) -> AppResult<Vec<(Achievement, i64)>> {
        let rows: Vec<crate::models::gamification::AchievementStatRow> = sqlx::query_as(
            r"SELECT a.id, a.title, a.description, a.icon, a.xp_reward,
                      a.category, a.sort_order, a.is_active,
                      (SELECT COUNT(*) FROM user_achievements ua
                        WHERE ua.achievement_id = a.id) AS holders
               FROM achievements a
               WHERE a.is_active = TRUE
               ORDER BY a.sort_order ASC",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.split()).collect())
    }
}
