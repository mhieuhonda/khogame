use crate::error::AppResult;
use crate::state::AppState;
use axum::extract::State;
use axum::response::Html;
use std::sync::Arc;

pub async fn health() -> Html<&'static str> {
    Html("OK")
}

pub async fn terms(State(_state): State<Arc<AppState>>) -> AppResult<Html<String>> {
    Ok(Html(r#"
    <!DOCTYPE html><html lang="vi"><head><meta charset="UTF-8"><title>Điều khoản - Kho Game</title><link rel="stylesheet" href="/static/css/style.css"></head>
    <body><header class="site-header"><div class="container header-inner"><a href="/" class="logo"><span>Kho Game</span></a></div></header>
    <main class="site-main"><div class="container"><h1>Điều khoản sử dụng</h1>
    <p>1. Kho Game là nền tảng chia sẻ game cho cộng đồng Việt Nam.</p>
    <p>2. Người dùng chịu trách nhiệm về nội dung và game mình đăng.</p>
    <p>3. Không đăng game vi phạm bản quyền hoặc chứa mã độc.</p>
    <p>4. Không spam, quảng cáo, hoặc bình luận không phù hợp.</p>
    <p>5. Ban quản trị có quyền xóa nội dung vi phạm mà không cần thông báo trước.</p>
    </div></main></body></html>"#.into()))
}

pub async fn privacy(State(_state): State<Arc<AppState>>) -> AppResult<Html<String>> {
    Ok(Html(r#"
    <!DOCTYPE html><html lang="vi"><head><meta charset="UTF-8"><title>Chính sách bảo mật - Kho Game</title><link rel="stylesheet" href="/static/css/style.css"></head>
    <body><header class="site-header"><div class="container header-inner"><a href="/" class="logo"><span>Kho Game</span></a></div></header>
    <main class="site-main"><div class="container"><h1>Chính sách bảo mật</h1>
    <p>1. Chúng tôi chỉ thu thập thông tin cần thiết từ Google OAuth (email, tên, ảnh đại diện).</p>
    <p>2. Chúng tôi không bán hoặc chia sẻ dữ liệu của bạn cho bên thứ ba.</p>
    <p>3. Mật khẩu không được lưu trữ (đăng nhập qua Google).</p>
    <p>4. Bạn có thể yêu cầu xóa tài khoản bất cứ lúc nào.</p>
    </div></main></body></html>"#.into()))
}
