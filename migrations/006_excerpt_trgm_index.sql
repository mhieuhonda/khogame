-- ============================================
-- 006: Trigram index cho excerpt
-- ============================================
-- Search dùng ILIKE trên 3 cột: title OR excerpt OR content.
-- 001/003 đã có trigram index cho title & content nhưng THIẾU excerpt —
-- PostgreSQL cần cả 3 index để dựng BitmapOr plan; thiếu 1 cột là query
-- rơi về Seq Scan toàn bảng games dù có 2 index sẵn.
-- Dùng khối DO $$ EXCEPTION như 003 để không fail khi dev thiếu pg_trgm.

DO $$
BEGIN
    CREATE INDEX IF NOT EXISTS idx_games_excerpt_trgm ON games USING gin (excerpt gin_trgm_ops);
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'pg_trgm/gin_trgm_ops không khả dụng — bỏ qua index trigram cho excerpt';
END
$$;
