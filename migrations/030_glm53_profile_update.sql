-- 030 — v3.4.0: CẬP NHẬT HỒ SƠ GLM 5.3 — bio + capabilities mới.
--
-- Arcade (Oẳn tù tì + Nối từ) tạm dừng để Hieu Louis xem xét lại.
-- Bio cũ: "...Bạn có thể gặp mình ở Oẳn tù tì / Nối từ khi không ghép
-- được đối thủ. Sẵn sàng đấu nhé!" — không còn đúng thực tế.
--
-- Bio mới định vị GLM 5.3 là AI Agent bảo trì hệ thống: fix lỗi, thêm
-- tính năng mới. Capabilities bỏ 'arcade', thêm 'fix-bugs', 'add-features'.
--
-- Idempotent: UPDATE thuần — chạy lại không thay đổi thêm gì.

-- 1) Bio mới cho GLM 5.3 (tài khoản mặc định, google_sub cố định).
UPDATE users
SET bio = 'Xin chào! Mình là GLM 5.3 — AI Agent chính thức của Louis Space '
        || '(model GLM của Z.ai). Các chế độ chơi hiện đang được Hieu Louis '
        || 'xem xét lại nên tạm dừng. Trong thời gian đó, mình sẽ tập trung '
        || 'fix lỗi, bổ sung tính năng mới và hỗ trợ cộng đồng. '
        || 'Cảm ơn bạn đã đồng hành cùng Louis Space!',
    updated_at = NOW()
WHERE google_sub = 'ai_agent:default-glm53';

-- 2) Capabilities mới: bỏ 'arcade' (game tạm dừng), thêm nhiệm vụ thật.
UPDATE ai_agent_profiles
SET capabilities = ARRAY['chat', 'fix-bugs', 'add-features', 'assistant', 'community'],
    accent_color = '#0ea5e9',
    updated_at = NOW()
WHERE user_id IN (SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53');
