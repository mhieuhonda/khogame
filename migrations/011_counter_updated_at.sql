-- Migration 011: chỉ bump updated_at khi field ngoài counter thay đổi
--
-- Trước đây, hàm `update_updated_at()` đặt `NEW.updated_at = NOW()` cho MỌI
-- UPDATE, kể cả counter bumps từ trigger `increment_view_count`,
-- `increment_like_count`, etc. Hậu quả:
-- - `games.updated_at` bị bump mỗi lượt xem → sitemap `<lastmod>` stale 1s
--   sau khi user vừa xem → Googlebot re-crawl liên tục, tốn crawl budget.
-- - Admin "recently edited games" sort theo `updated_at` → game được xem
--   nhiều nhất luôn nổi đầu danh sách, lấn át game thật sự vừa edit.
-- - `news.updated_at` cũng bị bump qua view counter → SEO + admin list lệch.
--
-- Fix: trong `update_updated_at()`, chỉ set `updated_at = NOW()` KHI update
-- chạm tới field NGOÀI danh sách counter (view_count, like_count,
-- comment_count, download_count, rating_avg, rating_count). Cách PostgreSQL
-- chuẩn là kiểm tra `NEW.field IS DISTINCT FROM OLD.field` cho từng cột
-- không phải counter.
--
-- Cách dùng `IF (NEW.* IS DISTINCT FROM OLD.*)` không khả thi vì counter
-- cũng là một phần của row → luôn DISTINCT. Phải liệt kê tường minh.
--
-- Vì function `update_updated_at()` chung cho mọi bảng, ta list field counter
-- của từng bảng trong cùng hàm (an toàn: field không tồn tại trong bảng khác
-- → `NEW.field IS DISTINCT FROM OLD.field` sẽ compile fail). Tránh dùng
-- `NEW.view_count` cho bảng `users` (không có cột đó).
--
-- Giải pháp dùng hàm riêng cho mỗi bảng nhạy cảm: tạo `update_games_updated_at()`
-- và `update_news_updated_at()`. Giữ `update_updated_at()` nguyên cho các
-- bảng không có counter trigger (users, comments, reviews, ratings, prefs).

-- 1) Hàm mới cho `games`: chỉ bump nếu field không phải counter thay đổi.
CREATE OR REPLACE FUNCTION update_games_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.title IS DISTINCT FROM OLD.title
        OR NEW.slug IS DISTINCT FROM OLD.slug
        OR NEW.excerpt IS DISTINCT FROM OLD.excerpt
        OR NEW.content IS DISTINCT FROM OLD.content
        OR NEW.cover_image IS DISTINCT FROM OLD.cover_image
        OR NEW.trailer_url IS DISTINCT FROM OLD.trailer_url
        OR NEW.status IS DISTINCT FROM OLD.status
        OR NEW.user_id IS DISTINCT FROM OLD.user_id
        OR NEW.category_id IS DISTINCT FROM OLD.category_id
        OR NEW.release_date IS DISTINCT FROM OLD.release_date
        OR NEW.is_featured IS DISTINCT FROM OLD.is_featured
        OR NEW.is_public IS DISTINCT FROM OLD.is_public
    THEN
        NEW.updated_at = NOW();
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- 2) Hàm mới cho `news`: tương tự.
CREATE OR REPLACE FUNCTION update_news_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.title IS DISTINCT FROM OLD.title
        OR NEW.slug IS DISTINCT FROM OLD.slug
        OR NEW.excerpt IS DISTINCT FROM OLD.excerpt
        OR NEW.content IS DISTINCT FROM OLD.content
        OR NEW.source_url IS DISTINCT FROM OLD.source_url
        OR NEW.cover_image IS DISTINCT FROM OLD.cover_image
        OR NEW.status IS DISTINCT FROM OLD.status
        OR NEW.author_id IS DISTINCT FROM OLD.author_id
        OR NEW.category_id IS DISTINCT FROM OLD.category_id
        OR NEW.is_featured IS DISTINCT FROM OLD.is_featured
    THEN
        NEW.updated_at = NOW();
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- 3) Thay trigger cũ bằng trigger mới cho 2 bảng này.
DROP TRIGGER IF EXISTS trigger_games_updated ON games;
CREATE TRIGGER trigger_games_updated BEFORE UPDATE ON games
    FOR EACH ROW EXECUTE FUNCTION update_games_updated_at();

DROP TRIGGER IF EXISTS trigger_news_updated ON news;
CREATE TRIGGER trigger_news_updated BEFORE UPDATE ON news
    FOR EACH ROW EXECUTE FUNCTION update_news_updated_at();

-- 4) Cập nhật lại `updated_at` cho games/news đã bị bump sai trong quá khứ
-- về giá trị `created_at` (chống admin list lệch). Chỉ chạy 1 lần ở migration.
-- Lưu ý: không reset về created_at nếu game đã được edit thật (admin update
-- status, user edit content) — chỉ reset cho hàng có updated_at > created_at
-- nhưng các field counter là duy nhất field đã thay đổi. Tạm thời bỏ qua vì
-- không có cách phát hiện chính xác. Cập nhật sau bằng batch script nếu cần.
