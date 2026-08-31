-- ============================================================================
-- Migration 036 — v3.7.0: MỞ RỘNG CỬA HÀNG + KHUNG AVATAR (nhiều kiểu)
-- ============================================================================
-- Mục tiêu (yêu cầu v3.7.0):
--   1) Thêm nhiều vật phẩm mới vào cửa hàng XP (giá đa tầng 35 → 5000 XP).
--   2) Thêm loại vật phẩm KHUNG AVATAR (avatar_frame) — trang trí viền
--      avatar trên hồ sơ + header + live chat. ĐẶC BIỆT: khung RỒNG LỬA
--      (frame_dragon_fire, 5000 XP) — vật phẩm đắt nhất cửa hàng.
--
-- Thiết kế:
--   * shop_items.kind mở rộng CHECK thêm 'avatar_frame' (idempotent —
--     drop + add lại constraint, tên constraint mặc định của Postgres là
--     shop_items_kind_check).
--   * shop_items.duration_hours: thời hạn hiệu lực (giờ) cho các kind
--     có thời gian (xp_boost / name_glow / avatar_frame). Layer Rust đọc
--     cột này để gia hạn đúng — migration cũ 024h cứng không còn.
--   * user_boosts thêm avatar_frame (id item đang kích hoạt) +
--     avatar_frame_until (hạn). Mua lại → GREATEST(now, hiện_tại) +
--     duration (cùng pattern name_glow). Mua khung khác → frame mới
--     THAY THẾ frame cũ (người dùng chủ động chọn) — không cộng dồn.
--
-- An toàn: toàn bộ UPDATE/INSERT idempotent (ON CONFLICT DO NOTHING /
-- WHERE guard) — chạy lại không lỗi, không nhân bản dữ liệu.
-- ============================================================================

-- ---------------------------------------------------------------------------
-- 1) Nới lỏng CHECK kind: thêm 'avatar_frame'
-- ---------------------------------------------------------------------------
ALTER TABLE shop_items DROP CONSTRAINT IF EXISTS shop_items_kind_check;
ALTER TABLE shop_items
  ADD CONSTRAINT shop_items_kind_check
  CHECK (kind IN ('streak_freeze', 'xp_boost', 'name_glow', 'mystery_box', 'avatar_frame'));

-- ---------------------------------------------------------------------------
-- 2) Cột duration_hours (giờ) — dùng bởi layer Rust khi gia hạn
-- ---------------------------------------------------------------------------
ALTER TABLE shop_items
  ADD COLUMN IF NOT EXISTS duration_hours INT NOT NULL DEFAULT 24;

-- Đồng bộ vật phẩm cũ:
--   name_glow seed 023 = "30 ngày" (720h) — trước đây Rust hardcode 30d.
UPDATE shop_items SET duration_hours = 720 WHERE id = 'name_glow';
--   xp_boost seed 023 = 24h (mặc định đúng).
UPDATE shop_items SET duration_hours = 24  WHERE id = 'xp_boost';

-- ---------------------------------------------------------------------------
-- 3) user_boosts: khung avatar đang kích hoạt + hạn
-- ---------------------------------------------------------------------------
ALTER TABLE user_boosts ADD COLUMN IF NOT EXISTS avatar_frame       VARCHAR(40);
ALTER TABLE user_boosts ADD COLUMN IF NOT EXISTS avatar_frame_until TIMESTAMPTZ;

-- ---------------------------------------------------------------------------
-- 4) VẬT PHẨM MỚI (12 item — giá đa tầng, đủ mọi ví XP)
--    Lưu ý giá: mystery_box seed 023 (45) đã được migration 032 sửa 100 —
--    KHÔNG đụng lại ở đây (032 là nguồn sự thật).
-- ---------------------------------------------------------------------------
INSERT INTO shop_items (id, name, description, icon, price, kind, duration_hours) VALUES
    -- Booster / tiêu dùng bổ sung
    ('name_glow_7',    'Viền Tên 7 Ngày',            'Tên trong Live Chat phát sáng vàng trong 7 ngày — thử nghiệm giá mềm trước khi nâng cấp bản 30 ngày.', '✨', 35,  'name_glow',   168),
    ('xp_boost_3d',    'XP Boost x2 (3 Ngày)',       'Nhân đôi mọi XP trong 72 giờ — cày cấp dã chiến 3 ngày 2 đêm.',                                        '🚀', 280, 'xp_boost',    72),
    -- KHUNG AVATAR — vòng đời 30 ngày, mua lại gia hạn
    ('frame_bronze',   'Khung Avatar Đồng',          'Viền kim loại đồng ấm áp — bước đầu tiên vào giới quý tộc avatar.',                                    '🥉', 150, 'avatar_frame', 720),
    ('frame_silver',   'Khung Avatar Bạc',           'Viền bạc sáng lạnh, ánh kim tinh tế — sang trọng hơn người thường một bậc.',                           '🥈', 300, 'avatar_frame', 720),
    ('frame_gold',     'Khung Avatar Vàng',          'Viền vàng rực 24K, glow mềm — biểu tượng đẳng cấp cộng đồng.',                                         '🥇', 600, 'avatar_frame', 720),
    ('frame_neon',     'Khung Avatar Neon',          'Vòng neon cyan–magenta nhấp nháy kiểu cyberpunk — nhìn một cái là nhớ.',                               '💫', 900, 'avatar_frame', 720),
    ('frame_phoenix',  'Khung Avatar Phượng Hoàng',  'Viền lửa phượng hoàng cam–đỏ bùng cháy — tái sinh từ tro tàn mỗi lần đăng nhập.',                      '🔥', 1500, 'avatar_frame', 720),
    ('frame_dragon_fire', 'Khung Avatar Rồng Lửa 🐲', 'VIỀN RỒNG LỬA HUYỀN THOẠI — vòng lửa 7 màu xoay tròn, vảy rồng ánh kim, hào quang bùng nổ. Vật phẩm ĐẮT NHẤT cửa hàng — chỉ dành cho người huyền thoại.', '🐲', 5000, 'avatar_frame', 720)
ON CONFLICT (id) DO NOTHING;

-- ---------------------------------------------------------------------------
-- 5) Ghi chú vận hành
--    * Frames KHÔNG vào user_inventory (khác streak_freeze) — state kích
--      hoạt nằm ở user_boosts.avatar_frame(_until), 1 khung active/user.
--    * Layer Rust (ShopRepo::buy) đọc duration_hours để gia hạn — nếu
--      admin chỉnh duration_hours trên DB, hiệu lực thay đổi tương ứng.
--    * Khung render CSS thuần (style.css class .avatar-frame-<id>) —
--      không ảnh hưởng tới ảnh upload của user, không JS.
-- ============================================================================
