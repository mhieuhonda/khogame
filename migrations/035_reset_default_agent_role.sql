-- Migration 035: Khôi phục role đúng cho AI Agent mặc định (GLM 5.3)
-- ============================================================
-- Bối cảnh (v3.6.2): trên prod ghi nhận user glm53 (AI Agent mặc định,
-- google_sub = 'ai_agent:default-glm53' do migration 027 seed) bị đổi
-- role thành 'moderator' tay qua admin panel. Hậu quả:
--   1. Mọi tính năng AI của hồ sơ (badge, hero FX, tham số, nút admin
--      "Đăng nhập tài khoản này") tắt lặng lẽ.
--   2. HOLE BẢO MẬT: role Moderator = staff → nếu bot GLM 5.3 đăng nhập
--      phiên web, nó có quyền truy cập /admin/* (require_admin chỉ check
--      is_staff) — vi phạm spec "AI không được đụng admin".
--
-- Fix dữ liệu: đặt role về 'ai_agent' — idempotent, chỉ đụng ĐÚNG user
-- có google_sub cố định của agent mặc định, không đụng user nào khác.
--
-- Lớp code phòng vệ song song (v3.6.2):
--   - `is_ai_agent_user()` nhận diện qua role HOẶC google_sub — tính
--     năng AI không còn phụ thuộc role không bị đổi tay.
--   - `require_admin` chặn tuyệt đối mọi user is_ai_agent_user() vào
--     /admin/* bất kể role.

UPDATE users
SET role = 'ai_agent',
    updated_at = NOW()
WHERE google_sub = 'ai_agent:default-glm53'
  AND role <> 'ai_agent';
