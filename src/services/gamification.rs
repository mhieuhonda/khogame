//! v2.9.0 — Gamification service: nơighép repo + notification thành
//! các "hook" gọi sau hành động người dùng (comment, đăng game, like…).
//!
//! Nguyên tắc:
//! - Mọi hàm đều BEST-EFFORT: lỗi gamification KHÔNG BAO GIỜ làm fail
//!   request chính (log warn rồi thôi — user vẫn comment thành công dù
//!   XP fail). Gamification là lớp "vui", không phải nghiệp vụ lõi.
//! - Hook được spawn bằng tokio::spawn tại call-site (fire-and-forget)
//!   để không cộng thêm latency vào request.

use crate::repositories::{GamificationRepo, NotificationRepo};
use sqlx::PgPool;
use uuid::Uuid;

/// Gửi notification khi lên cấp (level tăng so với trước khi cộng XP).
async fn notify_level_up(
    pool: &PgPool,
    user_id: Uuid,
    level: crate::models::gamification::LevelInfo,
) {
    let username = sqlx::query_scalar::<_, String>("SELECT username FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
    let _ = NotificationRepo::create_system(
        pool,
        user_id,
        &format!("Lên cấp {} — {}!", level.level, level.title),
        &format!(
            "Tổng XP của bạn giờ là {}. Tiếp tục phát huy để mở khóa cấp độ tiếp theo!",
            level.xp
        ),
        &format!("/u/{username}"),
    )
    .await;
}

/// Cộng XP + tự gửi notification lên cấp nếu vượt ngưỡng.
pub async fn award_xp(pool: &PgPool, user_id: Uuid, reason: &str, amount: i32) {
    if let Ok((_, level)) = GamificationRepo::award_xp(pool, user_id, reason, amount).await {
        // Level-up check: đọc level TRƯỚC không khả thi trong award đã gộp —
        // thay vào đó dùng logic "XP mới chạm đúng ngưỡng"? Đơn giản hoá:
        // so level suy từ (total - amount) với level mới.
        let prev = crate::models::gamification::level_from_xp(level.xp - amount.max(0));
        if level.level > prev.level {
            notify_level_up(pool, user_id, level).await;
        }
    }
}

/// Trao huy hiệu đạt điều kiện + XP thưởng + notification từng huy hiệu.
/// Gọi SAU khi award_xp (để achievement 'level_5' thấy XP mới nhất).
pub async fn check_achievements(pool: &PgPool, user_id: Uuid) {
    let granted = match GamificationRepo::check_and_award(pool, user_id).await {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!("gamification: check_achievements fail cho {user_id}: {e}");
            return;
        }
    };
    for a in granted {
        // XP thưởng huy hiệu (reason riêng để activity feed hiển thị đẹp)
        if a.xp_reward > 0 {
            let _ = GamificationRepo::award_xp(pool, user_id, "achievement", a.xp_reward).await;
        }
        let _ = NotificationRepo::create_system(
            pool,
            user_id,
            &format!("{} Mở khóa huy hiệu: {}", a.icon, a.title),
            &format!(
                "{} — thưởng {} XP. Xem bộ sưu tập huy hiệu của bạn!",
                a.description, a.xp_reward
            ),
            "/achievements",
        )
        .await;
        tracing::info!(user = %user_id, achievement = %a.id, "Trao huy hiệu");
    }
}

/// Hook sau khi ĐĂNG NHẬP thành công (mỗi lần login — cheap, chỉ 1-2 query).
pub async fn on_login(pool: &PgPool, user_id: Uuid) {
    // first_login achievement + các huy hiệu onboarding user có thể
    // đã đạt từ trước (avatar/bio/social nếu OAuth có sẵn ảnh)
    check_achievements(pool, user_id).await;
}

/// Hook sau khi ĐĂNG/PUBLISH game.
pub async fn on_game_published(
    pool: &PgPool,
    user_id: Uuid,
    _game_id: Uuid,
    game_slug: &str,
    game_title: &str,
) {
    award_xp(
        pool,
        user_id,
        "post_game",
        crate::repositories::gamification::xp::POST_GAME,
    )
    .await;
    check_achievements(pool, user_id).await;

    // Thông báo cho mọi follower: "người bạn theo dõi vừa đăng game"
    // (type 'new_game' — đã có sẵn trong DB enum từ 001).
    let followers: Vec<Uuid> =
        match sqlx::query_scalar("SELECT follower_id FROM follows WHERE followee_id = $1")
            .bind(user_id)
            .fetch_all(pool)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("gamification: lấy followers fail: {e}");
                return;
            }
        };
    if followers.is_empty() {
        return;
    }
    let username = sqlx::query_scalar::<_, String>("SELECT username FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
    let link = format!("/games/{game_slug}");
    for f in followers {
        let _ = sqlx::query(
            r"INSERT INTO notifications (user_id, actor_id, type, title, content, link)
               VALUES ($1, $2, 'new_game'::notification_type, $3, $4, $5)",
        )
        .bind(f)
        .bind(user_id)
        .bind("Người bạn theo dõi vừa đăng game mới")
        .bind(format!("{username} đã đăng «{game_title}» — đến xem ngay!"))
        .bind(&link)
        .execute(pool)
        .await;
    }
}

