use crate::error::{AppError, AppResult};
use crate::middleware::current_user_from_jar;
use crate::models::chat::ChatMessageWithUser;
use crate::repositories::ChatRepo;
use crate::state::{AppState, ChatEvent, PresenceAdd, MAX_WS_CONNS_PER_USER};
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError as BroadcastRecvError;
use uuid::Uuid;

/// Giới hạn tin nhắn: 500 ký tự (đủ cho câu chat ngắn, chặn spam novel-length).
/// Đếm theo `chars()` (Unicode scalar) — tiếng Việt 500 ký tự = ~1500 bytes UTF-8.
const MAX_MESSAGE_LEN: usize = 500;

/// Số tin nhắn gần nhất trả về cho HTTP history endpoint.
/// 50 là window đủ cho user mới vào hiểu context, không quá nặng (≈3KB JSON).
const HISTORY_LIMIT: i64 = 50;

/// Rate-limit per-user: tối đa 30 tin / 60s (0.5 msg/s avg). Spammer sẽ bị
/// reject sớm. Dùng cùng RateLimiter instance với toàn app — key riêng theo
/// `chat:<user_id>` để không đụng với bucket khác.
const CHAT_RATE_MAX: usize = 30;
const CHAT_RATE_WINDOW: u64 = 60;

// ============================================================
// HTTP GET /chat/history — fallback cho user mới vào + WebSocket fail
// ============================================================
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn history(State(state): State<Arc<AppState>>) -> AppResult<axum::Json<HistoryResponse>> {
    let (messages_res, today_count_res) = tokio::join!(
        ChatRepo::recent(&state.db, HISTORY_LIMIT),
        ChatRepo::count_today(&state.db),
    );
    let messages = messages_res?;
    // count_today fail-safe: trả 0 nếu DB lỗi (không block render history).
    let today_count = today_count_res.unwrap_or(0);
    let online = state.presence_count();
    Ok(axum::Json(HistoryResponse {
        // Reverse để client render cũ→mới từ trên xuống (DESC từ DB ngược
        // với thứ tự hiển thị tự nhiên).
        messages: messages.into_iter().rev().collect(),
        online,
        today_count,
    }))
}

#[derive(Debug, Serialize)]
pub struct HistoryResponse {
    /// 50 tin gần nhất (cũ→mới, sẵn sàng render tuần tự).
    pub messages: Vec<ChatMessageWithUser>,
    pub online: usize,
    pub today_count: i64,
}

// ============================================================
// WebSocket /chat/ws — realtime subscribe + send
// ============================================================
/// Auth wrapping: cookie jar extracted tại điểm này (trước khi upgrade WS)
/// vì các extractor không chạy được sau khi `on_upgrade` callback bắt đầu.
///
/// Trả về 401 nếu chưa đăng nhập, 403 nếu banned — `on_upgrade` chỉ chạy khi
/// cookie hợp lệ.
pub async fn ws_handler(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    headers: axum::http::HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    // v3.5.1 FIX (CSWSH defense-in-depth, audit task 5-a): middleware
    // `origin_check` chỉ phủ POST/PUT/PATCH/DELETE — GET handshake của WS
    // từng KHÔNG được kiểm Origin. SameSite=Lax đã chặn cross-site WS
    // handshake trên browser hiện đại, nhưng tầng gốc mới đúng chuẩn:
    // Origin/Referer (nếu có) phải khớp BASE_URL. Non-browser client
    // (không gửi Origin) vẫn qua như `verify_origin` vốn xử lý.
    if let Err(e) = crate::middleware::verify_origin(&headers, &state.config.base_url) {
        tracing::warn!("Từ chối WS upgrade /chat/ws: Origin không khớp BASE_URL");
        return e.into_response();
    }
    // Auth BEFORE upgrade: cookie không truyền được qua WS handshake (cookie
    // là HTTP header — chỉ có hiệu lực trong HTTP request, không qua WS frame).
    // Nên user phải được resolve ở đây, không trong WS callback.
    let user = match current_user_from_jar(&state, &jar).await {
        Some(u) => u,
        None => return AppError::Unauthorized.into_response(),
    };
    if user.is_banned {
        return AppError::Forbidden("Tài khoản đã bị khóa".into()).into_response();
    }

    let user_id = user.id;
    let is_staff = user.role.is_staff();

    // Heartbeat: gửi ping mỗi 30s để giữ connection sống và phát hiện client
    // đã đóng (NAT timeout, mạng yếu). Axum WS không có built-in keepalive.
    ws.max_message_size(64 * 1024)
        .max_frame_size(64 * 1024)
        .on_upgrade(move |socket| {
            // Clone state + user_id để move vào async task. user_id đủ cho
            // broadcast payload — không cần full User struct (đã có trong
            // ChatMessageWithUser khi INSERT vào DB và SELECT lại).
            async move { run_ws(state, socket, user_id, is_staff).await }
        })
}

