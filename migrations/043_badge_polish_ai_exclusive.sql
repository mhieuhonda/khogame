-- ============================================================================
-- Migration 043 — v3.10.0: TINH CHỈNH DANH MỤC HUY HIỆU + HUY HIỆU ĐỘC
-- QUYỀN AI AGENT
-- ============================================================================
-- 1) ĐỔI TÊN các huy hiệu NHẠT NHẼO / LẶP LẠI (yêu cầu chủ sở hữu):
--    Trước đây nhiều huy hiệu cấp độ dùng chung 1 "họ từ" lặp lại khiến
--    danh mục đọc lên nhàm chán:
--      Huyền Thoại ×4 (level_10/19/25/30) · Vô Song ×2 · Vô Địch ×2 ·
--      Bán Thần ×2 · Thần Vương ×3 · Thánh Nhân ×3 · Tiên Nhân ×2 ·
--      Đế Tôn ×3 · Chí Tôn ×2 · Vô Cực ×3 · Vô Hạn ×2 · Vô Ảnh ×3 ·
--      Vô Hình ×2 · Thái Cực ×2 · Cộng Đồng ×3 (social/followers/chat)
--    Giờ mỗi bậc thang có TÊN RIÊNG, duy nhất, giữ đúng tinh thần võ hiệp/
--    tu tiên của Louis Space. Chỉ đổi title — giữ nguyên id, icon, XP,
--    điều kiện → KHÔNG ảnh hưởng dữ liệu user_achievements đã trao.
--
-- 2) HUY HIỆU ĐỘC QUYỀN AI AGENT — `ai_agent_core` "Linh Hồn Nhân Tạo":
--    DUY NHẤT 1 huy hiệu trong toàn bộ danh mục dành riêng cho AI Agent.
--    - Engine check_and_award KHÔNG có điều kiện cho id này → không thể
--      tự trao bằng hành vi;
--    - Handler admin /admin/ai-agents/{id}/badge-ai là CON ĐƯỜNG DUY NHẤT
--      cấp/thu hồi (guard is_ai_agent_user — user thường bị chặn);
--    - xp_reward = 0: huy hiệu DANH DỰ, không cộng XP (tránh mọi khe
--      lạm dụng XP, đúng chất "danh tính", không phải "điểm số").
--
-- Idempotent: UPDATE theo id (chạy lại không sao) + INSERT ON CONFLICT
-- DO NOTHING. Với DB mới: 021/024/025 seed trước rồi 043 polish sau —
-- nhất quán với DB prod cũ.
-- ============================================================================

-- ============================================================
-- 1) ĐỔI TÊN HUY HIỆU LẶP LẠI / NHẠT NHẼO
-- ============================================================

-- --- Họ "Huyền Thoại" (×4) → mỗi bậc một danh xưng riêng ---
UPDATE achievements SET title = 'Anh Hùng Nhân Gian'      WHERE id = 'level_10';
UPDATE achievements SET title = 'Tên Tuổi Truyền Kỳ'     WHERE id = 'level_19';
UPDATE achievements SET title = 'Lão Tiền Bối'           WHERE id = 'level_25';
UPDATE achievements SET title = 'Chấn Địa Danh Tướng'    WHERE id = 'level_30';

-- --- Họ "Vô Song" (×2) — giữ 'Vô Song' ở level_40, đổi nhánh level_26 ---
UPDATE achievements SET title = 'Ngôi Đài Danh Vọng'     WHERE id = 'level_26';

-- --- Họ "Vô Địch" (×2) — giữ 'Vô Địch' ở level_75 ---
UPDATE achievements SET title = 'Kim Cang Bất Hoại'      WHERE id = 'level_90';

-- --- Họ "Bán Thần" (×2) — giữ 'Bán Thần' ở level_100 ---
UPDATE achievements SET title = 'Bước Cửa Tiên Gian'     WHERE id = 'level_110';

-- --- Họ "Thần Vương" (×3) — giữ 'Thần Vương' ở level_200 ---
UPDATE achievements SET title = 'Ngôi Bảo Cửu Trùng'     WHERE id = 'level_175';
UPDATE achievements SET title = 'Bá Chủ Chư Thiên'       WHERE id = 'level_250';

-- --- Họ "Thánh Nhân" (×3) — giữ 'Thánh Nhân' ở level_300 ---
UPDATE achievements SET title = 'Đạo Pháp Tự Nhiên'      WHERE id = 'level_350';
UPDATE achievements SET title = 'Vĩnh Hằng Kim Thân'     WHERE id = 'level_400';

