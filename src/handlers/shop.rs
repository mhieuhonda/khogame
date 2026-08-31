//! v3.0.0 — Handlers cửa hàng XP (/shop).

use crate::error::{AppError, AppResult};
use crate::middleware::{AuthUser, CurrentUser};
use crate::repositories::{GamificationRepo, ShopRepo};
use crate::state::AppState;
use crate::templates::ShopTemplate;
use axum::extract::State;
use std::sync::Arc;

/// GET /shop — trang cửa hàng (yêu cầu đăng nhập).
/// # Errors
/// Trả lỗi khi DB fail.
pub async fn shop_page(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<ShopTemplate> {
    let Some(user) = current_user else {
        return Err(AppError::Unauthorized);
    };
    let all = ShopRepo::list_for_user(&state.db, user.id).await?;
    // v3.7.0 — tách khung avatar ra đoạn riêng (preview swatch + banner
    // Rồng Lửa). Sort cùng price ASC giữ nguyên từ repo.
    let (frames, items): (Vec<_>, Vec<_>) = all
        .into_iter()
        .partition(|row| row.item.kind == "avatar_frame");
    let total_xp = GamificationRepo::total_xp(&state.db, user.id)
        .await
        .unwrap_or(0);
    let unread = crate::handlers::auth::unread_count(&state, user.id).await;
    Ok(ShopTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        items,
        frames,
        total_xp,
    })
}

/// POST /shop/buy — mua 1 vật phẩm (HTMX). Body: item_id.
/// Trả partial kết quả + cập nhật số dư (HX-Trigger events để client refresh).
/// # Errors
/// Trả lỗi khi thiếu XP / DB fail.
pub async fn buy_item(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    axum::extract::Form(form): axum::extract::Form<BuyForm>,
) -> AppResult<axum::response::Html<String>> {
    use rand::RngExt;
    let rand_val: i32 = rand::rng().random_range(0..10_000);
    let outcome = ShopRepo::buy(&state.db, user.id, &form.item_id, rand_val).await?;
    // v3.7.0 — mua khung avatar thành công → invalidate session cache để
    // header (mình) + mọi trang render lại với khung mới ngay lập tức
    // (không phải đợi cache TTL 10s tự hết hạn).
    if outcome.frame_id.is_some() {
        crate::middleware::invalidate_session_cache_for_user(user.id);
    }
    let msg = if outcome.mystery_xp > 0 {
        format!(
            "🎁 Hộp Bí Ẩn mở ra: <strong>+{} XP</strong>! Số dư: {} XP",
            outcome.mystery_xp, outcome.total_xp
        )
    } else if let Some(_fid) = &outcome.frame_id {
        // Thông báo riêng cho khung avatar — kèm hướng dẫn xem kết quả.
        format!(
            "✅ Đã kích hoạt khung avatar! Số dư còn <strong>{}</strong> XP — ra <a href=\"/profile\">hồ sơ</a> hoặc Live Chat để ngắm nhé!",
            outcome.total_xp
        )
    } else {
        format!(
            "✅ Mua thành công! Số dư còn <strong>{}</strong> XP",
            outcome.total_xp
        )
    };
    Ok(axum::response::Html(format!(
        "<div class='shop-result alert alert-success' data-xp-toast=\"Đã mua vật phẩm\">{msg}</div>"
    )))
}

#[derive(Debug, serde::Deserialize)]
pub struct BuyForm {
    pub item_id: String,
}
