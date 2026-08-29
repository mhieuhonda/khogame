-- 023 — v3.0.0 RETENTION ENGINE: 20 tính năng giữ chân người dùng.
--
-- Nguyên tắc giữ đúng convention codebase (theo 021):
-- * KHÔNG thêm cột vào bảng `users` hay `user_preferences` (FromRow
--   explicit-columns của code hiện hành không bị ảnh hưởng) — mọi dữ
--   liệu mới nằm bảng riêng.
-- * Idempotent tuyệt đối (IF NOT EXISTS / ON CONFLICT DO NOTHING) —
--   re-run an toàn, Coolify deploy lại nhiều lần không lo.
-- * Chỉ CREATE TABLE / CREATE INDEX / INSERT seed — KHÔNG DROP, KHÔNG
--   ALTER bảng cũ → prod data không thể bị mất.
-- * Counter update bằng SQL thủ công trong repo (không trigger mới).

-- ============================================================
-- 1) NHIỆM VỤ HẰNG NGÀY / HẰNG TUẦN (QUESTS)
-- ============================================================
-- Catalog nhiệm vụ. stat_key do code bump (view_game, comment, rate_game,
-- like_game, chat, download, share, review, add_collection).
-- period: 'daily' | 'weekly' (tuần bắt đầu thứ 2, giờ VN).
CREATE TABLE IF NOT EXISTS quest_catalog (
    id          VARCHAR(60) PRIMARY KEY,
    title       VARCHAR(120) NOT NULL,
    description TEXT NOT NULL,
    icon        VARCHAR(16)  NOT NULL,
    stat_key    VARCHAR(40)  NOT NULL,
    target      INT NOT NULL CHECK (target >= 1),
    xp_reward   INT NOT NULL CHECK (xp_reward >= 0),
    period      VARCHAR(10) NOT NULL DEFAULT 'daily'
                CHECK (period IN ('daily', 'weekly')),
    is_active   BOOLEAN NOT NULL DEFAULT TRUE
);

-- Tiến độ của user với 1 nhiệm vụ trong 1 kỳ (ngày VN hoặc tuần thứ-2).
-- period_date: với daily = ngày; với weekly = ngày thứ 2 của tuần đó
-- (chuẩn hoá ở code) — cùng 1 PK không thể trùng lặp.
CREATE TABLE IF NOT EXISTS user_quests (
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    quest_id     VARCHAR(60) NOT NULL REFERENCES quest_catalog(id) ON DELETE CASCADE,
    period_date  DATE NOT NULL,
    progress     INT NOT NULL DEFAULT 0,
    completed_at TIMESTAMPTZ,
    claimed_at   TIMESTAMPTZ,
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, quest_id, period_date)
);
CREATE INDEX IF NOT EXISTS idx_user_quests_period
    ON user_quests(user_id, period_date);

INSERT INTO quest_catalog (id, title, description, icon, stat_key, target, xp_reward, period) VALUES
    ('d_explorer',   'Nhà Thám Hiểm',    'Xem 3 trang game bất kỳ',                    '🔍', 'view_game',    3, 15, 'daily'),
    ('d_critic',     'Nhà Phê Bình Nhỏ', 'Đánh giá 1 game bằng sao',                   '⭐', 'rate_game',    1, 10, 'daily'),
    ('d_liker',      'Trái Tim Rộng',    'Thích 3 game',                               '❤️', 'like_game',    3, 10, 'daily'),
    ('d_talker',     'Hoạt Ngôn',        'Gửi 3 tin nhắn trong Live Chat',             '💬', 'chat',         3, 10, 'daily'),
    ('d_commenter',  'Chia Sẻ Ý Kiến',   'Viết 2 bình luận',                           '🗣️', 'comment',      2, 10, 'daily'),
    ('d_downloader', 'Thợ Săn Game',     'Tải 1 game',                                 '📥', 'download',     1, 15, 'daily'),
    ('d_sharer',     'Người Lan Tỏa',    'Chia sẻ 1 game',                             '📣', 'share',        1, 15, 'daily'),
    ('d_collector',  'Người Sưu Tầm',    'Thêm 1 game vào bộ sưu tập',                 '🗂️', 'add_collection', 1, 10, 'daily'),
    ('d_reviewer',   'Review Đầu Tuần',  'Viết 1 review game',                         '📝', 'review',       1, 20, 'daily'),
    ('w_marathon',   'Marathon Tuần',    'Xem 10 game trong tuần',                     '🏃', 'view_game',   10, 60, 'weekly'),
    ('w_social',     'Linh Hồn Cộng Đồng','Viết 8 bình luận trong tuần',               '🤝', 'comment',      8, 60, 'weekly'),
    ('w_rater',      'Chuyên Gia Xếp Sao','Đánh giá 4 game trong tuần',                '🎖️', 'rate_game',    4, 50, 'weekly'),
    ('w_liker',      'Cỗ Máy Yêu Thích', 'Thích 12 game trong tuần',                   '💫', 'like_game',   12, 50, 'weekly'),
    ('w_hunter',     'Kho Báu Tuần',     'Tải 4 game trong tuần',                      '🏹', 'download',     4, 60, 'weekly'),
    ('w_critic',     'Phê Bình Gia',     'Viết 2 review trong tuần',                   '✒️', 'review',       2, 60, 'weekly'),
    ('w_spreader',   'Đại Sứ Louis',     'Chia sẻ 3 game trong tuần',                  '🌍', 'share',        3, 50, 'weekly')
