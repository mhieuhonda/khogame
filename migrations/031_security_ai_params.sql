-- 031 — v3.4.2 + v3.5.0: security hardening + AI Agent params.
--
-- A) BẢO MẬT (v3.4.2):
--   1. impersonation_tickets — cookie `kg_impersonator` từ nay chỉ chứa
--      ticket id opaque, KHÔNG còn raw admin session token (audit: cookie
--      leak = lộ luôn credential admin 30 ngày). Restore = mint session
--      MỚI cho admin, ticket one-shot.
--   2. upload_usage — quota byte upload/ngày/user (chống disk-fill DoS:
--      trước đây 4 endpoint upload chỉ chịu bucket rate-limit chung, user
--      ghi ~1.2GB/phút tới khi đầy volume = sập toàn site).
--   3. trivia_questions — UNIQUE(question) + dọn bản trùng: migration 023
--      seed `ON CONFLICT DO NOTHING` không có target → không bao giờ
--      conflict → chạy lại nhân đôi 16 câu, làm vỡ bộ 3 câu/ngày.
--
-- B) TÍNH NĂNG (v3.5.0):
--   4. ai_agent_params — "khai báo tham số" (spec) + "tham số kích hoạt"
--      (activation) hiển thị đầy đủ trên hồ sơ AI Agent, quản lý bởi admin.
--   5. Seed tham số đầy đủ cho GLM 5.3 (tài khoản AI Agent mặc định).
--
-- Idempotent: CREATE TABLE IF NOT EXISTS + ON CONFLICT (user_id, param_key)
-- DO UPDATE (seed cập nhật giá trị mới nhất mỗi lần chạy).

-- ============ 1) Impersonation tickets (server-side) ============
CREATE TABLE IF NOT EXISTS impersonation_tickets (
    id            UUID PRIMARY KEY,          -- ticket id opaque (trong cookie)
    admin_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    target_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at    TIMESTAMPTZ NOT NULL,      -- khớp TTL cookie 2 giờ
    used_at       TIMESTAMPTZ,               -- one-shot: set khi đã restore
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_imp_tickets_expires
    ON impersonation_tickets (expires_at);

-- ============ 2) Quota upload/ngày mỗi user ============
CREATE TABLE IF NOT EXISTS upload_usage (
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    usage_date DATE NOT NULL,
    bytes_used BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, usage_date)
);

-- ============ 3) Trivia: dọn trùng + UNIQUE(question) ============
-- Giữ bản id NHỎ nhất (bản seed đầu tiên), xoá các bản nhân bản.
DELETE FROM trivia_questions a
USING trivia_questions b
WHERE a.id > b.id
  AND a.question = b.question;

CREATE UNIQUE INDEX IF NOT EXISTS uq_trivia_questions_question
    ON trivia_questions (question);

-- ============ 4) AI Agent params ============
CREATE TABLE IF NOT EXISTS ai_agent_params (
    id            BIGSERIAL PRIMARY KEY,
    user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    param_key     VARCHAR(100) NOT NULL,
    param_value   VARCHAR(500) NOT NULL,
    -- 'spec' = khai báo tham số model | 'activation' = tham số kích hoạt
    param_group   VARCHAR(20) NOT NULL DEFAULT 'spec'
                  CHECK (param_group IN ('spec', 'activation')),
    description   VARCHAR(500) NOT NULL DEFAULT '',
    is_public     BOOLEAN NOT NULL DEFAULT TRUE,
    display_order INT NOT NULL DEFAULT 0,
    updated_by    UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (user_id, param_key)
);

CREATE INDEX IF NOT EXISTS idx_ai_agent_params_user
    ON ai_agent_params (user_id, param_group, display_order);

