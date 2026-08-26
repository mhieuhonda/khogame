//! Services layer — cross-cutting flows ghép nhiều repository vào 1 nghiệp vụ.
//!
//! Mô tách biệt:
//! - `handlers/`: parse HTTP request, gọi services/repos, render response.
//! - `repositories/`: 1-1 với bảng DB, không có logic nghiệp vụ.
//! - `services/`: logic chéo — vd `audit()` ghi admin log, `game_publish()`
//!   tạo game + tags + notifications trong 1 tx.
//!
//! Hiện tại module có:
//! - `audit`: ghi admin log (chuyển từ `handlers/admin.rs`).
//! - `json_ld`: builder JSON-LD schema.org cho homepage + game detail
//!   (chuyển từ `handlers/games.rs`).

pub mod audit;
pub mod json_ld;
pub mod storage;

pub use audit::audit;
pub use json_ld::{build_breadcrumb_json_ld, build_game_json_ld, build_homepage_json_ld};