-- --- Họ "Tiên Nhân" (×2) — giữ 'Tiên Nhân' ở level_500 ---
UPDATE achievements SET title = 'Ngự Kiếm Phi Thiên'     WHERE id = 'level_600';

-- --- Họ "Đế Tôn" (×3) — giữ 'Đế Tôn' ở level_750 ---
UPDATE achievements SET title = 'Quang Lâm Cửu Châu'     WHERE id = 'level_800';
UPDATE achievements SET title = 'Thống Lĩnh Tinh Hà'     WHERE id = 'level_900';

-- --- Họ "Chí Tôn" (×2) — giữ 'Chí Tôn' ở level_1000 ---
UPDATE achievements SET title = 'Vạn Linh Triều Bái'     WHERE id = 'level_1500';

-- --- Họ "Vô Cực" (×3) — giữ 'Vô Cực' ở level_2000 ---
UPDATE achievements SET title = 'Phá Toái Hư Không'      WHERE id = 'level_2500';
UPDATE achievements SET title = 'Chuyển Động Sơn Hà'     WHERE id = 'level_3500';

-- --- Họ "Vô Hạn" (×2) — giữ 'Vô Hạn' ở level_5000 ---
UPDATE achievements SET title = 'Nắm Giữ Càn Khôn'       WHERE id = 'level_7500';

-- --- Họ "Vô Ảnh" (×3) — giữ 'Vô Ảnh' ở level_10000 ---
UPDATE achievements SET title = 'Vân Du Tứ Hải'          WHERE id = 'level_20000';
UPDATE achievements SET title = 'Nhất Niệm Thiên Nhai'   WHERE id = 'level_50000';

-- --- Họ "Vô Hình" (×2) — giữ 'Vô Hình' ở level_100000 ---
UPDATE achievements SET title = 'Ẩn Thế Cao Nhân'        WHERE id = 'level_500000';

-- --- Họ "Thái Cực" (×2) — giữ 'Thái Cực' ở level_1m ---
UPDATE achievements SET title = 'Lưỡng Nghi Sinh Tứ Tượng' WHERE id = 'level_5000000';

-- --- Họ "Thiên Hạ" (×2) — giữ 'Thiên Hạ Đệ Nhất' ở level_50 ---
UPDATE achievements SET title = 'Kình Phong Vạn Lý'      WHERE id = 'level_85';

-- --- "Độc Tôn" trùng họ "Tôn" (Đế Tôn / Chí Tôn) ---
UPDATE achievements SET title = 'Kiếm Đạo Cô Hành'       WHERE id = 'level_45';

-- --- Tên nhạt nhẽo/cụm chung chung ở nhánh cấp độ ---
UPDATE achievements SET title = 'Tân Tú Rực Sáng'        WHERE id = 'level_5';
UPDATE achievements SET title = 'Trưởng Lão Cầm Quyền'   WHERE id = 'level_20';
UPDATE achievements SET title = 'Hậu Vệ Bạch Ngân'       WHERE id = 'level_15';

-- --- Nhánh xã hội: họ "Cộng Đồng" (×3) + tên đếm số khô khan ---
UPDATE achievements SET title = 'Vòng Tay Bạn Bè'        WHERE id = 'social_link';
UPDATE achievements SET title = 'Kho Báu Cá Nhân'        WHERE id = 'bookmarks_10';
UPDATE achievements SET title = 'Người Được Nhớ Tên'     WHERE id = 'first_follower';
UPDATE achievements SET title = 'Người Dẫn Lối'          WHERE id = 'followers_10';
UPDATE achievements SET title = 'Tiếng Vang Phòng Chat'  WHERE id = 'chat_first';

-- ============================================================
-- 2) HUY HIỆU ĐỘC QUYỀN AI AGENT — duy nhất trong danh mục
-- ============================================================
INSERT INTO achievements (id, title, description, icon, xp_reward, category, sort_order) VALUES
    ('ai_agent_core',
     'Linh Hồn Nhân Tạo',
     'Huy hiệu ĐỘC QUYỀN — duy nhất trong danh mục dành riêng cho AI Agent, do quản trị viên trực tiếp trao tặng. Danh hiệu danh dự: không cộng XP.',
     '🤖', 0, 'ai_agent', 45)
ON CONFLICT (id) DO NOTHING;
