-- ============================================
-- 050: Friends + DM + Group chat + unlimited chat flag
-- ============================================
-- Tính năng v3.14.0 — Kết bạn / chat riêng / nhóm chat:
--   1) users.chat_unlimited: admin cấp cho member cụ thể quyền chat
--      không giới hạn ký tự (admin mặc định unlimited ở app logic).
--   2) friendships: lời mời kết bạn (pending → accepted/declined),
--      hủy kết bạn (xóa row), chặn (blocked).
--   3) chat_conversations + chat_members: hội thoại DM (2 người) và
--      nhóm chat (nhiều người, owner/admin/member).
--   4) dm_messages: tin nhắn riêng/nhóm (text + ảnh), soft-delete.
--   5) notification_type += friend_request, dm.
--
-- Hiệu năng:
--   - DM xác định bằng dm_key (cặp UUID sort, UNIQUE) — tìm hội thoại
--     DM O(1), không cần JOIN members 2 lần.
--   - Poll tin nhắn: idx (conversation_id, created_at DESC) + LIMIT —
--     index-only scan cho thread mới.
--   - Unread badge: 1 query COUNT với last_read_at, không N+1.
--   - Không dùng broadcast WS cho tin riêng (tránh leak nội dung cho
--     mọi client WS + tránh fan-out) — client poll HTMX 3s/thread mở.
-- ============================================

-- 1) Cờ chat không giới hạn ký tự (admin cấp tay từng member).
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS chat_unlimited BOOLEAN NOT NULL DEFAULT FALSE;

-- 2) Quan hệ kết bạn.
DO $$ BEGIN
    CREATE TYPE friend_status AS ENUM ('pending', 'accepted', 'declined', 'blocked');
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

CREATE TABLE IF NOT EXISTS friendships (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    requester_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    addressee_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status          friend_status NOT NULL DEFAULT 'pending',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (requester_id != addressee_id),
    UNIQUE (requester_id, addressee_id)
);

CREATE INDEX IF NOT EXISTS idx_friendships_requester ON friendships(requester_id, status);
CREATE INDEX IF NOT EXISTS idx_friendships_addressee ON friendships(addressee_id, status);

-- 3) Hội thoại (DM + nhóm) và thành viên.
CREATE TABLE IF NOT EXISTS chat_conversations (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kind        TEXT NOT NULL CHECK (kind IN ('dm', 'group')),
    -- DM: khóa cặp user đã sort "uuidA:uuidB" (UNIQUE — mỗi cặp 1 DM).
    -- Nhóm: NULL (nhiều nhóm song song).
    dm_key      TEXT UNIQUE,
    name        VARCHAR(100) NOT NULL DEFAULT '',
    avatar_url  TEXT DEFAULT '',
    created_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK ((kind = 'dm' AND dm_key IS NOT NULL) OR (kind = 'group'))
);

CREATE TABLE IF NOT EXISTS chat_members (
    conversation_id UUID NOT NULL REFERENCES chat_conversations(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role            TEXT NOT NULL DEFAULT 'member' CHECK (role IN ('owner', 'admin', 'member')),
    joined_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_read_at    TIMESTAMPTZ,
    PRIMARY KEY (conversation_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_chat_members_user ON chat_members(user_id);

-- 4) Tin nhắn riêng/nhóm (text và/hoặc ảnh, soft-delete cho kiểm duyệt).
CREATE TABLE IF NOT EXISTS dm_messages (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID NOT NULL REFERENCES chat_conversations(id) ON DELETE CASCADE,
    sender_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content         TEXT NOT NULL DEFAULT '',
    image_url       TEXT,
    is_deleted      BOOLEAN NOT NULL DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (content <> '' OR image_url IS NOT NULL)
);

CREATE INDEX IF NOT EXISTS idx_dm_messages_conv ON dm_messages(conversation_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_dm_messages_sender ON dm_messages(sender_id);

-- 5) Loại thông báo mới (pattern giống 013 — 1 value/statement).
ALTER TYPE notification_type ADD VALUE IF NOT EXISTS 'friend_request';
ALTER TYPE notification_type ADD VALUE IF NOT EXISTS 'dm';
