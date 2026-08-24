use crate::error::AppResult;
use crate::middleware::CurrentUser;
use crate::state::AppState;
use crate::templates::{PrivacyPageTemplate, TermsPageTemplate};
use axum::extract::State;
use axum::response::Html;
use std::sync::Arc;

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

pub async fn privacy(
    State(_state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<PrivacyPageTemplate> {
    Ok(PrivacyPageTemplate {
        current_user,
        unread_notifications: 0,
    })
}

pub async fn maintenance(State(_state): State<Arc<AppState>>) -> AppResult<Html<String>> {
    // Trang này cố tình đứng ngoài layout: hiển thị khi site đang bảo trì
    Ok(Html(r#"
    <!DOCTYPE html><html lang="vi" data-theme="dark"><head><meta charset="UTF-8"><title>Bảo trì - Kho Game</title><link rel="stylesheet" href="/static/css/style.css"></head>
    <body><main class="site-main"><div class="container" style="text-align:center;padding:80px 16px">
    <div style="font-size:72px">🛠️</div>
    <h1>Hệ thống đang bảo trì</h1>
    <p>Kho Game tạm thời ngừng phục vụ để nâng cấp. Vui lòng quay lại sau ít phút.</p>
    <p><a class="btn btn-primary" href="/">Thử lại</a></p>
    </div></main></body></html>"#.into()))
}
