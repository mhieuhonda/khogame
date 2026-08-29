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
    /// dễ spam (bình luận, chat). Vượt ngưỡng → không cộng thêm nhưng
    /// vẫn ghi log hoạt động (amount = 0) để activity feed đầy đủ.
    pub const MAX_COMMENT_XP_PER_DAY: i32 = 10;
    pub const MAX_CHAT_XP_PER_DAY: i32 = 20;
    pub const MAX_RECEIVED_LIKE_XP_PER_DAY: i32 = 50;
    pub const MAX_RECEIVED_DOWNLOAD_XP_PER_DAY: i32 = 50;
}

pub struct GamificationRepo;

impl GamificationRepo {
    /// Đọc tổng XP của user (0 nếu chưa có dòng nào).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn total_xp(pool: &PgPool, user_id: Uuid) -> AppResult<i32> {
        let xp: Option<i32> =
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

    /// Cộng XP + ghi log. Trả về (tổng XP mới, LevelInfo mới).
    /// Không thông báo ở tầng repo — caller quyết định (tránh lặp
    /// notification khi award_xp được gọi từ nhiều ngữ cảnh).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn award_xp(
        pool: &PgPool,
        user_id: Uuid,
        reason: &str,
        amount: i32,
    ) -> AppResult<(i32, LevelInfo)> {
        let mut tx = pool.begin().await?;
        // Anti-farm cap: đếm số event cùng reason đã có hôm nay
        let effective = if amount > 0 {
            let cap = match reason {
                "comment" => Some(xp::MAX_COMMENT_XP_PER_DAY),
                "chat_message" => Some(xp::MAX_CHAT_XP_PER_DAY),
                "received_like" => Some(xp::MAX_RECEIVED_LIKE_XP_PER_DAY),
                "received_download" => Some(xp::MAX_RECEIVED_DOWNLOAD_XP_PER_DAY),
                _ => None,
            };
            match cap {
                Some(cap) => {
                    let today_count: i64 = sqlx::query_scalar(
                        r"SELECT COUNT(*) FROM xp_events
                          WHERE user_id = $1 AND reason = $2
                            AND created_at >= date_trunc('day', NOW())",
                    )
                    .bind(user_id)
                    .bind(reason)
                    .fetch_one(&mut *tx)
                    .await?;
                    if today_count >= i64::from(cap) {
                        0 // đã chạm trần ngày — log amount 0
                    } else {
                        amount
                    }
                }
                None => amount,
            }
        } else {
            amount
        };
        // Log event (kể cả amount=0 — activity feed vẫn có hoạt động)
        sqlx::query("INSERT INTO xp_events (user_id, reason, amount) VALUES ($1, $2, $3)")
            .bind(user_id)
            .bind(reason)
            .bind(effective)
            .execute(&mut *tx)
            .await?;
        // Upsert tổng
        let total: i32 = sqlx::query_scalar(
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
        Ok((total, level_from_xp(total)))
    }

    /// Trạng thái điểm danh hôm nay của user.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn today_checkin(pool: &PgPool, user_id: Uuid) -> AppResult<Option<DailyCheckin>> {
        let row = sqlx::query_as::<_, DailyCheckin>(
            "SELECT user_id, checkin_date, streak, xp_awarded, created_at
             FROM daily_checkins WHERE user_id = $1 AND checkin_date = CURRENT_DATE",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// Điểm danh hôm nay (idempotent — đã điểm rồi thì trả về streak cũ).
    /// Trả về (streak, xp_awarded, level_after).
    /// XP = DAILY_CHECKIN + min(streak-1, MAX_STREAK_BONUS) thưởng chuỗi.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn do_checkin(pool: &PgPool, user_id: Uuid) -> AppResult<(i32, i32, LevelInfo)> {
        let mut tx = pool.begin().await?;
        // Đã điểm hôm nay chưa?
        let existing = sqlx::query_as::<_, DailyCheckin>(
            "SELECT user_id, checkin_date, streak, xp_awarded, created_at
             FROM daily_checkins WHERE user_id = $1 AND checkin_date = CURRENT_DATE",
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(c) = existing {
            let level = level_from_xp(Self::total_xp(pool, user_id).await?);
            return Ok((c.streak, c.xp_awarded, level));
        }
        // Chuỗi: hôm qua có điểm không?
        let yesterday_streak: Option<i32> = sqlx::query_scalar(
            "SELECT streak FROM daily_checkins
             WHERE user_id = $1 AND checkin_date = CURRENT_DATE - 1",
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?;
        let streak = yesterday_streak.map_or(1, |s| s + 1);
        let bonus = (streak - 1).min(xp::MAX_STREAK_BONUS);
        let xp_awarded = xp::DAILY_CHECKIN + bonus;
        sqlx::query(
            r"INSERT INTO daily_checkins (user_id, checkin_date, streak, xp_awarded)
               VALUES ($1, CURRENT_DATE, $2, $3)
               ON CONFLICT (user_id, checkin_date) DO NOTHING",
        )
        .bind(user_id)
        .bind(streak)
        .bind(xp_awarded)
        .execute(&mut *tx)
        .await?;
        // XP qua cùng tx để đảm bảo nhất quán
        sqlx::query(
            "INSERT INTO xp_events (user_id, reason, amount) VALUES ($1, 'daily_checkin', $2)",
        )
        .bind(user_id)
        .bind(xp_awarded)
        .execute(&mut *tx)
        .await?;
        let total: i32 = sqlx::query_scalar(
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
            let today = Utc::now().date_naive();
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
        }
        let s = sqlx::query_as::<_, Stats>(
            r"SELECT
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
                (SELECT COALESCE(total_xp, 0)::bigint FROM user_xp_totals WHERE user_id = $1) AS total_xp",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let level = level_from_xp(s.total_xp as i32).level;
        let met = |id: &str| -> bool {
            match id {
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
                _ => false,
            }
        };

        let catalog = Self::list_achievements(pool).await?;
        let mut granted = Vec::new();
        for a in &catalog {
            if met(&a.id) {
                if let Some(newly) = Self::grant_achievement(pool, user_id, &a.id).await? {
                    granted.push(newly);
                }
            }
        }
        Ok(granted)
    }

    /// Bảng xếp hạng top XP (kèm số game + chuỗi hiện tại).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn leaderboard_top_xp(pool: &PgPool, limit: i64) -> AppResult<Vec<LeaderboardEntry>> {
        let rows = sqlx::query_as::<_, LeaderboardEntry>(
            r"SELECT u.username, u.display_name, u.avatar_url,
                      x.total_xp,
                      (SELECT COUNT(*) FROM games g
                        WHERE g.user_id = u.id AND g.status = 'published') AS games_count,
                      COALESCE((SELECT c.streak FROM daily_checkins c
                        WHERE c.user_id = u.id
                        ORDER BY c.checkin_date DESC LIMIT 1), 0)::bigint AS streak
               FROM user_xp_totals x
               JOIN users u ON u.id = x.user_id
               WHERE u.is_banned = FALSE AND u.role <> 'ai_agent'
               ORDER BY x.total_xp DESC, u.created_at ASC
               LIMIT $1",
        )
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
        let c: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM daily_checkins WHERE checkin_date = CURRENT_DATE",
        )
        .fetch_one(pool)
        .await?;
        Ok(c)
    }

    /// Số huy hiệu được trao hôm nay (admin dashboard).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn achievements_today_count(pool: &PgPool) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM user_achievements
             WHERE earned_at >= date_trunc('day', NOW())",
        )
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
