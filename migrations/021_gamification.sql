-- 021 — v2.9.0 GAMIFICATION ENGINE: XP + cấp độ + điểm danh chuỗi +
-- huy hiệu + bộ sưu tập game + lịch sử xem + vote helpful cho review.
--
-- Thiết kế giữ đúng convention codebase:
-- * KHÔNG thêm cột vào bảng `users` (FromRow<User> explicit-columns sẽ
--   vỡ runtime) — mọi dữ liệu mới nằm bảng riêng 1-1 hoặc N-1.
-- * Idempotent (IF NOT EXISTS / ON CONFLICT DO NOTHING) — re-run an toàn.
-- * Counter `collections.game_count` + `user_xp_totals.total_xp` update
--   bằng SQL thủ công trong repo (không trigger) — các bảng mới không
--   được share trigger cũ.

-- ============================================================
-- 1) XP TỔNG HỢP (cache) + NHẬT KÝ XP (activity log kép)
-- ============================================================
-- user_xp_totals: cache tổng XP để đọc O(1) (chip cấp độ render ở mọi
-- comment/chat/profile — không thể SUM(xp_events) mỗi lần).
CREATE TABLE IF NOT EXISTS user_xp_totals (
    user_id     UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    total_xp    INT  NOT NULL DEFAULT 0 CHECK (total_xp >= 0),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- xp_events: append-only log mỗi lần cộng XP. Kiêm activity feed trên
-- hồ sơ (reason đủ ngữ nghĩa để render "đã đăng game", "đã bình luận"…).
CREATE TABLE IF NOT EXISTS xp_events (
    id         BIGSERIAL PRIMARY KEY,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    reason     VARCHAR(50) NOT NULL,
    amount     INT  NOT NULL,
    ref_id     UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_xp_events_user_created
    ON xp_events(user_id, created_at DESC);
-- Đếm XP theo loại trong ngày (anti-farm cap) — dùng (user_id, reason, created_at)
CREATE INDEX IF NOT EXISTS idx_xp_events_user_reason_created
    ON xp_events(user_id, reason, created_at DESC);

-- ============================================================
-- 2) ĐIỂM DANH HÀNG NGÀY + CHUỖI (STREAK)
-- ============================================================
CREATE TABLE IF NOT EXISTS daily_checkins (
    user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    checkin_date  DATE NOT NULL DEFAULT CURRENT_DATE,
    streak        INT  NOT NULL DEFAULT 1 CHECK (streak >= 1),
    xp_awarded    INT  NOT NULL DEFAULT 0,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, checkin_date)
);
-- Bảng xếp hạng chuỗi dài nhất / đếm điểm danh hôm nay
CREATE INDEX IF NOT EXISTS idx_checkins_streak ON daily_checkins(streak DESC);

-- ============================================================
-- 3) HUY HIỆU (ACHIEVEMENTS)
-- ============================================================
CREATE TABLE IF NOT EXISTS achievements (
    id          VARCHAR(50) PRIMARY KEY,
    title       VARCHAR(100) NOT NULL,
    description TEXT NOT NULL,
    icon        VARCHAR(16)  NOT NULL,
    xp_reward   INT NOT NULL DEFAULT 0,
    category    VARCHAR(30) NOT NULL DEFAULT 'general',
    sort_order  INT NOT NULL DEFAULT 100,
    is_active   BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TABLE IF NOT EXISTS user_achievements (
    user_id        UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    achievement_id VARCHAR(50) NOT NULL REFERENCES achievements(id) ON DELETE CASCADE,
    -- v2.9.0 showcase: user chọn tối đa 3 huy hiệu ghim lên hồ sơ
    is_showcased   BOOLEAN NOT NULL DEFAULT FALSE,
    earned_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, achievement_id)
);
CREATE INDEX IF NOT EXISTS idx_user_achievements_achievement
    ON user_achievements(achievement_id);

-- ============================================================
-- 4) BỘ SƯU TẬP GAME (COLLECTIONS)
-- ============================================================
CREATE TABLE IF NOT EXISTS collections (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title       VARCHAR(100) NOT NULL,
    description VARCHAR(300) NOT NULL DEFAULT '',
    is_public   BOOLEAN NOT NULL DEFAULT TRUE,
    game_count  INT NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_collections_user ON collections(user_id, created_at DESC);

CREATE TABLE IF NOT EXISTS collection_games (
    collection_id UUID NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    game_id       UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    added_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (collection_id, game_id)
);
CREATE INDEX IF NOT EXISTS idx_collection_games_game ON collection_games(game_id);

-- ============================================================
-- 5) LỊ SỬ XEM GAME ("Tiếp tục xem")
-- ============================================================
CREATE TABLE IF NOT EXISTS view_history (
    user_id   UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    game_id   UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    viewed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, game_id)
);
CREATE INDEX IF NOT EXISTS idx_view_history_recent ON view_history(user_id, viewed_at DESC);

