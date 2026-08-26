-- ============================================
-- Counter triggers: GREATEST(0, x - 1) chống underflow
--
-- Trước đây các trigger decrement dùng `count = count - 1` thuần.
-- Nếu comment_count/like_count/tag.usage_count đã là 0 do race condition
-- (trigger fires out-of-order, manual SQL update, schema drift), phép
-- trừ sẽ tạo ra -1 — vi phạm assumption toàn codebase (count >= 0).
-- UI hiển thị "-1 bình luận" rất khó hiểu.
--
-- Áp dụng cho cả game (migration 001) và news (migration 008) triggers.
-- ============================================

CREATE OR REPLACE FUNCTION decrement_game_comment_count()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE games SET comment_count = GREATEST(0, comment_count - 1) WHERE id = OLD.game_id;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION decrement_like_count()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE games SET like_count = GREATEST(0, like_count - 1) WHERE id = OLD.game_id;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION decrement_tag_usage()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE tags SET usage_count = GREATEST(0, usage_count - 1) WHERE id = OLD.tag_id;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

-- News decrement triggers (migration 008) — cùng issue
CREATE OR REPLACE FUNCTION decrement_news_like_count()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE news SET like_count = GREATEST(0, like_count - 1) WHERE id = OLD.news_id;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION decrement_news_comment_count()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE news SET comment_count = GREATEST(0, comment_count - 1) WHERE id = OLD.news_id;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;
