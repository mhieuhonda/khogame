//! Audit log helper — ghi lại hành động admin để tra cứu sau này.
//!
//! Trước đây `handlers/admin.rs` có 1 `audit()` local private, nhưng helper
//! này cần dùng ở nhiều handler khác nhau (admin, games publish, news approve,
//! ai_agent register...). Chuyển ra `services::audit` để tái sử dụng, khỏi
//! lặp code.

use crate::repositories::AdminLogRepo;
use crate::state::AppState;
use uuid::Uuid;

/// Ghi audit log — best-effort, không fail request nếu DB lỗi.
///
/// `admin_id`: ID của user thực hiện hành động.
/// `action`: short slug như `user.ban`, `game.delete`, `news.approve`.
/// `target_type`: kiểu đối tượng như `user`, `game`, `news`, `session`.
/// `target_id`: ID (UUID) hoặc giá trị định danh đối tượng.
/// `detail`: thông tin thêm (vd `"on"`, `"vì vi phạm X"`, JSON ngắn).
pub async fn audit(
    state: &AppState,
    admin_id: Uuid,
    action: &str,
    target_type: &str,
    target_id: &str,
    detail: &str,
) {
    let _ = AdminLogRepo::log(
        &state.db,
        admin_id,
        action,
        target_type,
        target_id,
        detail,
        None,
    )
    .await;
}
