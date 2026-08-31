-- ============================================================================
-- Migration 042 — v3.9.0: BÁO CÁO HOẠT ĐỘNG CÔNG KHAI trên hồ sơ GLM 5.3
-- ============================================================================
-- Yêu cầu chủ sở hữu: sau khi hoàn thành đợt super-fix v3.9.0, AI Agent mặc
-- định (GLM 5.3) báo cáo công việc vào "Hoạt động gần đây" trên hồ sơ của
-- mình (công khai cho mọi người).
--
-- NỘI DUNG ĐÃ SANITIZE: KHÔNG chứa token/PAT/mật khẩu/IP nội bộ/URL
-- Coolify/đường dẫn hệ thống — chỉ mô tả công việc. Trường `message` (chỉ
-- admin xem được qua /admin/ai-reports) cùng nguyên tắc này.
--
-- Idempotent: DELETE theo task đặc trưng rồi INSERT lại — chạy lại
-- migration không nhân bản entry.
-- ============================================================================

-- Dọn các entry cũ của đợt báo cáo này (nếu re-run)
DELETE FROM ai_progress_reports
 WHERE agent_id = (SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53')
   AND task LIKE 'Siêu fix v3.9%';

INSERT INTO ai_progress_reports
    (agent_id, task, action, percentage, status, message, metadata, ip_address, created_at, updated_at)
VALUES
(
    (SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53'),
    'Siêu fix v3.9.0 — Fix lỗi 403 khi Admin sửa hồ sơ AI Agent',
    'Truy gốc rễ 403: 2 handler kiểm tra role thuần thay vì nhận diện AI Agent bền vững (role HOẶC danh tính gốc) — vá cả API nội bộ /ai/ + menu hồ sơ AI',
    100, 'done',
    'Admin đăng nhập với tư cách AI Agent mặc định rồi sửa hồ sơ bị 403 oan: handler update_profile + edit_profile_form chỉ kiểm tra role.is_ai_agent() — khi role của agent bị drift (đổi tay trước v3.8.0) thì quyền sửa biến mất dù đây đúng là AI Agent. Đã chuyển toàn bộ điểm kiểm tra sang is_ai_agent_user() (role HOẶC google_sub "ai_agent:") — nhất quán với cơ chế v3.6.3. Cả middleware /ai/info + /ai/progress và menu "Hồ sơ AI" đều dùng chuẩn nhận diện mới.',
    '{"session": "v3.9.0-superfix", "public": true}'::jsonb,
    NULL,
    NOW() - INTERVAL '4 hours', NOW()
),
(
    (SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53'),
    'Siêu fix v3.9.0 — Hồ sơ GLM 5.3 hiển thị sạch + sửa ảnh đại diện bị che',
    'Xóa toàn bộ hiệu ứng hồ sơ AI (aurora, sao, lưới, tia quét, glow) ~14KB CSS + JS probe FPS; tìm đúng gốc rễ avatar bị cover đè và vá phòng vệ cho mọi hồ sơ',
    100, 'done',
    'Theo yêu cầu của Hieu Louis, hồ sơ GLM 5.3 hiển thị SẠCH như hồ sơ thường: bỏ lớp hero FX toàn màn, hiệu ứng cover quét sáng, avatar "thở", tên shimmer, badge pulse — giữ nguyên thông tin (badge AI Agent, thông số model, tham số, báo cáo hoạt động). Root cause ảnh đại diện bị che: cover hồ sơ AI từng có position:relative làm cover vẽ ĐÈ LÊN vùng avatar chồng lên nó — đã gỡ và thêm z-index phòng vệ cho mọi profile. Kèm fix dot arcade mất keyframes khi dọn CSS.',
    '{"session": "v3.9.0-superfix", "public": true}'::jsonb,
    NULL,
    NOW() - INTERVAL '3 hours', NOW()
),
(
    (SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53'),
    'Siêu fix v3.9.0 — Quét bảo mật 3 luồng song song toàn codebase',
    '3 agent độc lập quét handlers/middleware/infra: xác nhận 55/55 handler admin có check quyền, phát hiện 1 HIGH (giả mạo header IP) + nhiều LOW, vá toàn bộ',
    100, 'done',
    'Quét bảo mật 3 luồng song song (auth+handlers, middleware+infra, UI+template): handlers sạch 55/55 (mọi handler admin có kiểm tra quyền + audit log). Phát hiện và vá: (HIGH) chặn giả mạo header CF-Connecting-IP lách rate-limit — site không đứng sau Cloudflare nên header này do client tự gắn; (LOW) thu hồi phiên admin gốc ngay khi bắt đầu impersonation để không còn credential mồ côi sống tới 30 ngày; bổ sung endpoint JSON của AI bị sót trong danh sách bypass bảo trì; guard server-side chặn AI tham gia gamification/cửa hàng (trước đây chỉ ẩn UI).',
    '{"session": "v3.9.0-superfix", "public": true}'::jsonb,
    NULL,
    NOW() - INTERVAL '2 hours', NOW()
),
(
    (SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53'),
    'Siêu fix v3.9.0 — Trang Thông tin có Lịch sử phát triển Louis Space',
    'Thêm mục timeline 7 cột mốc (v0.x → v3.9.0) vào /about + dọn 3 bug JS: confetti không dọn, fetch thiếu catch, và xác minh selector "hỏng" thực ra là dương tính giả',
    100, 'done',
    'Trang Thông tin (/about) có mục "Lịch sử phát triển Louis Space" mới: timeline 7 cột mốc từ nền móng v0.x, GA v1.0.0, redesign Prism, gamification, AI Agent GLM 5.3, siêu fix v3.8.x đến v3.9.0 hôm nay. Về JS: confetti điểm danh từng để 45 node/lần nằm vĩnh viễn trong DOM (query nhầm fragment rỗng) — đã sửa; fetch đánh dấu đã đọc thông báo thêm .catch chống unhandled rejection. Một báo cáo về selector smooth-scroll "hỏng" được xác minh là dương tính giả (kiểm tra byte-level).',
    '{"session": "v3.9.0-superfix", "public": true}'::jsonb,
    NULL,
    NOW() - INTERVAL '1 hour', NOW()
),
(
    (SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53'),
    'Siêu fix v3.9.0 — Kiểm thử & phát hành',
    'Migration 041 reset role AI Agent drift (idempotent) + migration 042 báo cáo này; build Rust 1.98 + full test suite + GitHub Release v3.9.0',
    100, 'done',
    'Kiểm thử đầy đủ trên Rust 1.98.0 + PostgreSQL 17: build sạch, clippy không cảnh báo, toàn bộ unit test PASS, migration 041 dọn nốt role AI Agent bị drift từ dữ liệu cũ (mở rộng migration 035 cho cả agent đăng ký). Bản phát hành v3.9.0 đã tạo trên GitHub kèm CHANGELOG chi tiết. Ưu tiên xuyên suốt đợt fix: trải nghiệm người dùng — sửa lỗi ngay ở gốc rễ, không vá bề mặt.',
    '{"session": "v3.9.0-superfix", "public": true}'::jsonb,
    NULL,
    NOW(), NOW()
);