ON CONFLICT (id) DO NOTHING;

-- ============================================================
-- 2) VÒNG QUAY MAY MẮN (LUCKY SPIN) — 1 lượt/ngày
-- ============================================================
CREATE TABLE IF NOT EXISTS spins (
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    spin_date  DATE NOT NULL DEFAULT (NOW() AT TIME ZONE 'Asia/Ho_Chi_Minh')::date,
    prize_xp   INT NOT NULL CHECK (prize_xp >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, spin_date)
);

-- ============================================================
-- 3) CÂU ĐỐ HẰNG NGÀY (DAILY TRIVIA)
-- ============================================================
-- options là mảng JSON 4 phần tử; correct_index 0-3.
CREATE TABLE IF NOT EXISTS trivia_questions (
    id             SERIAL PRIMARY KEY,
    question       TEXT NOT NULL,
    options        JSONB NOT NULL,
    correct_index  INT  NOT NULL CHECK (correct_index >= 0 AND correct_index <= 3),
    explanation    TEXT NOT NULL DEFAULT '',
    is_active      BOOLEAN NOT NULL DEFAULT TRUE
);

-- 1 user trả lời tối đa 1 lần cho 1 câu hỏi (chặn retry farm XP).
CREATE TABLE IF NOT EXISTS trivia_answers (
    user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    question_id   INT  NOT NULL REFERENCES trivia_questions(id) ON DELETE CASCADE,
    answer_index  INT  NOT NULL,
    is_correct    BOOLEAN NOT NULL,
    answered_date DATE NOT NULL DEFAULT (NOW() AT TIME ZONE 'Asia/Ho_Chi_Minh')::date,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, question_id)
);
CREATE INDEX IF NOT EXISTS idx_trivia_answers_date
    ON trivia_answers(user_id, answered_date);

