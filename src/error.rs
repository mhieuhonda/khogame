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
            sqlx::Error::RowNotFound => AppError::NotFound("Bản ghi không tồn tại".into()),
            _ => AppError::Database(e),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::BadRequest(_) | AppError::Conflict(_) => StatusCode::BAD_REQUEST,
            AppError::Database(e) => {
                tracing::error!("DB error: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let msg = self.to_string();
        tracing::warn!("AppError: {} ({})", msg, status);

        #[derive(Template)]
        #[template(path = "partials/error.html")]
        struct ErrorPartial {
            message: String,
            status: u16,
        }

        if status == StatusCode::UNAUTHORIZED {
            return (
                StatusCode::UNAUTHORIZED,
                [("HX-Redirect", "/login")],
                "Redirecting to login...",
            )
                .into_response();
        }

        ErrorPartial {
            message: msg,
            status: status.as_u16(),
        }
        .render()
        .map(|html| (status, html).into_response())
        .unwrap_or_else(|_| (status, "Internal Server Error").into_response())
    }
}
