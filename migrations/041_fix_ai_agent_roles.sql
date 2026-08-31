-- ============================================================================
-- Migration 041 — v3.9.0: Reset role cho TOÀN BỘ AI Agent bị drift
-- ============================================================================
-- Bối cảnh: migration 035 (v3.6.3) chỉ reset role cho agent MẶC ĐỊNH
-- (google_sub = 'ai_agent:default-glm53'). Các agent ĐĂNG KÝ qua
-- /auth/ai/register mang google_sub 'ai_agent:{uuid}' — nếu role bị đổi
-- tay qua admin panel TRƯỚC khi v3.8.0 chặn set_role thì dữ liệu drift
-- vẫn còn: agent mất tính năng AI (nút hồ sơ AI, /ai/info, /ai/progress)
-- và — tệ hơn — giữ role staff cũ có thể đụng /admin/* trước lớp chặn
-- is_ai_agent_user() của require_admin.
--
-- Fix dữ liệu: đặt role về 'ai_agent' cho MỌI user có google_sub prefix
-- 'ai_agent:' (danh tính gốc không thể nhầm — cùng nguồn nhận diện với
-- is_ai_agent_user()). Idempotent: chỉ UPDATE hàng lệch role.
-- App-layer v3.8.0 đã chặn admin đổi role AI từ nay về sau; migration
-- này dọn NỘT dữ liệu cũ.
-- ============================================================================

UPDATE users
SET role = 'ai_agent',
    updated_at = NOW()
WHERE google_sub LIKE 'ai_agent:%'
  AND role <> 'ai_agent';
