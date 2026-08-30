-- 024 — v3.1.0: BIGINT XP + 100 Danh Hiệu mới + 2 game mới
-- (Oẳn tù tì / Kéo búa bao + Nối từ).
--
-- Thiết kế:
-- * Idempotent (IF NOT EXISTS / ON CONFLICT DO NOTHING / ALTER TYPE re-safe).
-- * Tiêu backward-compatible: user_xp_totals.total_xp INT → BIGINT để hỗ
--   trợ level tối đa 500 tỷ (level_from_xp formula-based, không giới hạn
--   array nữa).
-- * 100 Danh Hiệu mới trải đều các hạng mục (level / streak / comments /
--   games / likes / downloads / followers / reviews / bookmarks / repos /
--   news / chat / collections / social / rps / word_chain) — user đạt điều
--   kiện sẽ tự động được grant (FIX bug met() return false cho ID lạ).
-- * 2 bảng game mới: rps_plays (Oẳn tù tì) + word_chain_plays (Nối từ).

-- ============================================================
-- 1) BIGINT CHO total_xp — hỗ trợ level lên tới 500 tỷ
-- ============================================================
-- INT tối đa ~2.1 tỷ — không đủ cho level 500 tỷ (cần ~5e14 XP). Chuyển
-- sang BIGINT (i64, max ~9.2e18) dư sức. CHECK >= 0 giữ nguyên.
ALTER TABLE user_xp_totals
    ALTER COLUMN total_xp TYPE BIGINT
    USING total_xp::bigint;

-- xp_events.amount giữ INT — mỗi event đơn lẻ luôn nhỏ (max 300 XP/event).
-- ALTER TABLE xp_events ALTER COLUMN amount TYPE BIGINT USING amount::bigint;

