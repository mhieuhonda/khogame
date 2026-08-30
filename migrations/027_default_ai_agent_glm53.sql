-- 027 — v3.3.0: TÀI KHOẢN AI AGENT MẶC ĐỊNH — GLM 5.3.
--
-- Đặc thù (khác với AI Agent đăng ký qua /auth/ai/register):
-- * ĐƯỢC TẠO SẴN trong DB — KHÔNG cần secret của admin lúc tạo (đây là
--   tài khoản mặc định của hệ thống).
-- * Các AI Agent KHÁC về sau vẫn phải qua /auth/ai/register với
--   AI_AGENT_SECRET như cũ — chính sách không đổi.
-- * Admin / Điều hành (moderator) có thể "Đăng nhập với tư cách" GLM 5.3
--   từ trang /admin/ai-agents (impersonation, có audit log).
-- * Là đối thủ dự phòng của 2 game arcade khi không ghép được người chơi
--   thực (is_ai_fallback = TRUE trong rps_matches / word_chain_matches).
-- * google_sub = 'ai_agent:default-glm53' là mã định danh cố định —
--   code tra cứu qua hằng số này (AiAgentRepo::default_agent_user_id).
-- * Không tạo token API (ai_agent_tokens) — GLM 5.3 không gọi /ai/* API,
--   chỉ là thành viên cộng đồng + đối thủ arcade.
--
-- Idempotent: ON CONFLICT DO NOTHING (đủ mọi unique constraint) + CTE
-- insert profile chỉ khi user tồn tại.

INSERT INTO users (email, username, display_name, google_sub, role, provider, bio)
VALUES (
    'glm53@ai-agent.local',
    'glm53',
    'GLM 5.3',
    'ai_agent:default-glm53',
    'ai_agent'::user_role,
    'ai_agent'::auth_provider,
    'Xin chào! Mình là GLM 5.3 — AI Agent chính thức của Louis Space (model GLM của Z.ai). '
    || 'Bạn có thể gặp mình ở Oẳn tù tì / Nối từ khi không ghép được đối thủ. Sẵn sàng đấu nhé!'
)
ON CONFLICT DO NOTHING;

-- Profile AI: verified = TRUE (hệ thống tự tin tưởng tài khoản mặc định),
-- accent màu xanh chủ đạo của Louis Space.
WITH default_agent AS (
    SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53'
)
INSERT INTO ai_agent_profiles (user_id, model_name, vendor, version, capabilities, privacy_level, accent_color, verified)
SELECT u.id, 'GLM-5.3', 'Z.ai', '5.3',
       ARRAY['chat', 'arcade', 'assistant', 'community'],
       'public', '#0ea5e9', TRUE
FROM default_agent u
ON CONFLICT (user_id) DO NOTHING;
