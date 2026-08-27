-- ============================================
-- 016 — Role badge effects preference (v2.1.0)
--
-- Thêm cột `role_badge_effects` vào user_preferences: cho phép user
-- (Admin/Mod) BẬT/TẮT hiệu ứng khung chức vụ trên hồ sơ của mình:
--   - Admin     : chữ rainbow + khung lửa rực cháy (animation CSS)
--   - Moderator : hiệu ứng Glitch
--   - Member    : không có hiệu ứng (badge thường)
-- Toggle trong trang /profile/edit (chỉ hiện với staff).
--
-- DEFAULT TRUE: bật sẵn cho mọi user hiện tại & user mới — hiệu ứng
-- là "điểm nhấn" của chức vụ; ai không thích thì tự tắt trong hồ sơ.
-- ============================================

ALTER TABLE user_preferences
    ADD COLUMN IF NOT EXISTS role_badge_effects BOOLEAN NOT NULL DEFAULT TRUE;