-- ============================================================
-- 6) VOTE "HỮU ÍCH" CHO REVIEW (bảng reviews đã có từ 001,
--    helpful_count đã có — thiếu bảng vote để chống double-vote)
-- ============================================================
CREATE TABLE IF NOT EXISTS review_helpful_votes (
    user_id   UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    review_id UUID NOT NULL REFERENCES reviews(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, review_id)
);

-- ============================================================
-- 7) SEED CATALOG 25 HUY HIỆU (ON CONFLICT DO NOTHING — có thể edit
--    tay trong DB, seed không ghi đè)
-- ============================================================
INSERT INTO achievements (id, title, description, icon, xp_reward, category, sort_order) VALUES
    ('first_login',      'Khởi Đầu Hoàn Hảo',    'Đăng nhập lần đầu tiên vào Louis Space',            '🚀', 10,  'onboarding', 10),
    ('profile_avatar',   'Có Mặt Trên Bản Đồ',   'Cập nhật ảnh đại diện cá nhân',                      '📸', 10,  'onboarding', 20),
    ('profile_bio',      'Câu Chuyện Của Tôi',   'Viết vài dòng giới thiệu về bản thân trên hồ sơ',    '✍️', 10,  'onboarding', 30),
    ('social_link',      'Kết Nối Cộng Đồng',    'Thêm ít nhất một link mạng xã hội vào hồ sơ',        '🔗', 10,  'onboarding', 40),
    ('first_comment',    'Tiếng Nói Đầu Tiên',   'Viết bình luận đầu tiên',                            '💬', 10,  'content', 110),
    ('comments_10',      'Thành Viên Năng Nổ',   'Viết 10 bình luận',                                  '🗣️', 20,  'content', 120),
    ('comments_50',      'Bậc Thầy Trò Chuyện',  'Viết 50 bình luận',                                  '🎤', 50,  'content', 130),
    ('first_review',     'Nhà Phê Bình',         'Viết review đầu tiên cho một game',                  '⭐', 25,  'content', 140),
    ('first_game',       'Nhà Sáng Tạo',         'Đăng game đầu tiên lên Louis Space',                 '🎮', 50,  'creator', 210),
    ('games_5',          'Xưởng Game',           'Đăng 5 game lên cộng đồng',                          '🏭', 100, 'creator', 220),
    ('repo_first',       'Mã Nguồn Mở',          'Chia sẻ repo GitHub đầu tiên',                       '💻', 20,  'creator', 230),
    ('news_first',       'Phóng Viên Công Dân',  'Có bài tin đầu tiên được duyệt đăng',                '📰', 40,  'creator', 240),
    ('likes_received_50','50 Trái Tim',          'Game của bạn nhận tổng cộng 50 lượt thích',          '💖', 100, 'creator', 250),
    ('downloads_100',    'Ngôi Sao Đang Lên',    'Game của bạn đạt tổng 100 lượt tải',                 '📈', 100, 'creator', 260),
    ('first_like_given', 'Người Ủng Hộ',         'Thích một game lần đầu tiên',                        '❤️', 5,   'discovery', 310),
    ('first_bookmark',   'Nhà Sưu Tầm',          'Lưu game đầu tiên vào danh sách',                    '🔖', 5,   'discovery', 320),
    ('bookmarks_10',     'Bộ Sưu Tập 10 Game',   'Lưu 10 game vào danh sách của mình',                '📚', 30,  'discovery', 330),
    ('first_follower',   'Được Quan Tâm',        'Có người theo dõi đầu tiên',                         '👥', 15,  'social', 410),
    ('followers_10',     'Ngôi Sao Cộng Đồng',   'Có 10 người theo dõi',                               '🌟', 50,  'social', 420),
    ('chat_first',       'Người Của Cộng Đồng',  'Gửi tin nhắn đầu tiên trong chat',                   '🕹️', 10,  'social', 430),
    ('streak_3',         'Chuỗi Khởi Động',      'Điểm danh 3 ngày liên tiếp',                         '🔥', 30,  'streak', 510),
    ('streak_7',         'Tuần Vàng',            'Điểm danh 7 ngày liên tiếp',                         '🗓️', 70,  'streak', 520),
    ('streak_30',        'Bất Khuất',            'Điểm danh 30 ngày liên tiếp — kỷ luật thép!',        '👑', 300, 'streak', 530),
    ('level_5',          'Cao Thủ',              'Đạt cấp độ 5',                                       '🏅', 50,  'level', 610),
    ('level_10',         'Huyền Thoại',          'Đạt cấp độ 10',                                      '🏆', 200, 'level', 620)
ON CONFLICT (id) DO NOTHING;
