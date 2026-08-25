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
pub mod state;
pub mod templates;
pub mod utils;

pub use config::AppConfig;
pub use error::{AppError, AppResult};
pub use state::AppState;

use std::sync::Arc;

pub async fn run(config: AppConfig) -> anyhow::Result<()> {
    let state = AppState::new(config.clone()).await?;

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&state.db)
        .await
        .map_err(|e| anyhow::anyhow!("Migration failed: {}", e))?;

    let state = Arc::new(state);

    // Janitor nền: dọn session hết hạn & notification cũ mỗi 6h
    // (override bằng JANITOR_INTERVAL_SECS). Detached task — tự kết thúc
    // cùng process khi shutdown.
    tokio::spawn(janitor::run_janitor((*state).clone()));

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
/// Trả về khi nhận được tín hiệu đầu tiên. Tín hiệu thứ hai (nhấn Ctrl+C
/// lần nữa trong lúc chờ grace period) sẽ buộc thoát ngay nhờ hành vi
/// mặc định của tokio (không swallow).
///
/// Sau khi nhận tín hiệu, spawn bộ đếm grace period
/// (GRACEFUL_SHUTDOWN_TIMEOUT_SECS, mặc định 30s): nếu hết thời gian mà
/// vẫn còn connection treo (client chậm, download dài), force exit để
/// không treo vĩnh viễn chờ docker SIGKILL — đúng như comment tài liệu
/// đã hứa nhưng trước đây chưa được triển khai.
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
        _ = ctrl_c => tracing::info!("Nhận SIGINT — bắt đầu graceful shutdown"),
        _ = terminate => tracing::info!("Nhận SIGTERM — bắt đầu graceful shutdown"),
    }
    // Cưỡng chế grace period — bỏ qua giá trị 0/âm để tránh exit tức thì
    let grace_secs: u64 = std::env::var("GRACEFUL_SHUTDOWN_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|v| *v > 0)
        .unwrap_or(30);
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(grace_secs)).await;
        tracing::warn!(
            "Grace period {}s đã hết nhưng còn connection chưa đóng — force exit",
            grace_secs
        );
        std::process::exit(0);
    });
}
