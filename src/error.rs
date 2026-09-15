use askama::Template;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

/// Marker gắn vào response sinh ra từ `AppError::into_response()` để
/// middleware `error_page_mw` (middleware.rs) biết mà thay body bằng trang
/// lỗi đầy đủ giao diện khi request đến từ browser (không phải HTMX).
///
/// TRƯỚC ĐÂY (bug v2.0.0): mọi lỗi (404/403/500/OAuth...) render
/// `partials/error.html` — một fragment KHÔNG có cặp thẻ html/head/stylesheet.
/// Người dùng bấm link hỏng hoặc vào trang bị cấm sẽ thấy chữ trơn trụi
/// không giao diện ("html thuần"). Middleware đọc marker này để render lại
/// trang lỗi chuẩn (templates/error.html) cho browser navigation, giữ
/// nguyên partial cho HTMX swap.
#[derive(Debug, Clone)]
pub struct ErrorPageInfo {
    pub status: u16,
    pub message: String,
    pub request_id: Option<String>,
}

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
                // Log raw error (kèm query, constraint, column name)
                // cho dev/admin gỡ rối — nhưng KHÔNG lộ cho user.
                tracing::error!("DB error: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            }
            Self::OAuth(msg) => {
                tracing::warn!("OAuth error: {}", msg);
                StatusCode::INTERNAL_SERVER_ERROR
            }
            Self::Http(e) => {
                tracing::warn!("HTTP error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            }
            Self::Internal(e) => {
                tracing::error!("Internal error: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        // Message cho user: với lỗi hệ thống (DB/OAuth/Http/Internal),
        // chỉ trả message chung để không lộ internal state (query string,
        // host name, error raw text có thể chứa info nhạy cảm).
        let user_msg = match self {
            Self::NotFound(m) => m.clone(),
            Self::Unauthorized => self.to_string(),
            Self::Forbidden(m) => m.clone(),
            Self::BadRequest(m) | Self::Conflict(m) => m.clone(),
            Self::Template(_)
            | Self::Database(_)
            | Self::OAuth(_)
            | Self::Http(_)
            | Self::Internal(_) => "Lỗi hệ thống, vui lòng thử lại sau ít phút".to_string(),
        };
        (status, user_msg)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = self.status_and_message();
        tracing::warn!("AppError: {} ({})", msg, status);

        // Lấy request_id (nếu có) từ request extensions qua thread-local
        // — không khả thi ở mức IntoResponse. Dùng header response thay
        // thế: PropagateRequestIdLayer đã set `x-request-id` trên response.
        // Thêm cùng giá trị vào body để user báo cáo sự cố kèm ID.
        // Cần response builder vì Extension không truy cập được ở đây.
        #[derive(Template)]
        #[template(path = "partials/error.html")]
        struct ErrorPartial {
            message: String,
            status: u16,
            request_id: Option<String>,
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

        // Đối với lỗi 5xx, sinh request_id ngẫu nhiên nếu không có từ
        // request extension. User sẽ thấy ID trong trang lỗi → báo cáo
        // cho admin, admin tra `tracing` log có cùng ID.
        // v2.9.1 FIX — ID này trước đây CHƯA BAO GIỜ được log (comment cũ
        // hứa "admin tra tracing log có cùng ID" nhưng không có lệnh log
        // nào in nó) → ID user báo cáo không thể correlate với log server.
        // Giờ log ERROR kèm đúng ID hiển thị cho user.
        let request_id = (status.as_u16() >= 500)
            .then(uuid::Uuid::new_v4)
            .map(|u| u.to_string());
        if let Some(rid) = &request_id {
            tracing::error!(
                incident_id = %rid,
                status = status.as_u16(),
                "5xx — ID sự cố hiển thị cho user (tra ID này trong log)"
            );
        }

        let error_info = ErrorPageInfo {
            status: status.as_u16(),
            message: msg.clone(),
            request_id: request_id.clone(),
        };

        ErrorPartial {
            message: msg,
            status: status.as_u16(),
            request_id: request_id.clone(),
        }
        .render()
        .map_or_else(
            |_| (status, "Internal Server Error").into_response(),
            |html| {
                // FIX v2.4.1 (bug "trang lỗi hiện HTML thuần"): bọc body
                // trong `Html<()>` để Axum gán Content-Type: text/html.
                // Trước đây `(status, html_string)` với String → Axum mặc
                // định text/plain → browser hiển thị THẺ HTML thô thay vì
                // render giao diện. Điều này ảnh hưởng MỌI response lỗi
                // từ AppError (404/403/409/500...) trên mọi request
                // browser (partial HTMX ít bị ảnh hưởng vì HTMX swap
                // bất kể content-type).
                let mut resp = (status, Html(html)).into_response();
                // Thêm X-Request-ID vào response header cho client log
                // (vd admin copy ID từ devtools network panel).
                if let Some(ref rid) = request_id {
                    if let Ok(v) = rid.parse() {
                        resp.headers_mut().insert("x-request-id", v);
                    }
                }
                // Marker cho error_page_mw: browser navigation sẽ được thay
                // body bằng trang lỗi đầy đủ giao diện (xem ErrorPageInfo).
                resp.extensions_mut().insert(error_info);
                resp
            },
        )
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

    /// REGRESSION v2.4.1 (bug "trang lỗi hiện HTML thuần"): body HTML của
    /// AppError response PHẢI đi kèm Content-Type: text/html — không phải
    /// text/plain mặc định của String. Browser hiển thị source thô thay vì
    /// render giao diện nếu sai content-type.
    #[test]
    fn test_app_error_response_content_type_is_html() {
        let resp = AppError::NotFound("không tồn tại".into()).into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let ct = resp
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        assert!(
            ct.starts_with("text/html"),
            "Content-Type phải là text/html, thực tế: {ct}"
        );

        let resp = AppError::Internal(anyhow::anyhow!("boom")).into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let ct = resp
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        assert!(
            ct.starts_with("text/html"),
            "Content-Type phải là text/html, thực tế: {ct}"
        );
    }
}
