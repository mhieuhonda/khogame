-- 029 — v3.4.0: USER FEEDBACK — góp ý / báo cáo lỗi / bảo mật / nâng cấp / chức năng.
--
-- Người dùng gửi góp ý tới admin xem xét. Khác với bảng reports (báo cáo
-- game vi phạm), feedback là kênh 2 chiều user ↔ admin cho chính nền tảng:
--  * Góp ý           — ý kiến cải thiện chung
--  * Báo cáo lỗi     — bug user gặp trên site
--  * Bảo mật         — lỗ hổng bảo mật user phát hiện (chỉ admin xem,
--                       KHÔNG hiển thị công khai)
--  * Đề xuất nâng cấp — đề xuất nâng cấp hệ thống
--  * Đề xuất chức năng — đề xuất tính năng mới
--
-- Admin xem danh sách tại /admin/feedback, đổi trạng thái + trả lời.
-- Trạng thái tái sử dụng pattern report_status: pending → reviewing →
-- resolved/dismissed.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'feedback_category') THEN
        CREATE TYPE feedback_category AS ENUM (
            'general',    -- Góp ý chung
            'bug',        -- Báo cáo lỗi
            'security',   -- Bảo mật
            'upgrade',    -- Đề xuất nâng cấp
            'feature'     -- Đề xuất chức năng
        );
    END IF;
END
$$;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'feedback_status') THEN
        CREATE TYPE feedback_status AS ENUM (
            'pending',    -- Chờ xử lý
            'reviewing',  -- Đang xem xét
            'resolved',   -- Đã xử lý
            'dismissed'   -- Đã bỏ qua
        );
    END IF;
END
$$;

-- notification_type cần thêm 'feedback_status' để INSERT thông báo cho
-- staff khi có góp ý mới (cùng pattern 'report_status' của bảng reports).
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_enum
        WHERE enumtypid = 'notification_type'::regtype
          AND enumlabel = 'feedback_status'
    ) THEN
        ALTER TYPE notification_type ADD VALUE 'feedback_status';
    END IF;
END
$$;

CREATE TABLE IF NOT EXISTS user_feedback (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Người gửi (bắt buộc đăng nhập — chống spam ẩn danh)
    user_id        UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    category       feedback_category NOT NULL DEFAULT 'general',
    -- Tóm tắt ngắn (bắt buộc) — hiển thị ở list admin
    title          VARCHAR(200) NOT NULL,
    -- Nội dung chi tiết (bắt buộc)
    body           TEXT NOT NULL,
    -- URL trang liên quan (tuỳ chọn — "bạn gặp lỗi ở trang nào?")
    page_url       VARCHAR(2048) DEFAULT '',
    -- Trạng thái xử lý bởi admin
    status         feedback_status NOT NULL DEFAULT 'pending',
    -- Phản hồi của admin cho người gửi (hiển thị ở trang feedback của user)
    admin_response TEXT DEFAULT '',
    handled_by     UUID REFERENCES users(id) ON DELETE SET NULL,
    handled_at     TIMESTAMPTZ,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Admin lọc theo trạng thái (mặc định mở trang = pending)
CREATE INDEX IF NOT EXISTS idx_user_feedback_status
    ON user_feedback(status, created_at DESC);
-- Đếm feedback pending cho badge admin nav
CREATE INDEX IF NOT EXISTS idx_user_feedback_user
    ON user_feedback(user_id, created_at DESC);

DROP TRIGGER IF EXISTS trigger_user_feedback_updated ON user_feedback;
CREATE TRIGGER trigger_user_feedback_updated
    BEFORE UPDATE ON user_feedback
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();
