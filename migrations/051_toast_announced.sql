-- ============================================
-- 051: Toast realtime cho huy hiệu/lên cấp
-- ============================================
-- Vấn đề: huy hiệu/lên cấp chỉ tạo notification DB — user không hề biết
-- cho tới khi mở /notifications (badge header chỉ update khi load trang).
--
-- Giải pháp:
--   1) Tách type riêng 'achievement' + 'level_up' (trước đây dùng chung
--      'system' — không lọc được để báo realtime).
--   2) Cột `announced`: toast đã hiện cho user chưa. Endpoint
--      /notifications/toasts lấy + đánh dấu 1 query CTE atomic (không
--      bao giờ báo trùng, đa thiết bị OK). Mở /notifications KHÔNG cần
--      announced (đọc riêng bằng is_read như cũ).
-- ============================================

ALTER TYPE notification_type ADD VALUE IF NOT EXISTS 'achievement';
ALTER TYPE notification_type ADD VALUE IF NOT EXISTS 'level_up';

ALTER TABLE notifications
    ADD COLUMN IF NOT EXISTS announced BOOLEAN NOT NULL DEFAULT FALSE;

CREATE INDEX IF NOT EXISTS idx_notifications_toast
    ON notifications(user_id, created_at DESC)
    WHERE announced = FALSE AND is_read = FALSE;