INSERT INTO trivia_questions (question, options, correct_index, explanation) VALUES
    ('Chữ "AAA" trong ngành game chỉ game loại nào?', '["Game siêu phẩm ngân sách lớn","Game độc lập nhỏ","Game di động","Game trình duyệt"]'::jsonb, 0, 'AAA = game ngân sách khổng lồ của studio lớn.'),
    ('Thể loại "roguelike" nổi tiếng với đặc điểm nào?', '["Đồ hoạ pixel","Chết là mất, map sinh ngẫu nhiên","Chơi trực tuyến 100 người","Không có kẻ địch"]'::jsonb, 1, 'Roguelike: permadeath + procedural generation.'),
    ('Game nào thường được coi là dòng "metroidvania" đầu tiên?', '["Super Metroid","Pac-Man","Tetris","Pong"]'::jsonb, 0, 'Super Metroid đặt nền cho thể loại khám phá 2D.'),
    ('"Easter egg" trong game là gì?', '["Trứng trong game nấu ăn","Bí mật ẩn dành cho người khám phá","Tên một boss","Loại vũ khí"]'::jsonb, 1, 'Easter egg = nội dung ẩn mà dev giấu để người chơi khám phá.'),
    ('NPC là viết tắt của gì?', '["Non-Player Character","New Player Challenge","Network Play Console","Night Patrol Crew"]'::jsonb, 0, 'NPC = nhân vật không do người chơi điều khiển.'),
    ('"Speedrun" nghĩa là gì?', '["Chạy nhanh trong game đua","Chơi hoàn thành game trong thời gian ngắn nhất","Nạp tiền nhiều nhất","Chơi 10 game cùng lúc"]'::jsonb, 1, 'Speedrun là chơi hết game nhanh nhất có thể.'),
    ('Thể loại nào đúng với game trồng trọt chăn nuôi kiểu "Stardew Valley"?', '["Farm sim","Roguelike","MOBA","Rhythm"]'::jsonb, 0, 'Stardew Valley thuộc thể loại farm simulation.'),
    ('"HP" trong game thường có nghĩa là gì?', '["Hit Points / Health Points","High Performance","Happy Point","Hero Power"]'::jsonb, 0, 'HP = lượng máu/khỏe của nhân vật.'),
    ('"Open world" chỉ loại game nào?', '["Thế giới mở tự do khám phá","Chỉ chơi online","Màn hình mở rộng","Game mở hộp"]'::jsonb, 0, 'Open world = người chơi tự do di chuyển khám phá.'),
    ('Indie game là game do ai làm?', '["Độc lập, không thuộc phát hành viên lớn","Bộ quốc phòng","Chỉ Nintendo","Nhà nước"]'::jsonb, 0, 'Indie = independent, studio nhỏ tự chủ.'),
    ('"DLC" là viết tắt của gì?', '["Downloadable Content","Data Link Control","Direct Launch Code","Double Level Combo"]'::jsonb, 0, 'DLC = nội dung tải thêm sau khi phát hành.'),
    ('"Bug" trong game là gì?', '["Con bọ trong game","Lỗi phần mềm","Nhân vật phụ","Loại vũ khí"]'::jsonb, 1, 'Bug là lỗi lập trình gây hành vi sai.'),
    ('"Beta test" là giai đoạn nào?', '["Thử nghiệm trước khi phát hành chính thức","Bán chính thức","Tải về miễn phí","Trả tiền trước"]'::jsonb, 0, 'Beta = giai đoạn thử nghiệm rộng trước release.'),
    ('Game "sandbox" cho phép gì?', '["Tự do sáng tạo, không theo cốt truyện cứng","Chỉ chơi trong hộp cát ảo","Chơi trên điện thoại","Chơi với cờ vua"]'::jsonb, 0, 'Sandbox = thế giới cho phép tự do sáng tạo.'),
    ('"Buff" trong game nghĩa là gì?', '["Tăng chỉ số tạm thời","Gậy gỗ","Loại quái","Màn hình phụ"]'::jsonb, 0, 'Buff = hiệu ứng tăng mạnh chỉ số.'),
    ('"Grinding" trong game RPG là gì?', '["Làm việc lặp đi lặp lại để nhận thưởng","Xay bột","Nhảy dây","Đá bóng"]'::jsonb, 0, 'Grinding = farm lặp để lên cấp/tài nguyên.')
ON CONFLICT DO NOTHING;

-- ============================================================
-- 4) CỬA HÀNG XP (SHOP) + TỒN KHO + BOOST
-- ============================================================
CREATE TABLE IF NOT EXISTS shop_items (
    id          VARCHAR(40) PRIMARY KEY,
    name        VARCHAR(120) NOT NULL,
    description TEXT NOT NULL,
    icon        VARCHAR(16) NOT NULL,
    price       INT NOT NULL CHECK (price >= 0),
    kind        VARCHAR(30) NOT NULL
                CHECK (kind IN ('streak_freeze', 'xp_boost', 'name_glow', 'mystery_box')),
    is_active   BOOLEAN NOT NULL DEFAULT TRUE
);

