use crate::error::AppResult;
use crate::middleware::CurrentUser;
use crate::state::AppState;
use crate::templates::{
    AboutPageTemplate, ErrorTemplate, MarkdownGuideTemplate, PrivacyPageTemplate, TermsPageTemplate,
};
use askama::Template;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use std::sync::Arc;

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn terms(
    State(_state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<TermsPageTemplate> {
    // Trang đầy đủ layout (header/footer) thay vì HTML rời trước đây
    Ok(TermsPageTemplate {
        current_user,
        unread_notifications: 0,
    })
}

/// v3.2.0 — Trang Thông tin (/about): giới thiệu Louis Space, tác giả
/// Hieu Louis, hướng dẫn sử dụng, hướng dẫn viết README, FAQ.
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn about(
    State(_state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<AboutPageTemplate> {
    Ok(AboutPageTemplate {
        current_user,
        unread_notifications: 0,
    })
}

/// v3.11.0 — Trang Hướng dẫn Markdown toàn diện (GET /markdown).
///
/// Nội dung guide nằm ở `docs/markdown_guide.md` (include_str — build-time
/// embed, không tốn I/O runtime) và được render bằng CHÍNH engine
/// markdown của site (cached theo SHA256) → mọi tính năng được hướng dẫn
/// đều hiển thị đúng như thật — guide không bao giờ "lệch pha" với engine.
///
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn markdown_guide(
    State(_state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<MarkdownGuideTemplate> {
    const GUIDE_MD: &str = include_str!("../../docs/markdown_guide.md");
    Ok(MarkdownGuideTemplate {
        current_user,
        unread_notifications: 0,
        rendered: crate::services::markdown::render(GUIDE_MD),
    })
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn privacy(
    State(_state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<PrivacyPageTemplate> {
    Ok(PrivacyPageTemplate {
        current_user,
        unread_notifications: 0,
    })
}

/// Fallback handler cho mọi route không khớp — trả về trang 404
/// với giao diện Louis Space thay vì trang 404 mặc định của axum (chỉ
/// có chữ "Not Found"). Giữ trải nghiệm người dùng tốt hơn.
pub async fn not_found(CurrentUser(current_user): CurrentUser) -> Response {
    let body = ErrorTemplate {
        status: 404,
        message: "Trang bạn tìm không tồn tại hoặc đã bị di chuyển.".into(),
        current_user,
        request_id: None,
    }
    .render()
    .unwrap_or_else(|_| "404 Not Found".into());
    (StatusCode::NOT_FOUND, Html(body)).into_response()
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn maintenance(State(_state): State<Arc<AppState>>) -> AppResult<Html<String>> {
    // Trang này cố tình đứng ngoài layout: hiển thị khi site đang bảo trì
    Ok(Html(r#"
    <!DOCTYPE html><html lang="vi"><head><meta charset="UTF-8"><title>Bảo trì - Louis Space</title><link rel="stylesheet" href="/static/css/style.css">
    <script>(function(){try{var t=localStorage.getItem('ls-theme')||localStorage.getItem('kg-theme');if(t!=='dark'&&t!=='light'){t=window.matchMedia&&window.matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light';}document.documentElement.setAttribute('data-theme',t);}catch(e){document.documentElement.setAttribute('data-theme','light');}})();</script>
    </head>
    <body><main class="site-main"><div class="container" style="text-align:center;padding:80px 16px">
    <div style="font-size:72px">🛠️</div>
    <h1>Hệ thống đang bảo trì</h1>
    <p>Louis Space tạm thời ngừng phục vụ để nâng cấp. Vui lòng quay lại sau ít phút.</p>
    <p><a class="btn btn-primary" href="/">Thử lại</a></p>
    </div></main></body></html>"#.into()))
}