-- ============================================================
-- 2) 100 DANH HIỆU MỚI (id prefix → sort_order theo category)
-- ============================================================
-- sort_order bắt đầu từ 700+ để không đụng 25 seed cũ (max 620).
-- Category沿用 convention: level / streak / content / creator / social /
-- discovery. Thêm 2 category mới: arcade (RPS/Word Chain) + word_chain.
INSERT INTO achievements (id, title, description, icon, xp_reward, category, sort_order) VALUES
    -- === LEVEL TIERS (20) — mức level cao dần, XP thưởng tăng nhanh ===
    ('level_15',       'Chiến Binh Cao Cấp',  'Đạt cấp độ 15',                                '🥉', 60,    'level', 700),
    ('level_20',       'Bậc Lão Thành',       'Đạt cấp độ 20',                                '🥈', 90,    'level', 710),
    ('level_25',       'Hậu Phương Huyền Thoại','Đạt cấp độ 25',                              '🥇', 120,   'level', 720),
    ('level_30',       'Đại Huyền Thoại',    'Đạt cấp độ 30',                                '🏅', 160,   'level', 730),
    ('level_40',       'Vô Song',            'Đạt cấp độ 40',                                '⚔️', 220,   'level', 740),
    ('level_50',       'Thiên Hạ Đệ Nhất',    'Đạt cấp độ 50',                                '👑', 300,   'level', 750),
    ('level_75',       'Vô Địch',            'Đạt cấp độ 75',                                '🏹', 450,   'level', 760),
    ('level_100',      'Bán Thần',           'Đạt cấp độ 100',                               '🌀', 700,   'level', 770),
    ('level_150',      'Thần Chi Tướng',     'Đạt cấp độ 150',                               '✨', 1000,  'level', 780),
    ('level_200',      'Thần Vương',         'Đạt cấp độ 200',                               '🌟', 1500,  'level', 790),
    ('level_300',      'Thánh Nhân',         'Đạt cấp độ 300',                               '💫', 2500,  'level', 800),
    ('level_500',      'Tiên Nhân',          'Đạt cấp độ 500',                               '🪐', 5000,  'level', 810),
    ('level_750',      'Đế Tôn',            'Đạt cấp độ 750',                               '🌌', 10000, 'level', 820),
    ('level_1000',     'Chí Tôn',           'Đạt cấp độ 1000',                              '🔮', 20000, 'level', 830),
    ('level_2000',     'Vô Cực',            'Đạt cấp độ 2000',                              '☀️', 50000, 'level', 840),
    ('level_5000',     'Vô Hạn',            'Đạt cấp độ 5000',                              '🌠', 100000,'level', 850),
    ('level_10000',    'Vô Ảnh',            'Đạt cấp độ 10.000',                            '🌑', 250000,'level', 860),
    ('level_100000',   'Vô Hình',            'Đạt cấp độ 100.000',                           '⬛', 500000,'level', 870),
    ('level_1m',       'Thái Cực',          'Đạt cấp độ 1.000.000 (một triệu)',             '⚪', 1000000,'level', 880),
    ('level_max',      'Vô Biên',           'Đạt cấp độ tối đa — 500 TỶ!',                  '♾️', 5000000000,'level', 890),

    -- === STREAK TIERS (5) — chuỗi điểm danh dài hơn ===
    ('streak_50',      'Sắt Thép',          'Điểm danh 50 ngày liên tiếp',                   '🛡️', 400,   'streak', 900),
    ('streak_100',     'Trăm Ngày Vàng',    'Điểm danh 100 ngày liên tiếp',                  '💯', 1000,  'streak', 910),
    ('streak_365',     'Một Năm Bất Tận',   'Điểm danh 365 ngày liên tiếp — cả năm!',       '📅', 5000,  'streak', 920),
    ('streak_1000',    'Thousand Days Master','Điểm danh 1000 ngày liên tiếp',                '🏆', 20000, 'streak', 930),
    ('streak_champion','Chuỗi Huyền Thoại', 'Điểm danh tổng cộng 365 ngày trong đời',        '🌹', 8000,  'streak', 940),

    -- === COMMENTS TIERS (5) — viết nhiều bình luận ===
    ('comments_100',   'Truyền Nhân',       'Viết 100 bình luận',                            '🗨️', 100,   'content', 1000),
    ('comments_250',   'Cổ Võ Khích Lệ',    'Viết 250 bình luận',                            '📣', 250,   'content', 1010),
    ('comments_500',   'Tâm Điểm Cộng Đồng','Viết 500 bình luận',                            '🎯', 500,   'content', 1020),
    ('comments_1000',  'Bậc Thầy Giao Tiếp','Viết 1000 bình luận',                          '🎙️', 1000,  'content', 1030),
    ('comments_5000',  'Hòa Bình Thế Giới', 'Viết 5000 bình luận',                          '🕊️', 5000,  'content', 1040),

    -- === GAMES PUBLISHED (10) — xưởng game lớn dần ===
    ('games_10',       'Tiểu Xưởng Trưởng',  'Đăng 10 game lên cộng đồng',                   '🏭', 150,   'creator', 1100),
    ('games_25',       'Xưởng Trưởng',      'Đăng 25 game lên cộng đồng',                    '🏗️', 300,   'creator', 1110),
    ('games_50',       'Đại Xưởng',         'Đăng 50 game lên cộng đồng',                    '🏙️', 600,   'creator', 1120),
    ('games_100',      'Trăm Game',         'Đăng 100 game lên cộng đồng',                   '🏡', 1200,  'creator', 1130),
    ('games_250',      'Làng Game',         'Đăng 250 game lên cộng đồng',                   '🏘️', 3000,  'creator', 1140),
    ('games_500',      'Thành Game',        'Đăng 500 game lên cộng đồng',                   '🌃', 6000,  'creator', 1150),
    ('games_1000',     'Vương Quốc Game',   'Đăng 1000 game lên cộng đồng',                 '🏰', 12000, 'creator', 1160),
    ('games_2500',     'Đế Chế Game',       'Đăng 2500 game lên cộng đồng',                 '🏯', 30000, 'creator', 1170),
    ('games_5000',     'Thiên Hà Game',    'Đăng 5000 game lên cộng đồng',                  '🌉', 60000, 'creator', 1180),
    ('games_10000',    'Vũ Trụ Game',      'Đăng 10000 game lên cộng đồng',                '🌌', 120000,'creator', 1190),

    -- === LIKES RECEIVED (5) ===
    ('likes_received_100','Trăm Trái Tim',  'Game của bạn nhận tổng cộng 100 lượt thích',    '💗', 150,   'creator', 1200),
    ('likes_received_250','Hai Trăm Năm Mươi Trái Tim','Game của bạn nhận 250 lượt thích',  '💝', 300,   'creator', 1210),
    ('likes_received_500','Năm Trăm Trái Tim','Game của bạn nhận 500 lượt thích',            '💖', 600,   'creator', 1220),
    ('likes_received_1000','Nghìn Trái Tim','Game của bạn nhận 1000 lượt thích',             '❤️‍🔥', 1200,  'creator', 1230),
    ('likes_received_5000','Ngân Trái Tim', 'Game của bạn nhận 5000 lượt thích',             '💞', 6000,  'creator', 1240),

    -- === DOWNLOADS (5) ===
    ('downloads_250',  'Hai Trăm Năm Mươi Tải','Game của bạn đạt 250 lượt tải',              '📊', 150,   'creator', 1300),
    ('downloads_500',  'Nửa Nghìn Tải',     'Game của bạn đạt 500 lượt tải',                  '📈', 300,   'creator', 1310),
    ('downloads_1000', 'Nghìn Lượt Tải',    'Game của bạn đạt 1000 lượt tải',                 '🎯', 600,   'creator', 1320),
    ('downloads_5000', 'Năm Nghìn Lượt Tải','Game của bạn đạt 5000 lượt tải',                '🚀', 3000,  'creator', 1330),
    ('downloads_10000','Mười Nghìn Lượt Tải','Game của bạn đạt 10000 lượt tải',              '🌠', 6000,  'creator', 1340),

    -- === FOLLOWERS (5) ===
    ('followers_50',   'Năm Mươi Fan',      'Có 50 người theo dõi',                           '👥', 150,   'social', 1400),
    ('followers_100',  'Trăm Fan',         'Có 100 người theo dõi',                          '🌻', 300,   'social', 1410),
    ('followers_250',  'Hai Trăm Năm Mươi Fan','Có 250 người theo dõi',                       '🌼', 600,   'social', 1420),
    ('followers_500',  'Năm Trăm Fan',     'Có 500 người theo dõi',                          '🌟', 1200,  'social', 1430),
    ('followers_1000', 'Nghìn Fan Hâm Mộ', 'Có 1000 người theo dõi',                         '✨', 3000,  'social', 1440),

    -- === REVIEWS (5) ===
    ('reviews_5',      'Năm Review',        'Viết 5 review',                                  '📝', 50,    'content', 1500),
    ('reviews_10',     'Mười Review',      'Viết 10 review',                                 '📜', 100,   'content', 1510),
    ('reviews_25',     'Hai Lăm Review',   'Viết 25 review',                                '📄', 250,   'content', 1520),
    ('reviews_50',     'Năm Mươi Review',   'Viết 50 review',                                '📚', 500,   'content', 1530),
    ('reviews_100',    'Trăm Review',      'Viết 100 review',                               '🗄️', 1000,  'content', 1540),

    -- === BOOKMARKS (5) ===
    ('bookmarks_25',   'Hai Lăm Điểm',     'Lưu 25 game vào danh sách',                     '🔖', 50,    'discovery', 1600),
    ('bookmarks_50',   'Năm Mươi Điểm',    'Lưu 50 game vào danh sách',                     '📑', 100,   'discovery', 1610),
    ('bookmarks_100',  'Trăm Game Yêu Thích','Lưu 100 game vào danh sách',                  '📕', 200,   'discovery', 1620),
    ('bookmarks_250',  'Nhà Sưu Tầm Lớn',  'Lưu 250 game vào danh sách',                    '📗', 500,   'discovery', 1630),
    ('bookmarks_500',  'Thư Viện Game',    'Lưu 500 game vào danh sách',                    '📘', 1000,  'discovery', 1640),

    -- === REPOS (5) ===
    ('repos_5',        'Năm Repo',         'Chia sẻ 5 repo GitHub',                          '💻', 50,    'creator', 1700),
    ('repos_10',       'Mười Repo',        'Chia sẻ 10 repo GitHub',                         '⌨️', 100,   'creator', 1710),
    ('repos_25',       'Hai Lăm Repo',     'Chia sẻ 25 repo GitHub',                         '🖥️', 250,   'creator', 1720),
    ('repos_50',       'Năm Mươi Repo',    'Chia sẻ 50 repo GitHub',                         '🖱️', 500,   'creator', 1730),
    ('repos_100',      'Trăm Repo',        'Chia sẻ 100 repo GitHub',                        '🖨️', 1000,  'creator', 1740),

    -- === NEWS (5) ===
    ('news_5',         'Năm Bài Tin',       'Có 5 bài tin được duyệt đăng',                  '📰', 100,   'creator', 1800),
    ('news_10',        'Mười Bài Tin',      'Có 10 bài tin được duyệt đăng',                 '🗞️', 200,   'creator', 1810),
    ('news_25',        'Hai Lăm Bài Tin',   'Có 25 bài tin được duyệt đăng',                '📚', 500,   'creator', 1820),
    ('news_50',        'Năm Mươi Bài Tin',   'Có 50 bài tin được duyệt đăng',                '📋', 1000,  'creator', 1830),
    ('news_100',       'Trăm Bài Tin',     'Có 100 bài tin được duyệt đăng',                '🗂️', 2000,  'creator', 1840),

    -- === CHAT (5) ===
    ('chat_10',        'Mười Tin Nhắn',     'Gửi 10 tin nhắn chat',                           '💬', 20,    'social', 1900),
    ('chat_50',        'Năm Mươi Tin Nhắn', 'Gửi 50 tin nhắn chat',                          '💬', 50,    'social', 1910),
    ('chat_100',       'Trăm Tin Nhắn',    'Gửi 100 tin nhắn chat',                          '🗨️', 100,   'social', 1920),
    ('chat_500',       'Năm Trăm Tin Nhắn','Gửi 500 tin nhắn chat',                         '🔊', 250,   'social', 1930),
    ('chat_1000',      'Nghìn Tin Nhắn',   'Gửi 1000 tin nhắn chat',                        '📢', 500,   'social', 1940),

    -- === COLLECTIONS (5) ===
    ('collections_3',  'Ba Bộ Sưu Tập',     'Tạo 3 bộ sưu tập game',                          '📁', 30,    'discovery', 2000),
    ('collections_5',  'Năm Bộ Sưu Tập',    'Tạo 5 bộ sưu tập game',                          '🗂️', 50,    'discovery', 2010),
    ('collections_10', 'Mười Bộ Sưu Tập',   'Tạo 10 bộ sưu tập game',                        '🗃️', 100,   'discovery', 2020),
    ('collections_25', 'Hai Lăm Bộ Sưu Tập','Tạo 25 bộ sưu tập game',                        '🗄️', 250,   'discovery', 2030),
    ('collections_50', 'Năm Mươi Bộ Sưu Tập','Tạo 50 bộ sưu tập game',                      '📚', 500,   'discovery', 2040),

    -- === SOCIAL LINKS (5) — mở rộng mạng xã hội ===
    ('social_2',       'Mạng Xã Hội 2',    'Thêm 2 link mạng xã hội vào hồ sơ',              '🔗', 15,    'onboarding', 2100),
    ('social_3',       'Mạng Xã Hội 3',    'Thêm 3 link mạng xã hội vào hồ sơ',              '🔗', 20,    'onboarding', 2110),
    ('social_4',       'Mạng Xã Hội 4',    'Thêm 4 link mạng xã hội vào hồ sơ',              '🖇️', 30,    'onboarding', 2120),
    ('social_5',       'Mạng Xã Hội 5',    'Thêm 5 link mạng xã hội vào hồ sơ',              '📎', 40,    'onboarding', 2130),
    ('social_master',  'Bậc Thầy Mạng Xã Hội','Thêm 7+ link mạng xã hội vào hồ sơ',          '🌐', 100,   'onboarding', 2140),

    -- === RPS — Oẳn tù tì (5) ===
    ('rps_first_win',  'Bàn Tay Đầu Tiên',  'Thắng ván Oẳn tù tì đầu tiên',                   '✊', 10,    'arcade', 2200),
    ('rps_10_wins',    'Mười Chiến Thắng',  'Thắng 10 ván Oẳn tù tì',                         '✋', 50,    'arcade', 2210),
    ('rps_50_wins',    'Năm Mươi Chiến Thắng','Thắng 50 ván Oẳn tù tì',                       '✌️', 150,   'arcade', 2220),
    ('rps_100_wins',   'Trăm Chiến Thắng',  'Thắng 100 ván Oẳn tù tì',                       '🏆', 300,   'arcade', 2230),
    ('rps_500_wins',   'Bán Thần Oẳn Tù Tì','Thắng 500 ván Oẳn tù tì',                       '👑', 1500,  'arcade', 2240),

    -- === WORD CHAIN — Nối từ (5) ===
    ('word_chain_first','Nối Từ Đầu Tiên',  'Hoàn thành 1 lần nối từ hợp lệ',                 '🔤', 10,    'word_chain', 2300),
    ('word_chain_10',  'Mười Từ Nối',       'Hoàn thành 10 lần nối từ hợp lệ',               '🔡', 50,    'word_chain', 2310),
    ('word_chain_50',  'Năm Mươi Từ Nối',   'Hoàn thành 50 lần nối từ hợp lệ',               '🔠', 150,   'word_chain', 2320),
    ('word_chain_100', 'Trăm Từ Nối',      'Hoàn thành 100 lần nối từ hợp lệ',              '🔤', 300,   'word_chain', 2330),
    ('word_chain_500', 'Bậc Thầy Nối Từ',   'Hoàn thành 500 lần nối từ hợp lệ',              '📚', 1500,  'word_chain', 2340)
