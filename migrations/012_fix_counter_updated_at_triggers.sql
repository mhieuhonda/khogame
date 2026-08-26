-- ============================================
-- 012: FIX broken trigger functions from migration 011
-- ============================================
-- Mục đích: Sửa 2 hàm `update_games_updated_at()` và
-- `update_news_updated_at()` do migration 011 tạo ra có tham chiếu tới
-- CỘT KHÔNG TỒN TẠI → mọi UPDATE trên games/news đều crash ở runtime.
--
-- Phát hiện: Migration 011 đã được áp dụng vào prod nhưng unit test
-- (cargo test --lib) không kết nối DB thật → trigger function không
-- được thực thi → bug im lặng đến khi user/comment/like/admin-edit
-- chạm UPDATE → 500 error. Verification "Migration chain 001 → 011
-- idempotent" trong CHANGELOG chỉ kiểm tra DDL syntax, không kiểm tra
-- behavior của function.
--
-- Bug chi tiết:
--   1) `update_games_updated_at()` tham chiếu `NEW.is_public` — cột
--      `is_public` KHÔNG tồn tại trong bảng `games` (chỉ có `is_featured`).
--      → Mọi UPDATE games (comment_count bump từ trigger_comment_insert,
--      like_count bump từ trigger_like_insert, view_count bump từ
--      repository code, admin edit game, v.v.) đều fail với:
--      ERROR: column "is_public" of relation "games" does not exist
--      → INSERT comment/like rollback → user nhận 500.
--
--   2) `update_news_updated_at()` tham chiếu `NEW.author_id` và
--      `NEW.category_id`:
--      • Bảng `news` dùng `user_id` (không phải `author_id`).
--      • Bảng `news` dùng `category VARCHAR(50)` (không phải `category_id UUID`).
--      → Mọi UPDATE news (comment_count bump từ trigger_news_comment_insert,
--      like_count bump từ trigger_news_like_insert, view_count bump,
--      admin approve news, user edit pending news, v.v.) đều fail.
--      → INSERT news comment/like rollback → user nhận 500.
--
-- Tác động prod (nếu đã deploy v1.0.0 với migration 011):
--   • User không thể comment game (any game).
--   • User không thể like game.
--   • User không thể comment/like news.
--   • Admin không thể edit game/news.
--   • View/download counter không bump (silent failure vì repo code
--     thường dùng `let _ = pool.execute(...)` cho counter bump).
--   • Janitor dọn notification/sessions có thể crash nếu UPDATE chạm
--     trigger (sessions không có trigger games_updated nhưng admin
--     ban/unban user thì chạm trigger_users_updated — function này
--     không bị bug, chỉ 2 function games/news bị).
--
-- Cách fix: dùng `CREATE OR REPLACE FUNCTION` để thay body của 2 hàm
-- bằng phiên bản đúng. Function OID giữ nguyên → trigger hiện tại
-- (tạo ở migration 011) tự động pickup body mới, không cần DROP/CREATE
-- trigger.
--
-- Idempotent: `CREATE OR REPLACE` chạy lại không lỗi.
-- ============================================

