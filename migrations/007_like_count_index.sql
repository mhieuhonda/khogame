-- 007: Index cho sort "liked" (like_count DESC)
--
-- list_published / by_category / by_tag / search đều hỗ trợ sort=liked
-- (ORDER BY g.like_count DESC) nhưng like_count là cột duy nhất trong
-- nhóm sort (view_count, download_count, rating_avg, published_at)
-- KHÔNG có index — Postgres phải seq scan + sort toàn bảng games mỗi
-- lần user bấm "Yêu thích" trên mọi trang list.
--
-- Index partial chỉ hàng published để khớp WHERE status='published'
-- của mọi query sort (nhỏ hơn index đầy đủ, cập nhật nhanh hơn khi
-- game draft thay đổi like).

CREATE INDEX IF NOT EXISTS idx_games_like_count
    ON games (like_count DESC)
    WHERE status = 'published';
