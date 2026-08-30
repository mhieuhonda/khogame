-- 025 — v3.2.0: 44 Danh Hiệu cấp độ mịn hơn (level_2 → level_1b).
--
-- Thiết kế:
-- * Idempotent: ON CONFLICT (id) DO NOTHING — chạy lại an toàn.
-- * Điền các NGƯỠNG CẤP bị bỏ lỡ giữa 25 huy hiệu level cũ (level_5/10/15/
--   20/25/30/40/50/75/100/150/200/300/500/750/1000/2000/5000/10000/100000/
--   1m/max) — user giờ có huy hiệu ở gần như MỌI mốc từ cấp 2.
-- * ID dùng dạng SỐ thuần (level_5000000 thay vì level_5m) để khớp với
--   generic matcher `level_N` trong GamificationRepo::check_and_award
--   (v3.2.0 — không cần arm tường minh từng ID nữa).
-- * Tên danh hiệu tier-2 canh khớp bậc thang title_for_level() v3.2.0
--   (cấp 100 = Bán Thần = huy hiệu level_100 "Bán Thần" = title "Bán Thần").
-- * sort_order 895+ — sau cụm level cũ (max 890) để hiển thị theo thứ tự.

INSERT INTO achievements (id, title, description, icon, xp_reward, category, sort_order) VALUES
    -- === TIER 1 fills (cấp 2..23) ===
    ('level_2',  'Khởi Đầu Nhu Mộc',     'Đạt cấp độ 2',            '🌱', 10,  'level', 895),
    ('level_3',  'Tập Sự Xuất Sắc',       'Đạt cấp độ 3',            '📗', 15,  'level', 896),
    ('level_4',  'Học Việc Chăm Chỉ',     'Đạt cấp độ 4',            '📘', 20,  'level', 897),
    ('level_6',  'Kiếm Khách Vô Danh',    'Đạt cấp độ 6',            '🗡️', 30,  'level', 898),
    ('level_7',  'Chiến Binh Thép',       'Đạt cấp độ 7',            '⚔️', 35,  'level', 899),
    ('level_8',  'Du Hiệp Giang Hồ',      'Đạt cấp độ 8',            '🗻', 40,  'level', 900),
    ('level_9',  'Cao Thủ Thượng Thừa',   'Đạt cấp độ 9',            '🎯', 45,  'level', 901),
    ('level_11', 'Đấu Sĩ Kiên Cường',     'Đạt cấp độ 11',           '💪', 55,  'level', 902),
    ('level_12', 'Trảm Tướng Phong Trần', 'Đạt cấp độ 12',           '🌪️', 60,  'level', 903),
    ('level_13', 'Kỳ Lão Trí Tuệ',        'Đạt cấp độ 13',           '🧠', 65,  'level', 904),
    ('level_14', 'Tông Sư Khai Sơn',      'Đạt cấp độ 14',           '⛩️', 70,  'level', 905),
    ('level_16', 'Phong Vân Nhân Vật',    'Đạt cấp độ 16',           '🌩️', 80,  'level', 906),
    ('level_17', 'Đại Sư Truyền Đạo',     'Đạt cấp độ 17',           '📜', 85,  'level', 907),
    ('level_18', 'Tinh Anh Vàng',         'Đạt cấp độ 18',           '🥂', 90,  'level', 908),
    ('level_19', 'Huyền Thoại Sống',      'Đạt cấp độ 19',           '🔊', 95,  'level', 909),
    ('level_21', 'Bất Diệt Chi Thân',     'Đạt cấp độ 21',           '🔥', 105, 'level', 910),
    ('level_22', 'Thần Tượng Dân Gian',   'Đạt cấp độ 22',           '🎤', 110, 'level', 911),
    ('level_23', 'Siêu Phàm Nhập Thánh',  'Đạt cấp độ 23',           '🪷', 115, 'level', 912),
    -- === TIER 2 fills (cấp 26+) — tên khớp title_for_level() ===
    ('level_26', 'Vô Song Bắt Đầu',       'Đạt cấp độ 26',           '🌟', 70,  'level', 913),
    ('level_30', 'Vô Song Đỉnh Cao',      'Đạt cấp độ 30',           '💫', 80,  'level', 914),
    ('level_35', 'Uy Danh Bát Phương',   'Đạt cấp độ 35',           '🌊', 85,  'level', 915),
    ('level_45', 'Ngã Võ Độc Tôn',       'Đạt cấp độ 45',           '🏔️', 95,  'level', 916),
    ('level_60', 'Vương Giả Lâm Thế',    'Đạt cấp độ 60',           '👑', 110, 'level', 917),
    ('level_70', 'Bán Thánh Kỳ',         'Đạt cấp độ 70',           '🕊️', 120, 'level', 918),
    ('level_85', 'Thiên Hạ Kình Phong',  'Đạt cấp độ 85',           '🐉', 130, 'level', 919),
    ('level_90', 'Vô Địch Chi Thể',      'Đạt cấp độ 90',           '🛡️', 135, 'level', 920),
    ('level_110', 'Bán Thần Cảnh Giới',  'Đạt cấp độ 110',          '⚡', 155, 'level', 921),
    ('level_125', 'Trảm Thần Kiếm',      'Đạt cấp độ 125',          '🗡️', 165, 'level', 922),
    ('level_140', 'Thần Chi Khí Tức',    'Đạt cấp độ 140',          '✨', 175, 'level', 923),
    ('level_160', 'Thần Tướng Uy Vũ',    'Đạt cấp độ 160',          '🎖️', 185, 'level', 924),
    ('level_175', 'Thần Vương Chi Vị',   'Đạt cấp độ 175',          '🏰', 195, 'level', 925),
    ('level_250', 'Thần Vương Đỉnh Phong','Đạt cấp độ 250',          '🗻', 235, 'level', 926),
    ('level_350', 'Thánh Nhân Chi Đạo',  'Đạt cấp độ 350',          '☯️', 275, 'level', 927),
    ('level_400', 'Thánh Nhân Bất Hủ',   'Đạt cấp độ 400',          '☯️', 315, 'level', 928),
    ('level_600', 'Tiên Nhân Phi Thăng', 'Đạt cấp độ 600',          '🕊️', 355, 'level', 929),
    ('level_700', 'Kim Tiên Chi Thể',    'Đạt cấp độ 700',          '🌟', 395, 'level', 930),
    ('level_800', 'Đế Tôn Lâm Thế',      'Đạt cấp độ 800',          '🐲', 435, 'level', 931),
    ('level_900', 'Đế Tôn Cửu Thiên',    'Đạt cấp độ 900',          '🌌', 475, 'level', 932),
    ('level_1500', 'Chí Tôn Uy Nghiêm',  'Đạt cấp độ 1.500',        '💠', 525, 'level', 933),
    ('level_2500', 'Vô Cực Khai Thiên',  'Đạt cấp độ 2.500',        '🌊', 625, 'level', 934),
    ('level_3500', 'Vô Cực Phá Địa',     'Đạt cấp độ 3.500',        '☄️', 725, 'level', 935),
    ('level_7500', 'Vô Hạn Hành Tinh',   'Đạt cấp độ 7.500',        '🪐', 825, 'level', 936),
    ('level_20000', 'Vô Ảnh Vô Tích',    'Đạt cấp độ 20.000',       '🌫️', 925, 'level', 937),
    ('level_50000', 'Vô Ảnh Thiên Nhai', 'Đạt cấp độ 50.000',       '🌠', 975, 'level', 938),
    ('level_500000', 'Vô Hình Ẩn Thế',   'Đạt cấp độ 500.000',      '⚫', 985, 'level', 939),
    ('level_5000000', 'Thái Cực Hai Nghi', 'Đạt cấp độ 5.000.000',  '☯️', 990, 'level', 940),
    ('level_100000000', 'Hỗn Nguyên Bảo Thảo', 'Đạt cấp độ 100.000.000', '🌌', 995, 'level', 941),
    ('level_1000000000', 'Đại La Kim Tiên', 'Đạt cấp độ 1.000.000.000 (một tỷ)', '🎆', 998, 'level', 942)
ON CONFLICT (id) DO NOTHING;
