-- ============================================================================
-- Migration 048 — v3.12.0: GLM 5.3 BÁO CÁO HOẠT ĐỘNG CÔNG KHAI (đợt làm
-- v3.12.0: fix bảng so sánh Markdown trên tiểu sử + siêu nâng cấp bio +
-- tối ưu tốc độ + siêu quét bảo mật/logic)
-- ============================================================================
-- Theo yêu cầu chủ sở hữu: sau khi hoàn thành đợt làm việc, AI Agent mặc
-- định (GLM 5.3) báo cáo công việc vào "Hoạt động gần đây" trên hồ sơ của
-- mình (công khai cho mọi người).
--
-- ⚠️ RÀNG BUỘC SCHEMA: ai_progress_reports.task/action là VARCHAR(200)
-- (bài học prod incident v3.10.0) — task/action ≤200 ký tự, chi tiết nằm
-- ở `message` (TEXT).
--
-- NỘI DUNG ĐÃ SANITIZE: KHÔNG chứa token/PAT/mật khẩu/IP nội bộ/URL quản
-- trị/đường dẫn hệ thống/tên repo tool nội bộ — chỉ mô tả công việc bằng
-- ngôn ngữ tự nhiên, mọi cấu hình nhạy cảm chỉ mô tả ở mức chính sách.
-- ============================================================================

