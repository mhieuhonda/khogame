-- ============================================
-- Kho Game - Database Schema (PostgreSQL 17)
-- ============================================

-- Extensions
-- uuid-ossp không cần nữa: dùng gen_random_uuid() built-in từ PG13+
CREATE EXTENSION IF NOT EXISTS "pg_trgm";

-- ============================================
-- USERS
-- ============================================
CREATE TYPE user_role AS ENUM ('user', 'moderator', 'admin');
CREATE TYPE auth_provider AS ENUM ('google');

CREATE TABLE users (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email         VARCHAR(255) NOT NULL UNIQUE,
    username      VARCHAR(50) NOT NULL UNIQUE,
    display_name  VARCHAR(100) NOT NULL,
    avatar_url    TEXT,
    bio           TEXT DEFAULT '',
    google_sub    VARCHAR(255) UNIQUE NOT NULL,
    role          user_role NOT NULL DEFAULT 'user',
    provider      auth_provider NOT NULL DEFAULT 'google',
    is_banned     BOOLEAN NOT NULL DEFAULT FALSE,
    last_seen_at  TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_users_google_sub ON users(google_sub);
CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_created_at ON users(created_at DESC);

-- ============================================
-- CATEGORIES & TAGS
-- ============================================
CREATE TABLE categories (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        VARCHAR(50) NOT NULL UNIQUE,
    slug        VARCHAR(60) NOT NULL UNIQUE,
    description TEXT DEFAULT '',
    icon        VARCHAR(50) DEFAULT '',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE tags (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name       VARCHAR(50) NOT NULL UNIQUE,
    slug       VARCHAR(60) NOT NULL UNIQUE,
    usage_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_tags_slug ON tags(slug);
CREATE INDEX idx_tags_usage ON tags(usage_count DESC);

-- ============================================
-- GAMES
-- ============================================
CREATE TYPE game_status AS ENUM ('draft', 'published', 'archived', 'hidden', 'pending_review');
CREATE TYPE platform_type AS ENUM ('android', 'ios', 'windows', 'linux', 'macos');
CREATE TYPE age_rating AS ENUM ('everyone', 'teen', 'mature', 'adult');

CREATE TABLE games (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title           VARCHAR(200) NOT NULL,
    slug            VARCHAR(220) NOT NULL UNIQUE,
    excerpt         VARCHAR(500) DEFAULT '',
    content         TEXT NOT NULL DEFAULT '',
    status          game_status NOT NULL DEFAULT 'published',
    version         VARCHAR(50) DEFAULT '',
    developer       VARCHAR(150) DEFAULT '',
    publisher       VARCHAR(150) DEFAULT '',
    release_date    DATE,
    file_size       VARCHAR(50) DEFAULT '',
    age_rating      age_rating NOT NULL DEFAULT 'everyone',
    languages       TEXT[] DEFAULT '{}',
    trailer_url     TEXT DEFAULT '',
    cover_image     TEXT DEFAULT '',
    category_id     UUID REFERENCES categories(id) ON DELETE SET NULL,
    view_count      INTEGER NOT NULL DEFAULT 0,
    download_count  INTEGER NOT NULL DEFAULT 0,
    like_count      INTEGER NOT NULL DEFAULT 0,
    comment_count   INTEGER NOT NULL DEFAULT 0,
    share_count     INTEGER NOT NULL DEFAULT 0,
    rating_avg      NUMERIC(3,2) NOT NULL DEFAULT 0,
    rating_count    INTEGER NOT NULL DEFAULT 0,
    is_featured     BOOLEAN NOT NULL DEFAULT FALSE,
    published_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_games_user_id ON games(user_id);
CREATE INDEX idx_games_slug ON games(slug);
CREATE INDEX idx_games_status ON games(status);
CREATE INDEX idx_games_category ON games(category_id);
CREATE INDEX idx_games_published_at ON games(published_at DESC);
CREATE INDEX idx_games_view_count ON games(view_count DESC);
CREATE INDEX idx_games_download_count ON games(download_count DESC);
CREATE INDEX idx_games_rating_avg ON games(rating_avg DESC);
CREATE INDEX idx_games_title_trgm ON games USING gin (title gin_trgm_ops);
CREATE INDEX idx_games_content_trgm ON games USING gin (content gin_trgm_ops);

-- Game download links (hidden from viewers)
CREATE TABLE game_links (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id    UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    platform   platform_type NOT NULL,
    url        TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(game_id, platform)
);

CREATE INDEX idx_game_links_game ON game_links(game_id);

-- Game screenshots
CREATE TABLE game_screenshots (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id    UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    url        TEXT NOT NULL,
    caption    VARCHAR(255) DEFAULT '',
    position   INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_screenshots_game ON game_screenshots(game_id);

-- Game tags (many-to-many)
CREATE TABLE game_tags (
    game_id UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    tag_id  UUID NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (game_id, tag_id)
);

CREATE INDEX idx_game_tags_game ON game_tags(game_id);
CREATE INDEX idx_game_tags_tag ON game_tags(tag_id);

-- ============================================
-- INTERACTIONS
-- ============================================

-- Comments (with threaded replies)
CREATE TABLE comments (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id    UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    parent_id  UUID REFERENCES comments(id) ON DELETE CASCADE,
    content    TEXT NOT NULL,
    like_count INTEGER NOT NULL DEFAULT 0,
    is_pinned  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_comments_game ON comments(game_id, created_at DESC);
CREATE INDEX idx_comments_user ON comments(user_id);
CREATE INDEX idx_comments_parent ON comments(parent_id);

-- Likes
CREATE TABLE likes (
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    game_id    UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, game_id)
);

CREATE INDEX idx_likes_game ON likes(game_id);

-- Comment likes
CREATE TABLE comment_likes (
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    comment_id UUID NOT NULL REFERENCES comments(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, comment_id)
);

-- Ratings (1-5 stars)
CREATE TABLE ratings (
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    game_id    UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    score      SMALLINT NOT NULL CHECK (score >= 1 AND score <= 5),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, game_id)
);

CREATE INDEX idx_ratings_game ON ratings(game_id);

-- Reviews (detailed reviews with title + body)
CREATE TABLE reviews (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id    UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title      VARCHAR(200) NOT NULL,
    content    TEXT NOT NULL,
    rating     SMALLINT NOT NULL CHECK (rating >= 1 AND rating <= 5),
    helpful_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(game_id, user_id)
);

CREATE INDEX idx_reviews_game ON reviews(game_id, created_at DESC);
CREATE INDEX idx_reviews_user ON reviews(user_id);

-- Bookmarks / Favorites
CREATE TABLE bookmarks (
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    game_id    UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, game_id)
);

CREATE INDEX idx_bookmarks_user ON bookmarks(user_id);

-- Follow users
CREATE TABLE follows (
    follower_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    followee_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (follower_id, followee_id),
    CHECK (follower_id != followee_id)
);

CREATE INDEX idx_follows_follower ON follows(follower_id);
CREATE INDEX idx_follows_followee ON follows(followee_id);

-- Reports
CREATE TYPE report_status AS ENUM ('pending', 'reviewing', 'resolved', 'dismissed');
CREATE TYPE report_reason AS ENUM (
    'spam', 'malware', 'copyright', 'inappropriate', 'broken_link',
    'misleading', 'illegal', 'other'
);

CREATE TABLE reports (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id      UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    reporter_id  UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    reason       report_reason NOT NULL,
    description  TEXT DEFAULT '',
    status       report_status NOT NULL DEFAULT 'pending',
    resolved_by  UUID REFERENCES users(id),
    resolution   TEXT DEFAULT '',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at  TIMESTAMPTZ
);

CREATE INDEX idx_reports_game ON reports(game_id);
CREATE INDEX idx_reports_status ON reports(status);
CREATE INDEX idx_reports_reporter ON reports(reporter_id);

-- Downloads tracking
CREATE TABLE downloads (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id    UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    user_id    UUID REFERENCES users(id) ON DELETE SET NULL,
    platform   platform_type NOT NULL,
    ip_address VARCHAR(45),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_downloads_game ON downloads(game_id, created_at DESC);

-- Shares tracking
CREATE TYPE share_platform AS ENUM ('facebook', 'twitter', 'telegram', 'whatsapp', 'copy', 'native');

CREATE TABLE shares (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id    UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    user_id    UUID REFERENCES users(id) ON DELETE SET NULL,
    platform   share_platform NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_shares_game ON shares(game_id);

-- ============================================
-- NOTIFICATIONS
-- ============================================
CREATE TYPE notification_type AS ENUM (
    'comment', 'reply', 'like', 'follow', 'report_status',
    'system', 'new_game', 'review', 'rating', 'mention'
);

CREATE TABLE notifications (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    actor_id   UUID REFERENCES users(id) ON DELETE SET NULL,
    type       notification_type NOT NULL,
    title      VARCHAR(200) NOT NULL,
    content    TEXT DEFAULT '',
    link       TEXT DEFAULT '',
    is_read    BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_notifications_user ON notifications(user_id, is_read, created_at DESC);

-- ============================================
-- SESSIONS (cookie-based sessions stored in DB)
-- ============================================
CREATE TABLE sessions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  VARCHAR(64) NOT NULL UNIQUE,
    user_agent  TEXT DEFAULT '',
    ip_address  VARCHAR(45),
    expires_at  TIMESTAMPTZ NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_sessions_user ON sessions(user_id);
CREATE INDEX idx_sessions_token ON sessions(token_hash);
CREATE INDEX idx_sessions_expires ON sessions(expires_at);

-- ============================================
-- USER PREFERENCES
-- ============================================
CREATE TABLE user_preferences (
    user_id          UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    theme            VARCHAR(10) NOT NULL DEFAULT 'dark',
    email_notifications BOOLEAN NOT NULL DEFAULT TRUE,
    show_online      BOOLEAN NOT NULL DEFAULT TRUE,
    language         VARCHAR(10) NOT NULL DEFAULT 'vi',
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================
-- TRIGGERS: auto-update updated_at
-- ============================================
CREATE OR REPLACE FUNCTION update_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_users_updated BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();
CREATE TRIGGER trigger_games_updated BEFORE UPDATE ON games
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();
CREATE TRIGGER trigger_comments_updated BEFORE UPDATE ON comments
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();
CREATE TRIGGER trigger_reviews_updated BEFORE UPDATE ON reviews
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();
CREATE TRIGGER trigger_ratings_updated BEFORE UPDATE ON ratings
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();
CREATE TRIGGER trigger_preferences_updated BEFORE UPDATE ON user_preferences
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- ============================================
-- TRIGGERS: counters
-- ============================================
CREATE OR REPLACE FUNCTION increment_game_comment_count()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE games SET comment_count = comment_count + 1 WHERE id = NEW.game_id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION decrement_game_comment_count()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE games SET comment_count = comment_count - 1 WHERE id = OLD.game_id;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_comment_insert AFTER INSERT ON comments
    FOR EACH ROW EXECUTE FUNCTION increment_game_comment_count();
CREATE TRIGGER trigger_comment_delete AFTER DELETE ON comments
    FOR EACH ROW EXECUTE FUNCTION decrement_game_comment_count();

-- Like count triggers
CREATE OR REPLACE FUNCTION increment_like_count()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE games SET like_count = like_count + 1 WHERE id = NEW.game_id;
    INSERT INTO notifications (user_id, actor_id, type, title, link)
    SELECT g.user_id, NEW.user_id, 'like', 'Có người vừa thích game của bạn',
        '/games/' || g.slug
    FROM games g WHERE g.id = NEW.game_id AND g.user_id != NEW.user_id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION decrement_like_count()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE games SET like_count = like_count - 1 WHERE id = OLD.game_id;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_like_insert AFTER INSERT ON likes
    FOR EACH ROW EXECUTE FUNCTION increment_like_count();
CREATE TRIGGER trigger_like_delete AFTER DELETE ON likes
    FOR EACH ROW EXECUTE FUNCTION decrement_like_count();

-- Tag usage counter
CREATE OR REPLACE FUNCTION increment_tag_usage()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE tags SET usage_count = usage_count + 1 WHERE id = NEW.tag_id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION decrement_tag_usage()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE tags SET usage_count = usage_count - 1 WHERE id = OLD.tag_id;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_game_tag_insert AFTER INSERT ON game_tags
    FOR EACH ROW EXECUTE FUNCTION increment_tag_usage();
CREATE TRIGGER trigger_game_tag_delete AFTER DELETE ON game_tags
    FOR EACH ROW EXECUTE FUNCTION decrement_tag_usage();

-- ============================================
-- INITIAL DATA: Categories
-- ============================================
INSERT INTO categories (name, slug, description, icon) VALUES
    ('Hành động', 'hanh-dong', 'Game hành động, bắn súng, chiến đấu', 'action'),
    ('Phiêu lưu', 'phieu-luu', 'Game phiêu lưu, khám phá, giải đố', 'adventure'),
    ('Vai diễn', 'vai-dien', 'Game nhập vai (RPG)', 'rpg'),
    ('Chiến thuật', 'chien-thuat', 'Game chiến thuật, tư duy', 'strategy'),
    ('Thể thao', 'the-thao', 'Game thể thao, đua xe', 'sports'),
    ('Giải đố', 'giai-do', 'Game giải đố, trí tuệ', 'puzzle'),
    ('Mô phỏng', 'mo-phong', 'Game mô phỏng cuộc sống, kinh doanh', 'simulation'),
    ('Đua xe', 'dua-xe', 'Game đua xe, mô tô', 'racing'),
    ('Kinh dị', 'kinh-di', 'Game kinh dị, sinh tồn', 'horror'),
    ('Multiplayer', 'multiplayer', 'Game nhiều người chơi', 'multiplayer'),
    ('Indie', 'indie', 'Game độc lập, indie', 'indie'),
    ('Khác', 'khac', 'Thể loại khác', 'other')
ON CONFLICT (slug) DO NOTHING;