-- 1) Sửa `update_games_updated_at()` — bỏ `NEW.is_public` (không tồn tại),
--    bổ sung các cột còn thiếu (version, developer, publisher, file_size,
--    age_rating, languages, published_at) để bump updated_at khi các
--    trường này thay đổi (đảm bảo admin list "recently edited" chính xác).
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
        OR NEW.version IS DISTINCT FROM OLD.version
        OR NEW.developer IS DISTINCT FROM OLD.developer
        OR NEW.publisher IS DISTINCT FROM OLD.publisher
        OR NEW.file_size IS DISTINCT FROM OLD.file_size
        OR NEW.age_rating IS DISTINCT FROM OLD.age_rating
        OR NEW.languages IS DISTINCT FROM OLD.languages
        OR NEW.published_at IS DISTINCT FROM OLD.published_at
    THEN
        NEW.updated_at = NOW();
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- 2) Sửa `update_news_updated_at()` — đổi `NEW.author_id` → `NEW.user_id`,
--    đổi `NEW.category_id` → `NEW.category`, bổ sung `source_name`,
--    `reviewed_by`, `review_note`, `published_at` để bump updated_at khi
--    các trường này thay đổi (admin approve news → published_at đổi →
--    updated_at bump → admin list "recently edited" chính xác).
CREATE OR REPLACE FUNCTION update_news_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.title IS DISTINCT FROM OLD.title
        OR NEW.slug IS DISTINCT FROM OLD.slug
        OR NEW.excerpt IS DISTINCT FROM OLD.excerpt
        OR NEW.content IS DISTINCT FROM OLD.content
        OR NEW.source_url IS DISTINCT FROM OLD.source_url
        OR NEW.source_name IS DISTINCT FROM OLD.source_name
        OR NEW.cover_image IS DISTINCT FROM OLD.cover_image
        OR NEW.status IS DISTINCT FROM OLD.status
        OR NEW.user_id IS DISTINCT FROM OLD.user_id
        OR NEW.category IS DISTINCT FROM OLD.category
        OR NEW.is_featured IS DISTINCT FROM OLD.is_featured
        OR NEW.reviewed_by IS DISTINCT FROM OLD.reviewed_by
        OR NEW.review_note IS DISTINCT FROM OLD.review_note
        OR NEW.published_at IS DISTINCT FROM OLD.published_at
    THEN
        NEW.updated_at = NOW();
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- 3) Sanity check: chạy 1 UPDATE no-op trên 1 row thật (nếu có) để
--    force compile trigger function + verify column reference hợp lệ.
--    PostgreSQL lazy-compiles plpgsql function ở lần gọi đầu — nếu
--    column reference sai, lỗi chỉ surface khi user chạm UPDATE đầu
--    tiên → 500 error. Migration này chủ động force compile để fail-fast
--    ngay lúc deploy nếu fix sai.
--
-- Trick: `UPDATE games SET updated_at = updated_at WHERE id IN (SELECT ...)`
--    - SET updated_at = updated_at: không đổi giá trị → row không thực sự
--      thay đổi (PostgreSQL vẫn log WAL entry nhưng user data không đổi).
--    - Trigger function chạy, compile body (check column refs), rồi exit
--      sớm vì `IF` không match (không field khác) → không bump updated_at.
--    - Nếu function broken: compile fail → UPDATE fail → EXCEPTION →
--      migration fail → operator thấy lỗi rõ ràng ở deploy.
--    - Nếu table empty: 0 row match → trigger không fire → không verify
--      được, nhưng migration vẫn thành công (fresh DB không có user chạm
--      UPDATE nên bug không surface ngay; sẽ surface ở test đầu tiên).
DO $$
DECLARE
    g_id UUID;
    n_id UUID;
BEGIN
    SELECT id INTO g_id FROM games LIMIT 1;
    IF g_id IS NOT NULL THEN
        BEGIN
            UPDATE games SET updated_at = updated_at WHERE id = g_id;
        EXCEPTION WHEN OTHERS THEN
            RAISE EXCEPTION 'Migration 012: update_games_updated_at() vẫn lỗi sau fix: %', SQLERRM
            USING ERRCODE = 'P0001';
        END;
        RAISE NOTICE 'Migration 012: update_games_updated_at() verified OK (test row %)', g_id;
    ELSE
        RAISE NOTICE 'Migration 012: games table empty — skip verify update_games_updated_at() (will surface on first UPDATE)';
    END IF;

    SELECT id INTO n_id FROM news LIMIT 1;
    IF n_id IS NOT NULL THEN
        BEGIN
            UPDATE news SET updated_at = updated_at WHERE id = n_id;
        EXCEPTION WHEN OTHERS THEN
            RAISE EXCEPTION 'Migration 012: update_news_updated_at() vẫn lỗi sau fix: %', SQLERRM
            USING ERRCODE = 'P0001';
        END;
        RAISE NOTICE 'Migration 012: update_news_updated_at() verified OK (test row %)', n_id;
    ELSE
        RAISE NOTICE 'Migration 012: news table empty — skip verify update_news_updated_at() (will surface on first UPDATE)';
    END IF;
END
$$;
