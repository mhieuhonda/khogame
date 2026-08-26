-- ============================================
-- 013: Live Chat — chat realtime cộng đồng trên trang chủ
-- ============================================
-- Bảng lưu lịch sử chat để:
--   1) User mới vào trang chủ thấy tin nhắn gần đây (load qua HTTP GET)
--   2) Phục vụ admin kiểm duyệt / truy vết spam/abuse
-- Realtime delivery dùng WebSocket (axum::extract::ws) + broadcast
-- channel trong AppState — không phải DB. DB chỉ là backing store.
--
-- Thiết kế:
--   - user_id REFERENCES users(id) ON DELETE CASCADE: user bị xóa →
--     tin nhắn tự xóa (tránh tin nhăm của user không còn tồn tại).
--   - content TEXT: hỗ trợ unicode tiếng Việt, giới hạn 500 ký tự ở app.
--   - created_at DESC index: query "30 tin gần nhất" nhanh (index scan).
--   - author_ip VARCHAR(45): IPv4/IPv6 max 45 ký tự (cho admin tra spam).
--   - author_ua TEXT: User-Agent max 512 ký tự (đã clamp ở handler).
--   - is_deleted BOOLEAN: soft delete (admin ẩn tin nhưng giữ record
--     cho audit). Realtime vẫn broadcast "deleted" event để client xoá.
-- ============================================

CREATE TABLE chat_messages (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content     TEXT NOT NULL,
    author_ip   VARCHAR(45),
    author_ua   TEXT DEFAULT '',
    is_deleted  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_chat_messages_created_at ON chat_messages(created_at DESC);
CREATE INDEX idx_chat_messages_user_id ON chat_messages(user_id);

-- ============================================
-- Notification type cho chat
-- ============================================
ALTER TYPE notification_type ADD VALUE IF NOT EXISTS 'chat_mention';
