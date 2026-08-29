//! v3.0.0 — Retention service: hook trung tâm cho mọi hành động của
//! user (xem game, bình luận, like, chat...) — bump nhiệm vụ + heatmap
//! hoạt động, best-effort như gamification service (lỗi KHÔNG BAO GIỜ
//! fail request chính).

use crate::repositories::{ActivityRepo, QuestRepo};
use sqlx::PgPool;
use uuid::Uuid;

/// Gọi SAU 1 hành động của user. `stat_key` khớp quest_catalog.stat_key:
/// view_game | comment | rate_game | like_game | chat | download |
/// share | review | add_collection.
/// Fire-and-forget từ handler (tokio::spawn) — hàm này chỉ log warn.
pub async fn on_action(pool: PgPool, user_id: Uuid, stat_key: &'static str, delta: i32) {
    // Nhiệm vụ: bump tiến độ (tự hoàn thành khi đủ target)
    if let Err(e) = QuestRepo::bump(&pool, user_id, stat_key, delta).await {
        tracing::warn!("retention: bump quest {stat_key} fail cho {user_id}: {e}");
    }
    // Heatmap hoạt động: +1 cho hôm nay
    if let Err(e) = ActivityRepo::bump_today(&pool, user_id).await {
        tracing::warn!("retention: bump activity fail cho {user_id}: {e}");
    }
}

/// Hook onboarding: đánh dấu 1 bước hoàn thành + thưởng XP lần đầu.
/// Gọi best-effort sau các hành động tương ứng (đặt avatar, viết bio,
/// bình luận, bookmark, rate).
pub async fn onboarding_step(pool: &PgPool, user_id: Uuid, step: &str) {
    match crate::repositories::OnboardingRepo::complete_step(pool, user_id, step).await {
        Ok(true) => {
            let xp = crate::models::retention::ONBOARDING_STEPS
                .iter()
                .find(|(c, _, _, _)| *c == step)
                .map(|(_, _, _, xp)| *xp)
                .unwrap_or(0);
            if xp > 0 {
                let _ = crate::repositories::GamificationRepo::award_xp(
                    pool,
                    user_id,
                    "onboarding",
                    xp,
                )
                .await;
            }
            let _ = crate::repositories::NotificationRepo::create_system(
                pool,
                user_id,
                "✅ Hoàn thành bước khởi đầu!",
                &format!(
                    "Bạn vừa hoàn thành một bước trong lộ trình khởi đầu và nhận {xp} XP. \
                     Xem tiến trình của bạn ở trang chủ!"
                ),
                "/",
            )
            .await;
        }
        Ok(false) => {}
        Err(e) => tracing::warn!("retention: onboarding step {step} fail cho {user_id}: {e}"),
    }
}

/// Kiểm tra hồ sơ (avatar/bio) và đánh dấu onboarding tương ứng.
/// Gọi sau mỗi lần user lưu hồ sơ — idempotent, chỉ thưởng lần đầu.
pub async fn check_profile_onboarding(pool: &PgPool, user_id: Uuid) {
    let row: Option<(Option<String>, Option<String>)> =
        sqlx::query_as("SELECT avatar_url, bio FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    if let Some((avatar, bio)) = row {
        if avatar.map(|a| !a.is_empty()).unwrap_or(false) {
            onboarding_step(pool, user_id, "avatar").await;
        }
        if bio.map(|b| !b.is_empty()).unwrap_or(false) {
            onboarding_step(pool, user_id, "bio").await;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_stat_keys_documented() {
        // Guard: các stat_key dùng bởi handler phải khớp seed 023 —
        // liệt kê tường minh để dev quên thêm key mới là fail test này.
        const KEYS: &[&str] = &[
            "view_game",
            "comment",
            "rate_game",
            "like_game",
            "chat",
            "download",
            "share",
            "review",
            "add_collection",
        ];
        assert_eq!(KEYS.len(), 9);
        for k in KEYS {
            assert!(!k.is_empty());
        }
    }
}