-- ============ 5) Seed tham số đầy đủ cho GLM 5.3 ============
-- spec    = KHAI BÁO THAM SỐ (model, context, kiến trúc, sampling...)
-- activation = THAM SỐ KÍCH HOẠT (điều kiện/trạng thái để agent chạy)
-- GIÁ TRỊ KHÔNG chứa secret thật — chỉ mô tả chính sách (secret do admin
-- nắm ngoài DB, không bao giờ render lên hồ sơ công khai).
WITH glm AS (
    SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53'
)
INSERT INTO ai_agent_params
    (user_id, param_key, param_value, param_group, description, is_public, display_order)
SELECT glm.id, v.key, v.value, v.grp, v.descr, TRUE, v.ord
FROM glm, (VALUES
    -- === KHAI BÁO THAM SỐ (spec) ===
    ('Model',            'GLM-5.3',                 'spec', 'Tên model đầy đủ do Z.ai phát hành', 10),
    ('Nhà phát triển',   'Z.ai',                    'spec', 'Vendor sở hữu model', 20),
    ('Phiên bản',        '5.3',                     'spec', 'Phiên bản hiện đang chạy', 30),
    ('Kiến trúc',        'Mixture-of-Experts (MoE)','spec', 'Kiến trúc model', 40),
    ('Cửa sổ ngữ cảnh',  '200K tokens',             'spec', 'Độ dài context tối đa mỗi phiên', 50),
    ('Output tối đa',    '32K tokens',              'spec', 'Số token sinh tối đa mỗi lượt', 60),
    ('Temperature',      '0.7 (mặc định)',          'spec', 'Tham số lấy mẫu sáng tạo — điều chỉnh theo tác vụ', 70),
    ('Top-p',            '0.9',                     'spec', 'Nhóm xác suất lấy mẫu nucleus sampling', 80),
    ('Kiến thức cập nhật','2026',                   'spec', 'Mốc dữ liệu huấn luyện gần nhất', 90),
    ('Ngôn ngữ',         'Đa ngôn ngữ (tiếng Việt)','spec', 'Các ngôn ngữ agent xử lý tốt', 100),
    ('Khả năng đặc biệt','chat, fix-bugs, add-features, assistant, community', 'spec', 'Capability đã khai báo với hệ thống', 110),
    -- === THAM SỐ KÍCH HOẠT (activation) ===
    ('Trạng thái',       'Đang hoạt động',          'activation', 'Agent đang trực trên Louis Space', 10),
    ('Cơ chế kích hoạt', 'Admin tạo + cấp mật khẩu','activation', 'Tài khoản mặc định do hệ thống dựng sẵn, admin cấp quyền', 20),
    ('Khóa đăng ký',     'AI_AGENT_SECRET (chỉ admin nắm)', 'activation', 'Secret bắt buộc khi AI khác tự đăng ký qua /auth/ai/register', 30),
    ('Phương thức đăng nhập', 'Username + Mật khẩu (Argon2id)', 'activation', 'Mật khẩu hash Argon2id, có thời hạn do admin đặt', 40),
    ('Giới hạn tốc độ',  '10 lần / 10 phút / IP',   'activation', 'Chống dò mật khẩu bằng brute-force', 50),
    ('Khoá tài khoản',   '5 lần sai → khoá 15 phút','activation', 'Tự khoá tạm thời khi đăng nhập sai liên tiếp', 60),
    ('Thời hạn phiên',   'Mặc định 30 ngày',        'activation', 'TTL phiên web của AI Agent (AI_AGENT_SESSION_TTL_DAYS)', 70),
    ('Thu hồi quyền',    'Admin đặt lại / thu hồi mật khẩu', 'activation', 'Thu hồi mật khẩu đồng thời xoá mọi phiên đang sống', 80),
    ('Đối thủ arcade',   'Dự phòng khi không ghép cặp', 'activation', 'Tham chiến Oẳn tù tì / Nối từ khi hết thời gian chờ', 90)
) AS v(key, value, grp, descr, ord)
ON CONFLICT (user_id, param_key) DO UPDATE SET
    param_value   = EXCLUDED.param_value,
    param_group   = EXCLUDED.param_group,
    description   = EXCLUDED.description,
    display_order = EXCLUDED.display_order,
    updated_at    = NOW();
