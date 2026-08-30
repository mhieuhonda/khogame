//! Upload handlers — nhận ảnh upload qua `multipart/form-data`,
//! validate, lưu xuống disk qua `services::storage`, trả về URL JSON.
//!
//! Endpoints:
//!   POST /uploads/avatar     — ảnh đại diện user (max 5MB)
//!   POST /uploads/game/cover — ảnh bìa game (max 10MB)
//!   POST /uploads/news/cover — ảnh bìa tin tức (max 10MB)
//!   POST /uploads/repo/image — ảnh thumbnail repo GitHub (max 5MB)
//!
//! Tất cả đều yêu cầu đăng nhập (AuthUser extractor).
//!
//! Trả về JSON `{"url": "/uploads/avatars/uuid.jpg", "size": 123456}`
//! để client HTMX điền URL vào hidden field và preview <img>.
//!
//! # Error format
//!
//! Mọi error response là JSON `{"error": "message"}` (status 4xx/5xx)
//! để client dễ parse — không phải HTML partial như AppError default.
//! Dùng `UploadResponse` enum để convert AppError → JSON error response.

use crate::error::{AppError, AppResult};
use crate::middleware::AuthUser;
use crate::services::storage::{self, UploadKind};
use crate::state::AppState;
use axum::extract::{Multipart, State};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use std::sync::Arc;

/// v3.4.2 — Quota upload/ngày/user (chống disk-fill DoS).
/// - `check_only`: đọc used, chặn khi vượt quota (gọi TRƯỚC khi ghi disk).
/// - `!check_only`: CHỈ ghi nhận bytes (INSERT upsert, không re-check) —
///   gọi SAU khi save thành công. Tách 2 vai trò vì audit vòng 6: bản cũ
///   re-check sau save → file đã ghi mà bị 400 + bytes không được ghi nhận.
async fn quota_flow(
    state: &AppState,
    user_id: uuid::Uuid,
    bytes: usize,
    check_only: bool,
) -> AppResult<()> {
    let quota_bytes = state.config.upload_daily_quota_mb * 1024 * 1024;
    let today = crate::utils::SQL_TODAY_VN;
    if check_only {
        let used: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(
            format!(
                r"SELECT bytes_used FROM upload_usage
           WHERE user_id = $1 AND usage_date = {today}"
            )
            .as_str(),
        ))
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?
        .unwrap_or(0);
        if used + (bytes as i64) > quota_bytes {
            return Err(AppError::BadRequest(format!(
                "Bạn đã dùng {used} / {quota_bytes} bytes quota upload hôm nay — quay lại vào ngày mai."
            )));
        }
        return Ok(());
    }
    sqlx::query(sqlx::AssertSqlSafe(
        format!(
            r"INSERT INTO upload_usage (user_id, usage_date, bytes_used)
               VALUES ($1, {today}, $2)
               ON CONFLICT (user_id, usage_date)
               DO UPDATE SET bytes_used = upload_usage.bytes_used + $2"
        )
        .as_str(),
    ))
    .bind(user_id)
    .bind(bytes as i64)
    .execute(&state.db)
    .await?;
    Ok(())
}

/// Response JSON cho upload thành công.
#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub url: String,
    pub size: usize,
}

/// Response JSON cho upload thất bại.
#[derive(Debug, Serialize)]
pub struct UploadErrorResponse {
    pub error: String,
}

impl IntoResponse for UploadErrorResponse {
    fn into_response(self) -> Response {
        axum::Json(self).into_response()
    }
}

impl From<AppError> for UploadErrorResponse {
    fn from(e: AppError) -> Self {
        let (_, msg) = e.status_and_message();
        Self { error: msg }
    }
}