ON CONFLICT (id) DO NOTHING;

-- ============================================================
-- 3) BẢNG RPS_PLAYS — Oẳn tù tì (Kéo búa bao)
-- ============================================================
CREATE TABLE IF NOT EXISTS rps_plays (
    id           BIGSERIAL PRIMARY KEY,
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- user_choice / bot_choice: 'rock' (búa) | 'paper' (bao) | 'scissors' (kéo)
    user_choice  VARCHAR(8)  NOT NULL,
    bot_choice   VARCHAR(8)  NOT NULL,
    -- result: 'win' | 'lose' | 'draw'
    result       VARCHAR(8)  NOT NULL,
    xp_awarded   INT  NOT NULL DEFAULT 0 CHECK (xp_awarded >= 0),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_rps_plays_user_created
    ON rps_plays(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_rps_plays_user_wins
    ON rps_plays(user_id) WHERE result = 'win';

-- ============================================================
-- 4) BẢNG WORD_CHAIN_PLAYS — Nối từ
-- ============================================================
-- Thiết kế 1 play = 1 lượt user submit 1 từ. Server validate:
-- từ phải trong dictionary, phải bắt đầu bằng chữ cái kế tiếp từ bot
-- (cho session chain). Mỗi play độc lập không cần session — user gõ
-- 1 từ bất kỳ, server kiểm dictionary + thưởng XP + bot trả 1 từ mới.
CREATE TABLE IF NOT EXISTS word_chain_plays (
    id           BIGSERIAL PRIMARY KEY,
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    word         VARCHAR(100) NOT NULL,
    is_valid     BOOLEAN NOT NULL DEFAULT FALSE,
    bot_word     VARCHAR(100),
    xp_awarded   INT  NOT NULL DEFAULT 0 CHECK (xp_awarded >= 0),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_word_chain_user_created
    ON word_chain_plays(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_word_chain_user_valid
    ON word_chain_plays(user_id) WHERE is_valid = TRUE;

-- ============================================================
-- 5) INDEX BỔ TRỢ — count nhanh user_achievements theo category
-- ============================================================
-- Tăng tốc query achievement_stats (admin dashboard) khi catalog phình
-- lên 125+ huy hiệu. Không的独特 index PK đã đủ cho lookup user+ach.
CREATE INDEX IF NOT EXISTS idx_user_achievements_user_earned
    ON user_achievements(user_id, earned_at DESC);
