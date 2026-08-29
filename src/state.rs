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
    /// v2.9.0 — Typing indicator: ai đó đang gõ. Broadcast qua channel
    /// (không ghi DB — ephemeral). Client hiển thị "X đang gõ..." 4s.
    Typing { user_id: Uuid, display_name: String },
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
    /// Presence chat: map `user_id → số WebSocket connection đang mở`
    /// (ref-count — xem [`PresenceMap`]).
    /// v2.9.2 FIX: trước đây là `HashSet<Uuid>` (chỉ đếm user duy nhất) —
    /// mở 2 tab rồi đóng tab 1 sẽ xoá user khỏi set dù tab 2 còn kết nối
    /// → "số người online" giảm sai, event Presence broadcast sai cho mọi
    /// client. Giờ đếm theo connection (ref-count): user chỉ rời khỏi map
    /// khi ĐÓNG TẤT CẢ connection.
    pub chat_online: Arc<PresenceMap>,
}

/// v2.9.2 — Cap số WebSocket connection đồng thời mỗi user (chống DoS:
/// trước đây 1 user login được mở VÔ SỐ connection, mỗi connection =
/// 1 task + rx buffer 256 event → đốt bộ nhớ. 5 là dư cho multi-tab thật).
pub const MAX_WS_CONNS_PER_USER: usize = 5;

/// Kết quả đăng ký presence cho 1 WebSocket connection mới.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresenceAdd {
    /// Vượt `MAX_WS_CONNS_PER_USER` — caller phải từ chối connection ngay.
    Rejected,
    /// User mới online (0 → 1 connection). Payload = tổng số user online
    /// MỚI — caller broadcast `ChatEvent::Presence`.
    NewlyOnline(usize),
    /// User đã online từ trước (tab/cửa sổ thứ 2+). Đã tăng ref-count,
    /// payload = số user online (không đổi) — caller bỏ qua broadcast.
    Already(usize),
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
            chat_online: Arc::new(PresenceMap::new()),
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
        // Cache stale — query DB. Nếu nhiều task cùng đến đây cùng lúc, mỗi task sẽ
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
}

/// Presence map cho live chat — ref-count WebSocket connection theo user
/// (tách thành struct riêng để unit test không cần DB/PgPool).
#[derive(Default)]
pub struct PresenceMap {
    conns: std::sync::Mutex<std::collections::HashMap<Uuid, usize>>,
}

impl PresenceMap {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Đăng ký 1 WebSocket connection (atomic check+increment dưới 1 lock).
    /// Vượt `max_conns` trả `Rejected` và KHÔNG tăng count.
    pub fn add(&self, user_id: Uuid, max_conns: usize) -> PresenceAdd {
        let mut map = self
            .conns
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let count = map.entry(user_id).or_insert(0);
        if *count >= max_conns {
            // Không tăng — chặn connection mới (chống DoS mở hàng trăm WS).
            return PresenceAdd::Rejected;
        }
        *count += 1;
        if *count == 1 {
            PresenceAdd::NewlyOnline(map.len())
        } else {
            PresenceAdd::Already(map.len())
        }
    }

    /// Gỡ 1 connection (giảm ref-count). Chỉ trả `Some(số user online MỚI)`
    /// khi đây là connection CUỐI CÙNG (user thật sự offline) — caller
    /// broadcast `ChatEvent::Presence`; các trường hợp khác trả `None`.
    pub fn remove(&self, user_id: Uuid) -> Option<usize> {
        let mut map = self
            .conns
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(count) = map.get_mut(&user_id) {
            *count -= 1;
            if *count == 0 {
                map.remove(&user_id);
                return Some(map.len());
            }
        }
        None
    }

    /// Số user DUY NHẤT đang online.
    #[must_use]
    pub fn count(&self) -> usize {
        self.conns.lock().map_or(0, |m| m.len())
    }

    /// Danh sách UUID user đang online.
    #[must_use]
    pub fn user_ids(&self) -> Vec<Uuid> {
        self.conns
            .lock()
            .map(|m| m.keys().copied().collect())
            .unwrap_or_default()
    }
}

impl AppState {
    /// Đăng ký 1 WebSocket connection của user vào presence map — delega
    /// sang [`PresenceMap::add`] (giữ API cũ cho handlers).
    pub fn presence_add(&self, user_id: Uuid, max_conns: usize) -> PresenceAdd {
        self.chat_online.add(user_id, max_conns)
    }

    /// Gỡ 1 WebSocket connection — delega sang [`PresenceMap::remove`].
    pub fn presence_remove(&self, user_id: Uuid) -> Option<usize> {
        self.chat_online.remove(user_id)
    }

    /// Số user đang online (cho HTTP GET /chat/history trả về cùng lúc).
    pub fn presence_count(&self) -> usize {
        self.chat_online.count()
    }

    /// v2.9.0 — Danh sách UUID user đang online (panel chat / online-users).
    pub fn online_user_ids(&self) -> Vec<Uuid> {
        self.chat_online.user_ids()
    }
}

#[cfg(test)]
mod presence_tests {
    use super::*;

    /// Multi-tab: 2 connection cùng user → count user = 1; đóng tab 1
    /// user VẪN online (đây là bug v2.9.1 — HashSet xoá sớm); đóng tab 2
    /// mới offline thật.
    #[test]
    fn multi_tab_refcount() {
        let p = PresenceMap::new();
        let u = Uuid::new_v4();
        assert_eq!(p.add(u, 5), PresenceAdd::NewlyOnline(1));
        // Tab 2: count không đổi, không broadcast.
        assert_eq!(p.add(u, 5), PresenceAdd::Already(1));
        assert_eq!(p.count(), 1);
        // Đóng tab 1 — user vẫn online (trước đây bug: xoá khỏi set).
        assert_eq!(p.remove(u), None);
        assert_eq!(p.count(), 1);
        // Đóng tab 2 — offline thật, broadcast count mới = 0.
        assert_eq!(p.remove(u), Some(0));
        assert_eq!(p.count(), 0);
    }

    #[test]
    fn cap_connections_per_user() {
        let p = PresenceMap::new();
        let u = Uuid::new_v4();
        for i in 0..5 {
            let res = p.add(u, 5);
            assert_ne!(res, PresenceAdd::Rejected, "connection {i} phải được nhận");
        }
        // Connection thứ 6 bị chặn — count không tăng.
        assert_eq!(p.add(u, 5), PresenceAdd::Rejected);
        assert_eq!(p.count(), 1);
        // Gỡ 1 connection → chặn được gỡ đúng ref-count, không âm.
        assert_eq!(p.remove(u), None);
    }

    #[test]
    fn remove_unknown_user_is_noop() {
        let p = PresenceMap::new();
        assert_eq!(p.remove(Uuid::new_v4()), None);
        assert_eq!(p.count(), 0);
    }

    #[test]
    fn two_users_online_count_and_ids() {
        let p = PresenceMap::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        assert!(matches!(p.add(a, 5), PresenceAdd::NewlyOnline(1)));
        assert!(matches!(p.add(b, 5), PresenceAdd::NewlyOnline(2)));
        assert_eq!(p.count(), 2);
        let mut ids = p.user_ids();
        ids.sort();
        assert_eq!(ids, vec![a, b]);
    }
}