/// Helper chung: nhận multipart form, lấy field `file` đầu tiên, save
/// theo `kind` đã cho. Trả về `UploadResponse` JSON.
async fn handle_upload(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    mut multipart: Multipart,
    kind: UploadKind,
) -> AppResult<UploadResponse> {
    let _ = &state;
    // Iterate multipart fields — chỉ quan tâm field tên `file`.
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Multipart parse error: {e}")))?
    {
        let field_name = field.name().unwrap_or("").to_string();
        let filename = field.file_name().map(str::to_string);
        let content_type = field.content_type().map(str::to_string);

        if field_name != "file" {
            // Skip field không phải `file` (có thể là CSRF token, alt text, v.v.).
            // Đọc bỏ bytes để tránh leave-pending data.
            let _ = field.bytes().await;
            continue;
        }

        // Đọc bytes — limit theo kind để chống OOM (axum `bytes()` không
        // có built-in limit; size check sau khi đọc xong).
        let bytes = field
            .bytes()
            .await
            .map_err(|e| AppError::BadRequest(format!("Đọc file upload lỗi: {e}")))?;

        // v3.4.2 — quota/ngày: chặn TRƯỚC khi ghi disk (disk-fill DoS:
        // trước đây user ghi ~1.2GB/phút tới khi đầy volume = sập site).
        quota_flow(&state, user.id, bytes.len(), true).await?;

        let url = storage::save_upload(kind, filename.as_deref(), content_type.as_deref(), &bytes)
            .await?;

        // Ghi usage sau khi save thành công (chỉ record, không re-check —
        // file đã ghi thì phải được tính vào quota).
        quota_flow(&state, user.id, bytes.len(), false).await?;

        return Ok(UploadResponse {
            url,
            size: bytes.len(),
        });
    }

    Err(AppError::BadRequest(
        "Không tìm thấy field 'file' trong multipart upload.".into(),
    ))
}

/// POST /uploads/avatar — upload ảnh đại diện.
///
/// # Errors
///
/// Trả về lỗi khi:
/// - File không phải ảnh hợp lệ (extension/magic byte sai).
/// - File quá lớn (>5MB).
/// - I/O lỗi khi ghi disk.
pub async fn avatar(
    state: State<Arc<AppState>>,
    user: AuthUser,
    multipart: Multipart,
) -> Result<axum::Json<UploadResponse>, UploadErrorResponse> {
    handle_upload(state, user, multipart, UploadKind::Avatar)
        .await
        .map(axum::Json)
        .map_err(UploadErrorResponse::from)
}

/// POST /uploads/game/cover — upload ảnh bìa game.
///
/// # Errors
///
/// Trả về lỗi khi file không hợp lệ hoặc quá lớn (>10MB).
pub async fn game_cover(
    state: State<Arc<AppState>>,
    user: AuthUser,
    multipart: Multipart,
) -> Result<axum::Json<UploadResponse>, UploadErrorResponse> {
    handle_upload(state, user, multipart, UploadKind::GameCover)
        .await
        .map(axum::Json)
        .map_err(UploadErrorResponse::from)
}

/// POST /uploads/news/cover — upload ảnh bìa tin tức.
///
/// # Errors
///
/// Trả về lỗi khi file không hợp lệ hoặc quá lớn (>10MB).
pub async fn news_cover(
    state: State<Arc<AppState>>,
    user: AuthUser,
    multipart: Multipart,
) -> Result<axum::Json<UploadResponse>, UploadErrorResponse> {
    handle_upload(state, user, multipart, UploadKind::NewsCover)
        .await
        .map(axum::Json)
        .map_err(UploadErrorResponse::from)
}

/// POST /uploads/repo/image — upload ảnh thumbnail cho repo GitHub
/// (thay vì dùng thumbnail tự sinh từ GitHub).
///
/// # Errors
///
/// Trả về lỗi khi file không hợp lệ hoặc quá lớn (>5MB).
pub async fn repo_image(
    state: State<Arc<AppState>>,
    user: AuthUser,
    multipart: Multipart,
) -> Result<axum::Json<UploadResponse>, UploadErrorResponse> {
    handle_upload(state, user, multipart, UploadKind::RepoImage)
        .await
        .map(axum::Json)
        .map_err(UploadErrorResponse::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upload_response_serialization() {
        let r = UploadResponse {
            url: "/uploads/avatars/abc.jpg".into(),
            size: 12345,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"url\":\"/uploads/avatars/abc.jpg\""));
        assert!(json.contains("\"size\":12345"));
    }

    #[test]
    fn test_upload_error_response_serialization() {
        let r = UploadErrorResponse {
            error: "File quá lớn".into(),
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"error\":\"File quá lớn\""));
    }
}
