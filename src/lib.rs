pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod handlers;
pub mod janitor;
pub mod middleware;
pub mod models;
pub mod repositories;
pub mod routes;
pub mod services;
pub mod state;
pub mod templates;
pub mod utils;

pub use config::AppConfig;
pub use error::{AppError, AppResult};
pub use state::AppState;

use std::sync::Arc;

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn run(config: AppConfig) -> anyhow::Result<()> {
    // Base URL cho filter abs_url trong template (og:image/twitter:image
    // cần URL tuyệt đối — crawler không resolve path tương đối).
    crate::templates::init_base_url(&config.base_url);
    let state = AppState::new(config.clone()).await?;

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&state.db)
        .await
        .map_err(|e| anyhow::anyhow!("Migration failed: {e}"))?;

    let state = Arc::new(state);

    // Janitor nền: dọn session hết hạn & notification cũ mỗi 6h
    // (override bằng JANITOR_INTERVAL_SECS). Detached task — tự kết thúc
    // cùng process khi shutdown.
    tokio::spawn(janitor::run_janitor((*state).clone()));

    // v2.2.0 — Email flusher nền: gửi email queue mỗi 2 phút.
    // Detached task. Bỏ qua nếu SMTP chưa cấu hình (flush_pending sẽ noop).
    tokio::spawn(janitor::run_email_flusher((*state).clone()));

    // v2.9.1 — Job nền refresh metadata repo GitHub (số sao/fork/issues)
    // mỗi 3h (override bằng REPO_REFRESH_INTERVAL_SECS). FIX lỗi "repo
    // GitHub không cập nhật số sao" — trước đây metadata chỉ thay đổi khi
    // chủ repo bấm "Làm mới"/đăng lại thủ công. Detached task.
    tokio::spawn(janitor::run_repo_star_refresh((*state).clone()));
    // v3.0.0 — weekly digest email (sáng thứ 2 giờ VN)
    tokio::spawn(janitor::run_weekly_digest((*state).clone()));

    let app = routes::build_router(state);

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Listening on http://{}", addr);
    // Dùng into_make_service_with_connect_info để middleware có thể lấy IP thật
    // của client qua ConnectInfo<SocketAddr> extractor (chống giả mạo X-Forwarded-For).
    //
    // Graceful shutdown: khi nhận SIGTERM (docker stop / kubectl drain) hoặc
    // SIGINT (Ctrl+C), server ngừng nhận connection mới nhưng chờ tối đa
    // GRACEFUL_SHUTDOWN_TIMEOUT_SECS (mặc định 30s) cho các request đang
    // xử lý hoàn tất trước khi thoát — tránh drop request giữa chừng gây
    // lỗi 5xx cho người dùng cuối lúc deploy.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    tracing::info!("Server đã dừng an toàn (graceful shutdown hoàn tất)");
    Ok(())
}

/// Lắng nghe tín hiệu dừng: SIGTERM (container/orchestrator) và SIGINT (Ctrl+C).
///
/// Trả về khi nhận được tín hiệu đầu tiên. Sau khi nhận, spawn 2 task nền:
///   1. Grace period timer: sau `GRACEFUL_SHUTDOWN_TIMEOUT_SECS` (mặc định 30s),
///      nếu vẫn còn in-flight connection, `std::process::exit(0)` force-thoát
///      tránh treo vĩnh viễn chờ docker SIGKILL.
///   2. Second-signal handler: đợi SIGINT/SIGTERM thứ hai, nếu nhận →
///      `std::process::exit(1)` ngay lập tức (bypass grace period).
///      Operator nhấn Ctrl+C lần nữa khi đã chờ quá lâu sẽ force-kill thay
///      vì phải đợi hết grace period.
///
/// Lưu ý về behavior: tokio `signal::ctrl_c()` cài handler toàn cục đè
/// default OS action (terminate). Sau khi future đầu tiên hoàn thành (signal
/// thứ nhất) và bị drop trong `tokio::select!`, handler vẫn còn đăng ký
/// nhưng KHÔNG có future nào đợi → signal thứ hai bị swallow (không exit).
/// Fix: spawn thêm `ctrl_c()` future thứ hai đón signal thứ hai → exit(1).
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                tracing::error!("Không đăng ký được SIGTERM handler: {}", e);
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => tracing::info!("Nhận SIGINT — bắt đầu graceful shutdown"),
        () = terminate => tracing::info!("Nhận SIGTERM — bắt đầu graceful shutdown"),
    }
    // Cưỡng chế grace period — bỏ qua giá trị 0/âm để tránh exit tức thì
    let grace_secs: u64 = std::env::var("GRACEFUL_SHUTDOWN_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|v| *v > 0)
        .unwrap_or(30);
    // Task 1: grace period timer — force exit nếu in-flight connection vẫn
    // treo sau grace_secs (docker SIGKILL sẽ tới sau ~10s nữa, mình chủ động
    // exit trước để log sạch).
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(grace_secs)).await;
        tracing::warn!(
            "Grace period {}s đã hết nhưng còn connection chưa đóng — force exit",
            grace_secs
        );
        std::process::exit(0);
    });
    // Task 2: second-signal handler — operator nhấn Ctrl+C lần 2 (hoặc
    // SIGTERM lần 2 từ docker kill) sẽ force-exit ngay không chờ grace.
    // Trước đây comment doc nói "tín hiệu thứ hai sẽ force exit nhờ hành vi
    // mặc định của tokio" — KHÔNG ĐÚNG: tokio đã cài handler đè default,
    // signal thứ hai chỉ bị swallow. Phải spawn handler riêng để đón.
    tokio::spawn(async move {
        let second_ctrl_c = async {
            let _ = tokio::signal::ctrl_c().await;
        };
        #[cfg(unix)]
        let second_terminate = async {
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(mut sig) => {
                    sig.recv().await;
                }
                Err(_) => std::future::pending::<()>().await,
            }
        };
        #[cfg(not(unix))]
        let second_terminate = std::future::pending::<()>();
        tokio::select! {
            () = second_ctrl_c => {}
            () = second_terminate => {}
        }
        tracing::warn!("Nhận tín hiệu dừng lần 2 — force exit ngay (bypass grace period)");
        std::process::exit(1);
    });
}