/// Vòng đời 1 WebSocket connection:
/// 0. presence_add (ref-count + cap `MAX_WS_CONNS_PER_USER`) — vượt cap →
///    đóng ngay với close code 1013 (Try Again Later), không vào loop.
/// 1. Nếu user MỚI online (0→1 connection) → broadcast Presence{online}.
/// 2. Subscribe broadcast channel (TRƯỚC loop để tránh miss event đầu)
/// 3. Loop song song: select! giữa {recv WS, recv broadcast, heartbeat}
///    - WS frame → xử lý chat / delete command
///    - Broadcast event → forward xuống client qua `socket.send`
///    - Heartbeat 30s → ping giữ connection sống
/// 4. Khi recv trả None/Close/Error → break, presence_remove (giảm
///    ref-count — user chỉ offline khi đóng connection cuối), broadcast
///    Presence{online-1} chỉ khi user thật sự rời khỏi map.
///
/// v2.9.2 FIX: presence chuyển sang ref-count theo connection (multi-tab
/// đúng) + cap connection/user (chặn DoS mở hàng trăm WS đốt bộ nhớ).
async fn run_ws(state: Arc<AppState>, mut socket: WebSocket, user_id: Uuid, is_staff: bool) {
    // 0) Presence: đăng ký connection (atomic check+increment dưới 1 lock).
    match state.presence_add(user_id, MAX_WS_CONNS_PER_USER) {
        PresenceAdd::Rejected => {
            tracing::warn!(
                "Chat WS từ chối: user {user_id} vượt {MAX_WS_CONNS_PER_USER} connection"
            );
            // 1013 Try Again Later — client hiểu là “đóng tạm thời”.
            let _ = socket
                .send(Message::Close(Some(CloseFrame {
                    code: 1013,
                    reason: "Quá nhiều kết nối — hãy đóng bớt tab".into(),
                })))
                .await;
            return;
        }
        PresenceAdd::NewlyOnline(new_count) => {
            let _ = state
                .chat_tx
                .send(ChatEvent::Presence { online: new_count });
        }
        // Tab thứ 2+ của user đã online — không broadcast (count không đổi).
        PresenceAdd::Already(_) => {}
    }
    // 2) Subscribe broadcast trước khi loop — tránh miss event đầu tiên.
    let mut rx = state.chat_tx.subscribe();
    // Heartbeat 30s — gửi Ping để giữ connection sống (NAT timeout 60s+).
    let mut heartbeat = tokio::time::interval(Duration::from_secs(30));
    // Skip tick đầu (immediate fire) — chỉ ping sau 30s đầu.
    heartbeat.tick().await;

    // 3) Main loop: select! giữa 3 nguồn.
    loop {
        tokio::select! {
            // WS frame từ client — chat message hoặc admin delete command.
            msg = socket.recv() => match msg {
                Some(Ok(Message::Text(text))) => {
                    handle_text_frame(&state, user_id, is_staff, &text).await;
                }
                Some(Ok(Message::Binary(_))) => {
                    // Ignore binary — chat chỉ dùng text.
                }
                Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {
                    // Axum auto-reply Pong cho Ping.
                }
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
            },
            // Broadcast event từ server → forward xuống client.
            recv_result = rx.recv() => {
                match recv_result {
                    Ok(event) => {
                        if let Ok(json) = serde_json::to_string(&event) {
                            if socket
                                .send(Message::Text(json.into()))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                    Err(BroadcastRecvError::Lagged(skipped)) => {
                        // Lagging receiver — dropped `skipped` events.
                        // Client sẽ tự lấy lại history qua HTTP nếu cần.
                        tracing::debug!("Chat WS lagged: skipped {skipped} events");
                    }
                    Err(BroadcastRecvError::Closed) => break,
                }
            }
            // Heartbeat ping.
            _ = heartbeat.tick() => {
                if socket
                    .send(Message::Ping(vec![].into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    }

    // 4) Cleanup: giảm ref-count presence (chỉ khi là connection cuối thì
    // mới broadcast count mới — tab 2 còn mở thì user vẫn online).
    if let Some(new_count) = state.presence_remove(user_id) {
        let _ = state
            .chat_tx
            .send(ChatEvent::Presence { online: new_count });
    }
}

/// Xử lý 1 text frame từ client. Có 2 loại:
///   - Chat message: chuỗi thuần content, không phải JSON. Đơn giản hoá
///     client (chỉ cần gửi text thuần). Backend wrap vào ChatEvent.
///   - JSON command: {"action":"delete","id":"..."} cho admin ẩn tin.
async fn handle_text_frame(state: &AppState, user_id: Uuid, is_staff: bool, raw: &str) {
    // Thử parse JSON command trước — nếu fail thì coi như plain text message.
    if let Ok(cmd) = serde_json::from_str::<WsCommand>(raw) {
        match cmd.action.as_str() {
            "delete" if is_staff => {
                if let Some(id_str) = cmd.id.as_deref() {
                    if let Ok(id) = Uuid::parse_str(id_str) {
                        match ChatRepo::soft_delete(&state.db, id).await {
                            Ok(true) => {
                                let _ = state.chat_tx.send(ChatEvent::Delete { id });
                            }
                            Ok(false) => {} // already deleted or not found
                            Err(e) => tracing::warn!("Chat soft_delete DB error: {e}"),
                        }
                    }
                }
                return;
            }
            _ => {} // fallthrough to plain-text
        }
    }
    // Plain-text chat message: trim, clamp length, rate-limit, INSERT, broadcast.
    let content = raw.trim();
    if content.is_empty() {
        return;
    }
    let char_count = content.chars().count();
    if char_count > MAX_MESSAGE_LEN {
        // Truncate thay vì reject — giữ UX mượt (user paste > 500 sẽ vẫn thấy
        // tin gửi đi, chỉ bị cắt. Client có char counter để phòng trước).
        let truncated: String = content.chars().take(MAX_MESSAGE_LEN).collect();
        send_message(state, user_id, &truncated).await;
        return;
    }
    send_message(state, user_id, content).await;
}

/// Helper: validate rate-limit, INSERT vào DB, broadcast cho mọi subscriber.
/// Tách ra để truncate path và normal path dùng chung.
async fn send_message(state: &AppState, user_id: Uuid, content: &str) {
    // Rate-limit per-user — key `chat:<user_id>` độc lập với bucket khác
    // (rate_limit middleware không chạy cho WS frame, chỉ chạy cho HTTP request).
    let key = format!("chat:{user_id}");
    if !state
        .rate_limiter
        .check(&key, CHAT_RATE_MAX, CHAT_RATE_WINDOW)
    {
        tracing::warn!("Chat rate limit hit for user {user_id}");
        // Không gửi error về client — client sẽ thấy tin nhắn không được
        // broadcast, đây là intentional UX (giữ chat nhẹ). Để client hiển thị
        // tin nhắn local, sau vài giây sẽ tự hiểu là server đã drop.
        return;
    }
    match ChatRepo::create(&state.db, user_id, content, None, None).await {
        Ok(message) => {
            let _ = state.chat_tx.send(ChatEvent::Message { message });
            // v2.9.0 — XP chat + huy hiệu chat đầu tiên (best-effort)
            let db = state.db.clone();
            tokio::spawn(async move {
                crate::services::gamification::on_chat_message(&db, user_id).await;
            });
            // v3.0.0 — quest chat + heatmap (best-effort)
            let db_ret = state.db.clone();
            let ret_uid = user_id;
            tokio::spawn(async move {
                crate::services::retention::on_action(db_ret, ret_uid, "chat", 1).await;
            });
        }
        Err(e) => tracing::error!("Chat create DB error: {e}"),
    }
}

/// JSON command từ client qua WebSocket. Hiện chỉ support `delete` (admin).
#[derive(Debug, Deserialize)]
struct WsCommand {
    action: String,
    #[serde(default)]
    id: Option<String>,
}

// ============================================================
// HTTP POST /chat/delete/{id} — admin/staff ẩn tin nhắn qua HTTP
// (dùng cho admin panel sau này, hoặc client không hỗ trợ WS)
// ============================================================
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn http_delete(
    State(state): State<Arc<AppState>>,
    crate::middleware::AuthUser(user): crate::middleware::AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<axum::Json<serde_json::Value>> {
    if !user.role.is_staff() {
        return Err(AppError::Forbidden("Chỉ staff mới ẩn được tin nhắn".into()));
    }
    if ChatRepo::soft_delete(&state.db, id).await? {
        let _ = state.chat_tx.send(ChatEvent::Delete { id });
    }
    Ok(axum::Json(serde_json::json!({"ok": true})))
}

// ============================================================
// HTTP GET /chat/online — poll fallback cho client không hỗ trợ WS
// ============================================================
pub async fn online(State(state): State<Arc<AppState>>) -> axum::Json<OnlineResponse> {
    axum::Json(OnlineResponse {
        online: state.presence_count(),
    })
}

#[derive(Debug, Serialize)]
pub struct OnlineResponse {
    pub online: usize,
}

// ============================================================
// Auth hint: cho client biết endpoint yêu cầu login (WS handshake
// không tự redirect được — client phải tự check)
// ============================================================
/// Trả về 200 nếu đã đăng nhập, 401 nếu chưa — client dùng trước khi mở WS
/// để tránh mở WS vào rồi bị đóng ngay (race).
pub async fn auth_check(
    crate::middleware::CurrentUser(user): crate::middleware::CurrentUser,
) -> Response {
    if user.is_some() {
        StatusCode::OK.into_response()
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

#[derive(Deserialize)]
pub struct SuggestParams {
    pub q: Option<String>,
}

// ============================================================
// v2.9.0 — TYPING INDICATOR + ONLINE USERS LIST
// ============================================================

/// POST /chat/typing — broadcast "đang gõ" (không ghi DB).
/// Client gọi throttle 3s/lần khi user gõ. AuthUser chống spam mạo danh.
/// # Errors
///
/// Trả về lỗi khi chưa đăng nhập.
pub async fn typing(
    State(state): State<Arc<AppState>>,
    crate::middleware::AuthUser(user): crate::middleware::AuthUser,
) -> StatusCode {
    // Rate-limit: mỗi user tối đa 20 typing event/phút (client throttle
    // 3s = 20/min, kẻ xấu gọi liên tục sẽ bị chặn).
    let key = format!("chat-typing:{}", user.id);
    if !state.rate_limiter.check(&key, 20, 60) {
        return StatusCode::TOO_MANY_REQUESTS;
    }
    let _ = state.chat_tx.send(ChatEvent::Typing {
        user_id: user.id,
        display_name: user.display_name,
    });
    StatusCode::OK
}

/// GET /chat/online-users — danh sách user đang online (panel chat).
/// Trả về JSON {online: n, users: [{username, display_name, avatar_url, role}]}.
/// # Errors
///
/// Trả về lỗi khi DB fail.
pub async fn online_users(
    State(state): State<Arc<AppState>>,
) -> AppResult<axum::Json<OnlineUsersResponse>> {
    let ids = state.online_user_ids();
    let users = crate::repositories::collection::online_users_info(&state.db, &ids).await?;
    Ok(axum::Json(OnlineUsersResponse {
        online: ids.len(),
        users,
    }))
}

#[derive(Debug, Serialize)]
pub struct OnlineUsersResponse {
    pub online: usize,
    pub users: Vec<crate::repositories::collection::OnlineUser>,
}