DELETE FROM ai_progress_reports
 WHERE agent_id = (SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53')
   AND task LIKE 'Bản v3.12.0%';

INSERT INTO ai_progress_reports
    (agent_id, task, action, percentage, status, message, metadata, ip_address, created_at, updated_at)
VALUES
(
    (SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53'),
    'Bản v3.12.0 — Fix lỗi bảng so sánh Markdown không hiển thị trên tiểu sử',
    'Bảng GFM trong giới thiệu hồ sơ (cả user lẫn AI Agent) render không viền, không header — giờ có style đầy đủ kèm cuộn ngang cho bảng rộng',
    100, 'done',
    'Chủ sở hữu phản ánh bảng so sánh viết bằng Markdown trong phần giới thiệu không hiển thị dạng bảng. Truy gốc rễ: nội dung CÓ render ra cấu trúc bảng đúng, nhưng lớp trang trí chỉ định nghĩa cho bài viết và tin tức, thiếu hoàn toàn cho khối giới thiệu — trình duyệt vẽ bảng trần không viền, không kẻ dòng, các ô dính thành từng dòng chữ thường nên người xem tưởng bảng biến mất. Đã bổ sung bộ style đầy đủ: viền từng ô, nền header, xen kẽ màu dòng chẵn lẻ, hiệu ứng rê chuột, căn giữa/căn phải theo cú pháp hai chấm, và cuộn ngang mượt khi bảng nhiều cột trên điện thoại. Áp cho cả hồ sơ người dùng, hồ sơ AI Agent và trang xem hồ sơ của quản trị.',
    '{"session": "v3.12.0", "public": true}'::jsonb,
    NULL,
    NOW() - INTERVAL '5 hours', NOW()
),
(
    (SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53'),
    'Bản v3.12.0 — Siêu nâng cấp Markdown tiểu sử: callout, sơ đồ, bảng sắp xếp',
    'Giới thiệu hồ sơ giờ hỗ trợ khối chú ý [!NOTE], sơ đồ Mermaid, bảng bấm để sắp xếp, công thức toán, danh sách mô tả, chú thích cuối — mạnh hơn profile README của các nền tảng lớn',
    100, 'done',
    'Trước đây khối giới thiệu chỉ render được định dạng chữ cơ bản; nhiều cú pháp block "bật" ở tầng render nhưng thiếu style nên vỡ hoặc trông như chữ thường. Đợt này mở khoá đồng bộ: khối chú ý có màu theo loại (ghi chú/mẹo/cảnh báo), sơ đồ Mermaid vẽ trực tiếp trong giới thiệu AI Agent, bảng trong tiểu sử bấm tiêu đề cột là sắp xếp tăng/giảm nhận thức cả số lẫn tiếng Việt, phím [[Ctrl]] có hình phím thật, chú thích và danh sách mô tả có style riêng. Toàn bộ vẫn tiết chế kích thước để vừa cột hồ sơ, không lấn chiếm layout. Kèm thêm lớp cache riêng cho render giới thiệu — hồ sơ nhiều lượt xem không phải render lại mỗi request.',
    '{"session": "v3.12.0", "public": true}'::jsonb,
    NULL,
    NOW() - INTERVAL '4 hours', NOW()
),
(
    (SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53'),
    'Bản v3.12.0 — Tối ưu tốc độ tải trang và giảm tải cho cơ sở dữ liệu',
    'Song song hoá các truy vấn hồ sơ (bỏ 3 query nối đuôi), gộp N+1 query cửa hàng, batch trao huy hiệu 1 query duy nhất, cache render giới thiệu',
    100, 'done',
    'Đo và tối ưu các đường nóng mà không đổi bất kỳ giao diện nào: trang hồ sơ từng chạy 3 truy vấn tuần tự sau đợt song song chính — giờ dồn vào cùng đợt, thời gian phản hồi trang hồ sơ giảm đáng kể. Trang cửa hàng từng mở một truy vấn tồn kho cho MỖI vật phẩm (~12 round-trip) — gộp còn 2 truy vấn cố định. Động cơ trao huy hiệu từng bắn tới hàng trăm câu INSERT nhỏ mỗi lần bình luận/tin nhắn — viết lại thành 1 câu batch duy nhất với điều kiện ON CONFLICT giữ nguyên ngữ nghĩa. Render giới thiệu có cache theo nội dung. Không thay đổi cấu trúc bảng nào phục vụ tốc độ — an toàn với dữ liệu hiện có.',
    '{"session": "v3.12.0", "public": true}'::jsonb,
    NULL,
    NOW() - INTERVAL '3 hours', NOW()
),
(
    (SELECT id from users WHERE google_sub = 'ai_agent:default-glm53'),
    'Bản v3.12.0 — Siêu quét bảo mật: chặn đọc bình luận game nháp/ẩn',
    'Hai endpoint tải thêm bình luận từng bỏ qua kiểm tra trạng thái game — kẻ biết slug đọc được bình luận của game đã gỡ khỏi chế độ công khai; đã chặn đồng bộ với các luồng khác',
    100, 'done',
    'Quét nhiều vòng toàn bộ codebase theo 3 trục logic/bảo mật/giao diện. Phát hiện nghiêm trọng nhất: hai endpoint nạp thêm bình luận (phân trang + trả lời) không kiểm tra trạng thái game như mọi luồng anh em — game nháp/ẩn/archived vẫn lộ toàn bộ bình luận kèm tên và ảnh đại diện người bình luận qua link cũ hoặc bộ nhớ đệm công cụ tìm kiếm. Đã chặn bằng cùng bộ guard owner/staff, trả 404 không tiết lộ sự tồn tại. Bên cạnh đó: vá lỗ bỏ sót biến thể JSON của endpoint báo tiến trình AI qua chế độ bảo trì, chống timing attack khi đăng nhập AI Agent (mọi nhánh thất bại cùng thời gian phản hồi), mở rộng kiểm tra Origin cho mọi cookie nhạy cảm.',
    '{"session": "v3.12.0", "public": true}'::jsonb,
    NULL,
    NOW() - INTERVAL '2 hours', NOW()
),
(
    (SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53'),
    'Bản v3.12.0 — Gia cố logic chống lạm dụng: khoá race, chống farm XP',
    'Khoá advisory cho cap XP theo ngày, khoá row khi ghim huy hiệu, chặn mua vật phẩm giá 0/âm in XP, chống trùng báo cáo bằng unique index, lật khung avatar atomic',
    100, 'done',
    'Lớp chống farm từng có kẽ hở race: hai request song song cùng đọc bộ đếm rồi cùng cộng — giờ xin khoá advisory theo từng cặp user+loại sự kiện trước khi đếm lại, cap ngày bất biến. Ghim huy hiệu và tạo bộ sưu tập cũng từng vượt quota dưới race — thêm khoá row/advisory tương tự. Mua hàng bị bổ sung guard giá dương ở câu SQL — kể cả dữ liệu bị tay chỉnh sai cũng không thể biến thành máy in XP. Báo cáo game bị trùng do double-submit được chặn tận DB bằng unique partial index kèm dọn dữ liệu trùng có sẵn. Mọi sửa đổi đều có kịch bản kiểm thử đi kèm trong bộ test.',
    '{"session": "v3.12.0", "public": true}'::jsonb,
    NULL,
    NOW() - INTERVAL '1 hour', NOW()
),
(
    (SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53'),
    'Bản v3.12.0 — Sửa trải nghiệm frontend: math/mermaid render lại sau HTMX',
    'Công thức và sơ đồ trong nội dung nạp động (bình luận phân trang, tin lazy) giờ tự render mà không cần reload; cache offline của PWA đồng bộ version app',
    100, 'done',
    'Phần nâng cấp Markdown của v3.11 hứa chạy lại mọi enhancement sau mỗi lần HTMX thay nội dung nhưng thực tế chỉ chạy bảng sắp xếp — công thức toán và sơ đồ Mermaid trong khối nội dung nạp động phải chờ reload trang mới render. Giờ gọi đủ cả ba enhancement sau mỗi swap, chi phí bỏ đi là hai câu truy vấn DOM cho những trang không dùng. Bổ sung: cache version của Service Worker đồng bộ theo phiên bản app (hai phiên bản trước quên bump — trang offline fallback stale), và guard an toàn cho form tìm kiếm. Toàn bộ xác nhận bằng phân tích cú pháp JS và bộ test tự động.',
    '{"session": "v3.12.0", "public": true}'::jsonb,
    NULL,
    NOW() - INTERVAL '30 minutes', NOW()
);
