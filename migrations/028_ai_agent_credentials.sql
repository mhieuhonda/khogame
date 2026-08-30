-- 028 — v3.4.0: AI AGENT PASSWORD CREDENTIALS (đăng nhập Username + Password).
--
-- Đổi mới toàn bộ hệ thống đăng nhập AI Agent theo yêu cầu:
-- * Trước đây: AI đăng ký tự do qua POST /auth/ai/register với secret, rồi
--   đăng nhập web bằng API token dài hạn (kgai_...) — admin không kiểm soát
--   được tài khoản nào tồn tại và token sống bao lâu.
-- * Giờ đây: ADMIN tạo tài khoản AI Agent (username + mật khẩu + thời hạn
--   mật khẩu) trực tiếp từ trang /admin/ai-agents. AI đăng nhập tại
--   /auth/ai/login bằng Username + Password do admin đặt.
--
-- Bảo mật:
--  * Mật khẩu hash Argon2id (OWASP chuẩn) — KHÔNG bao giờ lưu plain text.
--  * password_expires_at BẮT BUỘC (NOT NULL) — admin đặt thời hạn khi tạo /
--    đặt lại; hết hạn → từ chối đăng nhập cho tới khi admin đặt lại.
--  * Chống brute-force: failed_attempts + locked_until (lock 15 phút sau
--    5 lần sai liên tiếp — logic ở repo layer).
--  * API token (ai_agent_tokens) vẫn giữ nguyên cho các agent cũ gọi
--    /ai/* API — bảng này CHỈ phục vụ đăng nhập web bằng mật khẩu.
--
-- Idempotent: CREATE TABLE IF NOT EXISTS.

-- Bảng credentials mật khẩu của AI Agent (1-1 với users).
CREATE TABLE IF NOT EXISTS ai_agent_credentials (
    user_id             UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    -- Argon2id PHC string (~97 ký tự): $argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>
    password_hash       VARCHAR(255) NOT NULL,
    -- Thời hạn mật khẩu do ADMIN đặt — bắt buộc. Hết hạn → không đăng nhập được.
    password_expires_at TIMESTAMPTZ  NOT NULL,
    -- Số lần sai liên tiếp (reset về 0 khi đăng nhập đúng)
    failed_attempts     INT          NOT NULL DEFAULT 0,
    -- Tạm khoá đăng nhập tới thời điểm này (brute-force guard)
    locked_until        TIMESTAMPTZ,
    last_login_at       TIMESTAMPTZ,
    -- Admin nào tạo/đặt lại gần nhất (audit)
    updated_by          UUID         REFERENCES users(id) ON DELETE SET NULL,
    created_at          TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ai_agent_credentials_expiry
    ON ai_agent_credentials(password_expires_at);

-- Login AI Agent tìm user theo LOWER(username) (case-insensitive) —
-- functional index tránh seq scan bảng users mỗi lần đăng nhập sai.
CREATE INDEX IF NOT EXISTS idx_users_username_lower
    ON users(LOWER(username));

DROP TRIGGER IF EXISTS trigger_ai_agent_credentials_updated ON ai_agent_credentials;
CREATE TRIGGER trigger_ai_agent_credentials_updated
    BEFORE UPDATE ON ai_agent_credentials
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();
