-- ============================================
-- 008: News module — tin tức cộng đồng với admin approval
-- Workflow: draft → pending → published → archived
--           (rejected là status riêng để admin phân biệt)
-- Người dùng đăng tin → vào hàng đợi pending → admin duyệt → published.
-- Tránh lan truyền tin giả trên nền tảng cộng đồng.
-- ============================================

CREATE TYPE news_status AS ENUM ('draft', 'pending', 'published', 'archived', 'rejected');

CREATE TABLE news (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title           VARCHAR(200) NOT NULL,
    slug            VARCHAR(220) NOT NULL UNIQUE,
    excerpt         VARCHAR(500) DEFAULT '',
    content         TEXT NOT NULL DEFAULT '',
    cover_image     TEXT DEFAULT '',
    category        VARCHAR(50) DEFAULT '',   -- 'game', 'tech', 'industry', 'esports', 'community'…
    source_url      TEXT DEFAULT '',           -- link nguồn (bắt buộc nếu không phải tin gốc)
    source_name      VARCHAR(150) DEFAULT '',  -- tên nguồn (VnExpress, GameK, IGN Vietnam…)
    status          news_status NOT NULL DEFAULT 'pending',
    -- IP + UA của người đăng (admin xem được để truy vết spam/abuse)
    author_ip       VARCHAR(45),
    author_ua       TEXT DEFAULT '',
    -- Thông tin duyệt (nếu bị reject/publish bởi admin)
    reviewed_by     UUID REFERENCES users(id) ON DELETE SET NULL,
    review_note     TEXT DEFAULT '',           -- lý do reject / note nội bộ
    -- Counter (giống games)
    view_count      INTEGER NOT NULL DEFAULT 0,
    like_count      INTEGER NOT NULL DEFAULT 0,
    comment_count   INTEGER NOT NULL DEFAULT 0,
    is_featured     BOOLEAN NOT NULL DEFAULT FALSE,
    published_at    TIMESTAMPTZ,                -- chỉ set khi status='published'
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_news_slug ON news(slug);
CREATE INDEX idx_news_user ON news(user_id);
CREATE INDEX idx_news_status ON news(status);
CREATE INDEX idx_news_published_at ON news(published_at DESC);
CREATE INDEX idx_news_view_count ON news(view_count DESC);
CREATE INDEX idx_news_category ON news(category);
CREATE INDEX idx_news_title_trgm ON news USING gin (title gin_trgm_ops);
CREATE INDEX idx_news_content_trgm ON news USING gin (content gin_trgm_ops);

-- Trigger update updated_at
CREATE TRIGGER trigger_news_updated BEFORE UPDATE ON news
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- ============================================
-- News likes — tách riêng khỏi game likes để dễ tuning rate-limit
-- ============================================
CREATE TABLE news_likes (
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    news_id    UUID NOT NULL REFERENCES news(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, news_id)
);

CREATE INDEX idx_news_likes_news ON news_likes(news_id);

-- Trigger increment/decrement like_count
CREATE OR REPLACE FUNCTION increment_news_like_count()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE news SET like_count = like_count + 1 WHERE id = NEW.news_id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION decrement_news_like_count()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE news SET like_count = like_count - 1 WHERE id = OLD.news_id;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_news_like_insert AFTER INSERT ON news_likes
    FOR EACH ROW EXECUTE FUNCTION increment_news_like_count();
CREATE TRIGGER trigger_news_like_delete AFTER DELETE ON news_likes
    FOR EACH ROW EXECUTE FUNCTION decrement_news_like_count();

-- ============================================
-- News comments — tách bảng riêng (news có thể tắt comment,
-- comment riêng không thread sâu như game)
-- ============================================
CREATE TABLE news_comments (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    news_id    UUID NOT NULL REFERENCES news(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    parent_id  UUID REFERENCES news_comments(id) ON DELETE CASCADE,
    content    TEXT NOT NULL,
    like_count INTEGER NOT NULL DEFAULT 0,
    is_pinned  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_news_comments_news ON news_comments(news_id, created_at DESC);
CREATE INDEX idx_news_comments_user ON news_comments(user_id);
CREATE INDEX idx_news_comments_parent ON news_comments(parent_id);

CREATE TRIGGER trigger_news_comments_updated BEFORE UPDATE ON news_comments
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- News comment likes
CREATE TABLE news_comment_likes (
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    comment_id UUID NOT NULL REFERENCES news_comments(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, comment_id)
);

CREATE INDEX idx_news_comment_likes_comment ON news_comment_likes(comment_id);

-- Trigger counter cho news comments
CREATE OR REPLACE FUNCTION increment_news_comment_count()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE news SET comment_count = comment_count + 1 WHERE id = NEW.news_id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION decrement_news_comment_count()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE news SET comment_count = comment_count - 1 WHERE id = OLD.news_id;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_news_comment_insert AFTER INSERT ON news_comments
    FOR EACH ROW EXECUTE FUNCTION increment_news_comment_count();
CREATE TRIGGER trigger_news_comment_delete AFTER DELETE ON news_comments
    FOR EACH ROW EXECUTE FUNCTION decrement_news_comment_count();

-- ============================================
-- Notification type mở rộng cho news
-- ============================================
ALTER TYPE notification_type ADD VALUE IF NOT EXISTS 'news_approved';
ALTER TYPE notification_type ADD VALUE IF NOT EXISTS 'news_rejected';
ALTER TYPE notification_type ADD VALUE IF NOT EXISTS 'news_comment';
