use askama::Template;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Không tìm thấy trang: {0}")]
    NotFound(String),
    #[error("Không có quyền truy cập")]
    Unauthorized,
    #[error("Cấm truy cập: {0}")]
    Forbidden(String),
    #[error("Dữ liệu không hợp lệ: {0}")]
    BadRequest(String),
    #[error("Xung đột dữ liệu: {0}")]
    Conflict(String),
    #[error("Lỗi cơ sở dữ liệu: {0}")]
    Database(sqlx::Error),
    #[error("Lỗi template: {0}")]
    Template(#[from] askama::Error),
    #[error("Lỗi OAuth: {0}")]
    OAuth(String),
    #[error("Lỗi HTTP: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Lỗi nội bộ: {0}")]
    Internal(#[from] anyhow::Error),
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => Self::NotFound("Bản ghi không tồn tại".into()),
            // Unique violation (23505): map sang Conflict để handler phân
            // biệt được race trùng slug/UNIQUE constraint thay vì nhận 500
            // chung chung. as_database_error() trả &[u8] code chuẩn SQLSTATE.
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                Self::Conflict("Dữ liệu đã tồn tại (trùng khóa duy nhất)".into())
            }
            _ => Self::Database(e),
        }
    }
}

impl AppError {
    /// HTTP status + thông điệp người dùng cho lỗi này. Tách riêng để
    /// unit test được (`IntoResponse` cần render template, khó test hơn).
    pub fn status_and_message(&self) -> (StatusCode, String) {
        let status = match self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::BadRequest(_) | Self::Conflict(_) => StatusCode::BAD_REQUEST,
            Self::Database(e) => {
                tracing::error!("DB error: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = self.status_and_message();
        tracing::warn!("AppError: {} ({})", msg, status);

        #[derive(Template)]
        #[template(path = "partials/error.html")]
        struct ErrorPartial {
            message: String,
            status: u16,
        }

        if status == StatusCode::UNAUTHORIZED {
            // HX-Redirect cho client HTMX; Location + 303 cho browser
            // thường (form POST truyền thống) — trước đây trình duyệt
            // thường thấy body text trơ trọi 'Redirecting to login...'
            // thay vì được chuyển trang.
            return (
                StatusCode::SEE_OTHER,
                [
                    ("HX-Redirect", "/login"),
                    ("Location", "/login"),
                    ("Cache-Control", "no-store"),
                ],
                "Redirecting to login...",
            )
                .into_response();
        }

        ErrorPartial {
            message: msg,
            status: status.as_u16(),
        }
        .render().map_or_else(|_| (status, "Internal Server Error").into_response(), |html| (status, html).into_response())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unique violation phải map sang Conflict (409 logic), không phải 500.
    #[test]
    fn test_conflict_maps_to_bad_request_status_not_500() {
        let e = AppError::Conflict("trùng slug".into());
        let (status, _msg) = e.status_and_message();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    /// Mọi variant phải map đúng nhóm status — guard hồi quy khi thêm variant mới.
    #[test]
    fn test_status_mapping_all_variants() {
        assert_eq!(
            AppError::NotFound("x".into()).status_and_message().0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            AppError::Unauthorized.status_and_message().0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            AppError::Forbidden("x".into()).status_and_message().0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            AppError::BadRequest("x".into()).status_and_message().0,
            StatusCode::BAD_REQUEST
        );
    }
}
