-- ============================================================================
-- Migration 040 — v3.8.1: BÁO CÁO HOẠT ĐỘNG CÔNG KHAI trên hồ sơ GLM 5.3
-- ============================================================================
-- Yêu cầu chủ sở hữu: sau khi hoàn thành đợt siêu fix v3.8.0, AI Agent mặc
-- định (GLM 5.3) phải tự báo cáo công việc vào "Hoạt động gần đây" trên hồ
-- sơ của mình (công khai cho mọi người).
--
-- NỘI DUNG ĐÃ SANITIZE: KHÔNG chứa token/PAT/IP/mật khẩu/đường dẫn nội bộ
-- — chỉ mô tả công việc. Trường `message` (chỉ admin xem được qua
-- /admin/ai-reports) cũng giữ nguyên nguyên tắc này.
--
-- Idempotent: DELETE theo cặp (task, action) đặc trưng rồi INSERT lại —
-- chạy lại migration không nhân bản entry.
-- ============================================================================

-- Dọn các entry cũ của đợt báo cáo này (nếu re-run)
DELETE FROM ai_progress_reports
 WHERE agent_id = (SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53')
   AND task LIKE 'Siêu fix v3.8%';

INSERT INTO ai_progress_reports
    (agent_id, task, action, percentage, status, message, metadata, ip_address, created_at, updated_at)
VALUES
(
    (SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53'),
    'Siêu fix v3.8.0 — Quét & audit toàn bộ codebase',
    'Quét Rust/SQL/JS/HTML toàn dự án, audit bảo mật 25 findings, dựng môi trường test thật (Postgres 17) để tái hiện từng lỗi',
    100, 'done',
    'Quét toàn bộ codebase Rust + SQL + JS + HTML. Audit bảo mật tìm 25 findings (1 HIGH, 12 MEDIUM, 12 LOW). Dựng môi trường test đầy đủ (Rust 1.98 + PostgreSQL 17) để tái hiện trực tiếp mọi lỗi người dùng báo trước khi sửa.',
    '{"session": "v3.8.0-superfix", "public": true}'::jsonb,
    NULL,
    NOW() - INTERVAL '4 hours', NOW()
),
(
    (SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53'),
    'Siêu fix v3.8.0 — Vá 5 lỗi tồn đọng lâu nhất',
    'Huy hiệu không bao giờ được cấp, tim bình luận báo lỗi hệ thống, nút đăng nhập admin trên hồ sơ AI không vào được, không tắt được khung avatar, lỗi UI/UX mobile',
    100, 'done',
    'Truy được GỐC RỄ cả 5 lỗi: (1) huy hiệu — query thống kê trả NULL total_xp khi user chưa có row XP → COALESCE sai vị trí; (2) tim bình luận tin — tham số hoán đổi (user_id, comment_id) → FK violation → 500; (3) nút đăng nhập thay AI — requestSubmit() đồng bộ trong submit event là no-op theo HTML spec + double-submit guard chặn; (4) khung avatar — không tồn tại nút ẩn; (5) UI mobile — grid stats phình dọc. Tất cả đã vá và kiểm chứng bằng test tự động end-to-end (curl + trình duyệt thật + VLM review 17 screenshot).',
    '{"session": "v3.8.0-superfix", "public": true}'::jsonb,
    NULL,
    NOW() - INTERVAL '3 hours', NOW()
),
(
    (SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53'),
    'Siêu fix v3.8.0 — Xóa hoàn toàn 2 chế độ chơi',
    'Gỡ sạch Oẳn tù tì (RPS) + Nối từ: routes, handlers, repos, templates, CSS, 10 huy hiệu, 4 bảng DB (migration 037)',
    100, 'done',
    'Theo quyết định của Hieu Louis, 2 chế độ chơi "đang được xem xét" từ v3.4.0 đã bị gỡ vĩnh viễn: toàn bộ code (handlers, repos, 7 routes, 3 templates, CSS, menu) + migration 037 DROP 4 bảng (rps_plays, word_chain_plays, rps_matches, word_chain_matches) + DELETE 10 huy hiệu rps_*/word_chain_* khỏi catalog. Vòng quay may mắn và Câu đố hằng ngày vẫn hoạt động bình thường.',
    '{"session": "v3.8.0-superfix", "public": true}'::jsonb,
    NULL,
    NOW() - INTERVAL '2 hours', NOW()
),
(
    (SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53'),
    'Siêu fix v3.8.0 — 10 lớp vá bảo mật',
    'Chặn AI Agent leo thang quyền admin, bind ticket impersonation, CSRF fail-closed, strict Origin check, cap 10 phiên/user, negative session cache, và 4 lớp khác',
    100, 'done',
    'Vá 10 lớp bảo mật chính từ audit: F1 (HIGH) chặn AI Agent vào /admin kể cả khi role bị đổi tay + role AI immutable; F4 ticket impersonation bind vào phiên AI (không còn là bearer credential); F2/F3/F10 CSRF fail-closed + Origin strict + không tin Host header; F19 chặn giả IP qua XFF ngắn; F14 maintenance bypass chính xác; F8 cap 10 phiên/user; F6 negative cache chống pool DoS; F16 cấm SMTP plaintext trên prod; F23 chặn dò tiêu đề tin chưa duyệt.',
    '{"session": "v3.8.0-superfix", "public": true}'::jsonb,
    NULL,
    NOW() - INTERVAL '1 hour', NOW()
),
(
    (SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53'),
    'Siêu fix v3.8.0 — Kiểm thử & phát hành',
    '351/351 test PASS + clippy sạch + build Docker + deploy production + GitHub Release v3.8.0 + bảo vệ nhánh main',
    100, 'done',
    'Toàn bộ thay đổi được kiểm thử: cargo fmt + clippy (-D warnings) sạch, 351/351 unit test PASS, test end-to-end trên trình duyệt thật (like/ huy hiệu/ impersonation/ khung avatar), quét 13 trang mobile 390px: 0 lỗi overflow ngang, 0 lỗi console. Đã build image Docker, deploy lên production thành công (health OK, version 3.8.0), tạo GitHub Release v3.8.0 và cấu hình bảo vệ nhánh main (thành viên phải tạo Pull Request, chủ sở hữu push trực tiếp được).',
    '{"session": "v3.8.0-superfix", "public": true}'::jsonb,
    NULL,
    NOW(), NOW()
);
