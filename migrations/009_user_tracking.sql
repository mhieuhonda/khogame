-- ============================================
-- 009: User tracking fields for admin detail view
-- Lưu IP/UA lúc signup và login gần nhất để admin truy vết abuse.
-- Moderator KHÔNG thấy được các trường này (chỉ admin).
-- ============================================

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS signup_ip VARCHAR(45),
    ADD COLUMN IF NOT EXISTS signup_ua TEXT DEFAULT '',
    ADD COLUMN IF NOT EXISTS last_login_ip VARCHAR(45),
    ADD COLUMN IF NOT EXISTS last_login_ua TEXT DEFAULT '',
    ADD COLUMN IF NOT EXISTS last_login_at TIMESTAMPTZ;

-- Index cho truy vết IP (admin search "ai đăng từ IP này")
CREATE INDEX IF NOT EXISTS idx_users_signup_ip ON users(signup_ip);
CREATE INDEX IF NOT EXISTS idx_users_last_login_ip ON users(last_login_ip);

-- Backfill last_login_at từ sessions cũ (lấy session gần nhất)
UPDATE users u
SET last_login_at = (
    SELECT MAX(s.created_at) FROM sessions s WHERE s.user_id = u.id
),
last_login_ip = (
    SELECT s.ip_address FROM sessions s
    WHERE s.user_id = u.id
    ORDER BY s.created_at DESC LIMIT 1
),
last_login_ua = COALESCE((
    SELECT s.user_agent FROM sessions s
    WHERE s.user_id = u.id
    ORDER BY s.created_at DESC LIMIT 1
), '')
WHERE u.last_login_at IS NULL;
