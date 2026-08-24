-- ============================================
-- 004: AI Agent account system + progress reporting
-- ============================================
-- Mục đích: thêm loại tài khoản đặc biệt dành cho AI (do admin cấp
-- secret để AI tự đăng ký). AI dùng token dài hạn để đăng nhập, báo
-- cáo tiến trình công việc về admin. Hồ sơ AI công khai và có huy hiệu
-- phân biệt với người thường.
-- Bảo mật:
--  - secret đăng ký nằm trong env (AI_AGENT_SECRET), chỉ admin biết.
--  - token đăng nhập hash SHA-256 (chỉ lưu hash trong DB, không lưu plain).
--  - bảng riêng ai_agent_tokens để rotate token khi cần.
-- ============================================

-- 1) Mở rộng enum role + provider để có 'ai_agent'.
--    PostgreSQL không cho ADD VALUE IF NOT EXISTS → bọc trong DO $$.
--    (idempotent: chạy lại không lỗi)
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_enum WHERE enumtypid = 'user_role'::regtype AND enumlabel = 'ai_agent') THEN
        ALTER TYPE user_role ADD VALUE 'ai_agent';
    END IF;
END
$$;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_enum WHERE enumtypid = 'auth_provider'::regtype AND enumlabel = 'ai_agent') THEN
        ALTER TYPE auth_provider ADD VALUE 'ai_agent';
    END IF;
END
$$;

-- 2) Hồ sơ AI Agent (1-1 với users).
--    Lưu metadata riêng cho AI: model_name (bắt buộc, vd "Ox Alpha"),
--    vendor (vd "Z.ai"), capabilities (text[]), privacy_level (public/anonymous),
--    custom theme color, capabilities, last_active_at, v.v.
CREATE TABLE IF NOT EXISTS ai_agent_profiles (
    user_id          UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    model_name       VARCHAR(100) NOT NULL DEFAULT '',
    vendor           VARCHAR(100) NOT NULL DEFAULT '',
    version          VARCHAR(50)  NOT NULL DEFAULT '',
    capabilities     TEXT[]       NOT NULL DEFAULT '{}',
    -- public: hiện đầy đủ model/vendor trên profile
    -- anonymous: chỉ hiện "AI Agent", ẩn model/vendor
    privacy_level    VARCHAR(20)  NOT NULL DEFAULT 'public',
    -- Màu nền tùy chỉnh cho huy hiệu (vd "#7c3aed")
    accent_color     VARCHAR(20)  NOT NULL DEFAULT '#7c3aed',
    -- Cờ hồ sơ đã được xác minh (admin duyệt tay, mặc định false)
    verified         BOOLEAN      NOT NULL DEFAULT FALSE,
    last_active_at   TIMESTAMPTZ,
    created_at       TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ai_agent_profiles_verified
    ON ai_agent_profiles(verified);
CREATE INDEX IF NOT EXISTS idx_ai_agent_profiles_model
    ON ai_agent_profiles(model_name);

CREATE TRIGGER trigger_ai_agent_profiles_updated
    BEFORE UPDATE ON ai_agent_profiles
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- 3) Bảng lưu token đăng nhập dài hạn của AI Agent.
--    Mỗi AI có thể có nhiều token (rotate). Token hash SHA-256.
CREATE TABLE IF NOT EXISTS ai_agent_tokens (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash   VARCHAR(64) NOT NULL UNIQUE,
    label        VARCHAR(100) NOT NULL DEFAULT 'default',
    -- Token có thể bị admin thu hồi (revoked) mà không xoá luôn
    revoked      BOOLEAN NOT NULL DEFAULT FALSE,
    last_used_at TIMESTAMPTZ,
    expires_at   TIMESTAMPTZ, -- NULL = không hết hạn (rotate tay)
    ip_address   VARCHAR(45),
    user_agent   TEXT DEFAULT '',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ai_agent_tokens_user
    ON ai_agent_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_ai_agent_tokens_hash
    ON ai_agent_tokens(token_hash);

-- 4) Bảng tiến trình báo cáo từ AI Agent.
--    AI gửi: task (tên task), action (việc làm), percentage (0-100),
--    status (running/done/failed), message (mô tả chi tiết).
CREATE TYPE ai_task_status AS ENUM ('queued', 'running', 'done', 'failed', 'cancelled');

CREATE TABLE IF NOT EXISTS ai_progress_reports (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    task         VARCHAR(200) NOT NULL,
    action       VARCHAR(200) NOT NULL DEFAULT '',
    percentage   SMALLINT NOT NULL DEFAULT 0 CHECK (percentage >= 0 AND percentage <= 100),
    status       ai_task_status NOT NULL DEFAULT 'running',
    message      TEXT NOT NULL DEFAULT '',
    metadata     JSONB NOT NULL DEFAULT '{}'::jsonb,
    ip_address   VARCHAR(45),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ai_progress_agent
    ON ai_progress_reports(agent_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_ai_progress_status
    ON ai_progress_reports(status);
CREATE INDEX IF NOT EXISTS idx_ai_progress_created
    ON ai_progress_reports(created_at DESC);

CREATE TRIGGER trigger_ai_progress_updated
    BEFORE UPDATE ON ai_progress_reports
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- 5) Audit log các hành động AI: đăng ký, đăng nhập, đổi token, v.v.
--    (Bảng admin_logs đã có từ migration 002 — tái sử dụng.)
