-- ============================================
-- 003: Make trigram indexes resilient
-- ============================================
-- Mục đích: Tái tạo 2 index trigram (idx_games_title_trgm, idx_games_content_trgm)
-- bằng khối DO $$ ... EXCEPTION, để migration không fail khi extension pg_trgm
-- chưa được cài trong môi trường test/dev. Trên prod (có pg_trgm), đây là no-op.
-- Việc này giúp dễ dàng chạy local mà không cần cài postgresql-contrib.

DO $$
BEGIN
    DROP INDEX IF EXISTS idx_games_title_trgm;
    CREATE INDEX idx_games_title_trgm ON games USING gin (title gin_trgm_ops);
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'pg_trgm/gin_trgm_ops không khả dụng — bỏ qua index trigram cho title';
END
$$;

DO $$
BEGIN
    DROP INDEX IF EXISTS idx_games_content_trgm;
    CREATE INDEX idx_games_content_trgm ON games USING gin (content gin_trgm_ops);
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'pg_trgm/gin_trgm_ops không khả dụng — bỏ qua index trigram cho content';
END
$$;
