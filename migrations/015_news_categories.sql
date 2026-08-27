-- ============================================
-- 015: News categories — table riêng cho thể loại tin tức
-- ============================================
-- Trước đây: NEWS_CATEGORIES hardcode trong src/handlers/news.rs (8 mục cố định).
-- Admin muốn thêm/xóa thể loại tin mà không deploy code mới → cần table.
--
-- Workflow:
-- - Admin thêm "Cập nhật" → INSERT INTO news_categories → tin tức có thể chọn
--   category mới (select trong form /news/new).
-- - Admin xoá "Cập nhật" → DELETE FROM news_categories WHERE slug='cap-nhat'
--   → tin cũ có category='cap-nhat' vẫn giữ giá trị text (ON DELETE RESTRICT
--   không dùng vì không có FK — news.category là VARCHAR, không reference).
--
-- Cột news.category trong bảng news giữ kiểu VARCHAR (không đổi) để giữ
-- tương thích lùi — tin cũ không bị ảnh hưởng khi admin đổi tên category.
-- Slug là khoá — admin đổi `name` hiển thị, slug giữ nguyên để URL ổn định.
-- ============================================

CREATE TABLE news_categories (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        VARCHAR(50) NOT NULL UNIQUE,
    slug        VARCHAR(60) NOT NULL UNIQUE,
    description TEXT DEFAULT '',
    icon        VARCHAR(50) DEFAULT '',
    -- Thứ tự hiển thị — admin có thể drag-drop để sắp (TODO UI sau).
    -- Mặc định 0 → sắp theo name ASC.
    sort_order  INTEGER NOT NULL DEFAULT 0,
    is_active   BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_news_categories_active ON news_categories(is_active, sort_order, name);
CREATE INDEX idx_news_categories_slug ON news_categories(slug);

-- Trigger update updated_at (dùng function có sẵn từ migration 001).
CREATE TRIGGER trigger_news_categories_updated BEFORE UPDATE ON news_categories
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- Seed với 8 category mặc định (match NEWS_CATEGORIES trong code để
-- user không bị mất category khi upgrade từ v1.3.x lên v1.4.0).
-- ON CONFLICT DO NOTHING để idempotent — chạy lại migration không lỗi.
INSERT INTO news_categories (name, slug, description, icon, sort_order) VALUES
    ('Tin game',      'game',      'Tin tức về game — release, update, sự kiện',   'game',      10),
    ('Công nghệ',     'tech',      'Tin công nghệ liên quan game — GPU, console, engine', 'tech',     20),
    ('Ngành game',    'industry',  'Tin ngành game — doanh thu, sáp nhập, chiến lược', 'industry', 30),
    ('Esports',       'esports',   'Tin thể thao điện tử — tournament, team, player', 'esports',  40),
    ('Cộng đồng',     'community', 'Tin cộng đồng — event, fanart, chia sẻ',         'community', 50),
    ('Review',        'review',    'Review game — đánh giá chi tiết',                 'review',    60),
    ('Cập nhật',      'update',    'Cập nhật patch — hotfix, balance, DLC',           'update',    70),
    ('Khác',          'other',     'Thể loại khác — không thuộc nhóm trên',           'other',     80)
ON CONFLICT (slug) DO NOTHING;
