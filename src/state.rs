use crate::config::AppConfig;
use crate::middleware::RateLimiter;
use crate::models::chat::ChatMessageWithUser;
use serde::Serialize;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use uuid::Uuid;

/// Sự kiện realtime cho Live Chat — đẩy qua WebSocket đến mọi client đang kết nối.
///
/// `Message` = tin nhắn mới (broadcast khi user gửi chat).
/// `Delete` = admin ẩn tin (broadcast để client xoá khỏi UI).
/// `Presence` = số user online cập nhật (broadcast khi client connect/disconnect).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatEvent {
    /// Tin nhắn mới — payload là message kèm author info.
    Message { message: ChatMessageWithUser },
    /// Tin nhắn bị admin ẩn — client thay nội dung bằng placeholder "đã ẩn".
    Delete { id: Uuid },
    /// Cập nhật số user đang online — client hiển thị ở header chat card.
    Presence { online: usize },
}

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<AppConfig>,
    pub http_client: reqwest::Client,
    pub rate_limiter: Arc<RateLimiter>,
    /// Cache maintenance mode (làm mới mỗi 30s)
    maintenance_cache: Arc<tokio::sync::RwLock<(bool, std::time::Instant)>>,
    /// Broadcast channel cho Live Chat realtime. Sender clone rẻ — mỗi
    /// WebSocket handler subscribe qua `subscribe()`. Buffer 256: đủ cho
    /// burst khi có nhiều user online cùng nhận message; vượt sẽ drop oldest
    /// (acceptable cho chat — client có fallback HTTP history).
    pub chat_tx: broadcast::Sender<ChatEvent>,
    /// Set user_id đang online (đếm presence). RwLock vì read nhiều hơn write.
    pub chat_online: Arc<std::sync::Mutex<std::collections::HashSet<Uuid>>>,
}

impl AppState {
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn new(config: AppConfig) -> anyhow::Result<Self> {
        let db = crate::db::connect(&config.database_url).await?;
        // Fail-fast: PgPoolOptions::connect có thể trả Ok dù DB thực sự
        // misconfigured (vd: database sai, postmaster đang restart). Phát
        // `SELECT 1` ngay — nếu fail, crash với message rõ ràng thay vì
        // để mỗi request đầu tiên đều 500.
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&db)
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "DB health check (SELECT 1) failed sau khi connect — \
                     pool mở được nhưng query fail: {e}. \
                     Có thể DATABASE_URL trỏ DB sai, postmaster đang restart, \
                     hoặc migration chưa chạy. Check config + `psql $DATABASE_URL -c '\\dt'`."
                )
            })?;
        tracing::info!("DB health check (SELECT 1) OK");
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            // Connect timeout riêng ngắn hơn — nếu DNS/TCP handshake chậm
            // (5s+), khả năng cao là mạng/registrar bị lỗi, không phải do
            // server target phản hồi chậm. Fail nhanh để error.rs có thể
            // log với delay ngắn, thay vì treo 15s tổng.
            .connect_timeout(Duration::from_secs(5))
            .user_agent(format!("KhoGame/{} (Rust)", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            db,
            config: Arc::new(config),
            http_client,
            rate_limiter: Arc::new(RateLimiter::new()),
            maintenance_cache: Arc::new(tokio::sync::RwLock::new((
                false,
                std::time::Instant::now(),
            ))),
            // Buffer 256: đủ cho burst khi nhiều user online cùng nhận message.
            // Vượt threshold → oldest drop (lagging receiver bị skip, acceptable
            // cho chat vì client có HTTP history fallback để khôi phục).
            chat_tx: broadcast::channel::<ChatEvent>(256).0,
            chat_online: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
        })
    }

    /// Kiểm tra maintenance mode với cache 30 giây
    pub async fn maintenance_enabled(&self) -> bool {
        // Đọc cache trước — nếu còn fresh thì trả ngay không cần lock write.
        {
            let cache = self.maintenance_cache.read().await;
            if cache.1.elapsed() < Duration::from_secs(30) {
                return cache.0;
            }
        }
        // Cache stale — query DB. Nếu nhiều task cùng到这里, mỗi task sẽ
        // query DB (TOCTOU giữa read và write lock). Đây là perf hit nhỏ,
        // không phải correctness bug (mỗi query cho cùng kết quả trong
        // cửa sổ này). Tránh double-check lock để giữ code đơn giản.
        let on = crate::repositories::SettingsRepo::get(&self.db, "maintenance_mode")
            .await
            .ok()
            .flatten()
            .is_some_and(|v| v == "on");
        let mut cache = self.maintenance_cache.write().await;
        *cache = (on, std::time::Instant::now());
        on
    }

    /// Đánh dấu 1 user online (thêm vào presence set). Trả về số online MỚI
    /// sau khi thêm — caller broadcast số này qua `chat_tx`.
    ///
    /// Trả về `None` nếu user đã online ở một connection khác (không thay đổi
    /// count) — caller có thể bỏ qua presence broadcast trong trường hợp đó
    /// để tránh spam event trùng.
    pub fn presence_add(&self, user_id: Uuid) -> Option<usize> {
        let mut set = self
            .chat_online
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if set.insert(user_id) {
            Some(set.len())
        } else {
            None
        }
    }

    /// Đánh dấu 1 user offline (xoá khỏi presence set). Trả về số online MỚI
    /// sau khi xoá, hoặc `None` nếu user không có trong set (không thay đổi).
    pub fn presence_remove(&self, user_id: Uuid) -> Option<usize> {
        let mut set = self
            .chat_online
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if set.remove(&user_id) {
            Some(set.len())
        } else {
            None
        }
    }

    /// Số user đang online (cho HTTP GET /chat/history trả về cùng lúc).
    pub fn presence_count(&self) -> usize {
        self.chat_online.lock().map_or(0, |s| s.len())
    }
}