INSERT INTO shop_items (id, name, description, icon, price, kind) VALUES
    ('streak_freeze', 'Streak Freeze ❄️',   'Bảo vệ chuỗi điểm danh 1 lần khi bạn quên. Tự động kích hoạt ở lần điểm danh kế tiếp.', '🧊', 60,  'streak_freeze'),
    ('xp_boost',      'XP Boost x2 (24h)',  'Nhân đôi mọi XP nhận được trong 24 giờ —Leo cấp nhanh gấp đôi.',                        '⚡', 120, 'xp_boost'),
    ('name_glow',     'Viền Tên 30 Ngày',   'Tên trong Live Chat phát sáng vàng rực rỡ trong 30 ngày — ai cũng nhìn thấy.',          '✨', 100, 'name_glow'),
    ('mystery_box',   'Hộp Bí Ẩn 🎁',       'Mở ra nhận ngẫu nhiên 10–150 XP. Có when cười lớn, có khi... đen đủi!',                 '🎁', 45,  'mystery_box')
ON CONFLICT (id) DO NOTHING;

-- Tồn kho vật phẩm tiêu dùng (streak_freeze: mua nhiều lần cộng dồn).
CREATE TABLE IF NOT EXISTS user_inventory (
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id    VARCHAR(40) NOT NULL REFERENCES shop_items(id) ON DELETE CASCADE,
    quantity   INT NOT NULL DEFAULT 0 CHECK (quantity >= 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, item_id)
);

-- Boost có thời hạn (1 row/user — mua lại gia hạn bằng GREATEST).
CREATE TABLE IF NOT EXISTS user_boosts (
    user_id         UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    xp_boost_until  TIMESTAMPTZ,
    name_glow_until TIMESTAMPTZ,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================
-- 5) STREAK FREEZE ĐÃ DÙNG CHO NGÀY NÀO (chống double-consume)
-- ============================================================
CREATE TABLE IF NOT EXISTS streak_freeze_usage (
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    freeze_date DATE NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, freeze_date)
);

-- ============================================================
-- 6) REFERRAL — GIỚI THIỆU BẠN BÈ
-- ============================================================
CREATE TABLE IF NOT EXISTS user_referral_codes (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    code    VARCHAR(20) NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_referral_codes_code ON user_referral_codes(code);

CREATE TABLE IF NOT EXISTS referrals (
    referred_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    referrer_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_referrals_referrer ON referrals(referrer_id);

-- ============================================================
-- 7) HEATMAP HOẠT ĐỘNG (activity per day, GitHub-style 90 ngày)
-- ============================================================
CREATE TABLE IF NOT EXISTS user_activity_days (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    day     DATE NOT NULL DEFAULT (NOW() AT TIME ZONE 'Asia/Ho_Chi_Minh')::date,
    activity_count INT NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, day)
);

-- ============================================================
-- 8) TÙY CHỌN THÔNG BÁO (in-app per-type + digest tuần)
-- ============================================================
-- Vắng row = bật tất cả (default TRUE) — không phá hành vi hiện tại.
CREATE TABLE IF NOT EXISTS user_notification_prefs (
    user_id        UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    inapp_follow   BOOLEAN NOT NULL DEFAULT TRUE,
    inapp_new_game BOOLEAN NOT NULL DEFAULT TRUE,
    inapp_review   BOOLEAN NOT NULL DEFAULT TRUE,
    inapp_mention  BOOLEAN NOT NULL DEFAULT TRUE,
    weekly_digest  BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================
-- 9) ANNIVERSARY — thưởng sinh nhật tài khoản (chống trùng bằng PK)
-- ============================================================
CREATE TABLE IF NOT EXISTS anniversaries_awarded (
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    years      INT NOT NULL CHECK (years >= 1),
    awarded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, years)
);

-- ============================================================
-- 10) ONBOARDING CHECKLIST — 5 bước đầu cho người mới
-- ============================================================
CREATE TABLE IF NOT EXISTS onboarding_steps (
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    step         VARCHAR(30) NOT NULL,
    completed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, step)
);

-- ============================================================
-- 11) INDEX phụ trợ cho leaderboard mùa/tháng + tuần (xp_events quét
--     theo thời gian — trước đây chỉ có index (user_id, created_at)).
-- ============================================================
CREATE INDEX IF NOT EXISTS idx_xp_events_created
    ON xp_events (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_user_activity_days_day
    ON user_activity_days (day);