/// Hook sau khi viết bình luận.
pub async fn on_comment(pool: &PgPool, user_id: Uuid) {
    award_xp(
        pool,
        user_id,
        "comment",
        crate::repositories::gamification::xp::COMMENT,
    )
    .await;
    check_achievements(pool, user_id).await;
}

/// Hook sau khi like game (người like + chủ game đều được xét).
pub async fn on_like(pool: &PgPool, actor_id: Uuid, game_owner_id: Uuid) {
    // Người like: huy hiệu discovery (không cộng XP cho like để tránh farm)
    check_achievements(pool, actor_id).await;
    // Chủ game: XP nhận like
    if actor_id != game_owner_id {
        award_xp(
            pool,
            game_owner_id,
            "received_like",
            crate::repositories::gamification::xp::RECEIVED_LIKE,
        )
        .await;
        check_achievements(pool, game_owner_id).await;
    }
}

/// Hook sau khi bookmark game.
pub async fn on_bookmark(pool: &PgPool, user_id: Uuid) {
    check_achievements(pool, user_id).await;
}

/// Hook sau khi được follow mới.
pub async fn on_follow(pool: &PgPool, followee_id: Uuid) {
    award_xp(
        pool,
        followee_id,
        "received_follow",
        crate::repositories::gamification::xp::RECEIVED_FOLLOW,
    )
    .await;
    check_achievements(pool, followee_id).await;
}

/// Hook sau khi game được tải về (chủ game nhận XP).
pub async fn on_download(pool: &PgPool, game_owner_id: Uuid) {
    award_xp(
        pool,
        game_owner_id,
        "received_download",
        crate::repositories::gamification::xp::RECEIVED_DOWNLOAD,
    )
    .await;
    check_achievements(pool, game_owner_id).await;
}

/// Hook sau khi viết review.
pub async fn on_review(pool: &PgPool, user_id: Uuid, game_owner_id: Uuid) {
    award_xp(
        pool,
        user_id,
        "review",
        crate::repositories::gamification::xp::REVIEW,
    )
    .await;
    check_achievements(pool, user_id).await;
    // Chủ game cũng được xét huy hiệu likes_received / downloads…
    // (không XP — review là công của reviewer)
    if user_id != game_owner_id {
        check_achievements(pool, game_owner_id).await;
    }
}

/// Hook sau khi chia sẻ repo GitHub.
pub async fn on_repo(pool: &PgPool, user_id: Uuid) {
    award_xp(
        pool,
        user_id,
        "repo",
        crate::repositories::gamification::xp::REPO,
    )
    .await;
    check_achievements(pool, user_id).await;
}

/// Hook sau khi gửi tin nhắn chat.
pub async fn on_chat_message(pool: &PgPool, user_id: Uuid) {
    award_xp(
        pool,
        user_id,
        "chat_message",
        crate::repositories::gamification::xp::CHAT_MESSAGE,
    )
    .await;
    // chat_first achievement — không cần check toàn bộ (đắt) nhưng đơn
    // giản hoá bằng check_and_award (query tổng vẫn 1 round-trip)
    check_achievements(pool, user_id).await;
}

/// Hook sau khi admin DUYỆT tin tức của user.
pub async fn on_news_approved(pool: &PgPool, author_id: Uuid) {
    award_xp(
        pool,
        author_id,
        "post_news",
        crate::repositories::gamification::xp::POST_NEWS,
    )
    .await;
    check_achievements(pool, author_id).await;
}

/// Hook sau khi user CẬP NHẬT HỒ SƠ (avatar/bio/social → huy hiệu onboarding).
pub async fn on_profile_update(pool: &PgPool, user_id: Uuid) {
    check_achievements(pool, user_id).await;
}

/// Thông báo chào mừng thành viên mới (gọi 1 lần khi tạo user).
pub async fn send_welcome(pool: &PgPool, user_id: Uuid, display_name: &str) {
    let _ = NotificationRepo::create_system(
        pool,
        user_id,
        "Chào mừng đến Louis Space! 🎮",
        &format!(
            "Xin chào {display_name}! Hoàn thiện hồ sơ, điểm danh hàng ngày và khám phá \
             game để tích XP, lên cấp và mở khóa huy hiệu. Chúc bạn vui vẻ!"
        ),
        "/profile/edit",
    )
    .await;
}
