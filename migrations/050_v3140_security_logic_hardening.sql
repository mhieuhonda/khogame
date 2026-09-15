-- ============================================================================
-- Migration 050 — v3.14.0: SIÊU FIX bảo mật + logic (đợt audit 3 trục)
-- ============================================================================
-- Nguyên tắc: KHÔNG BAO GIỜ sửa/xóa migration đã apply (sqlx 0.9 validate
-- checksum — sửa file cũ = crash boot prod, bài học incident v0.5.1/v3.13).
-- Mọi thay đổi schema ở đây đều là file MỚI, idempotent, và an toàn với
-- dữ liệu cũ:
--   * CHECK constraints dùng NOT VALID → chỉ áp cho row MỚI, không scan/
--     không fail vì dữ liệu legacy.
--   * CREATE OR REPLACE cho function (like notify) — giữ behavior cũ cho
--     dữ liệu đã có, chỉ chống spam cho tương tác mới.
--
-- Nội dung:
--   1) games.status DEFAULT 'published' → 'draft' (đồng bộ với
--      GameStatus::default() = Draft trong code — typo/struct thiếu field
--      không còn tự xuất bản game).
--   2) CHECK games.rating_avg BETWEEN 0 AND 5 (NOT VALID).
--   3) CHECK xp_events.amount <> 0 (NOT VALID — XP 0 vô nghĩa, chống bug
--      cộng/trừ 0 làm bẩn history).
--   4) increment_like_count(): chỉ tạo notification khi CHƯA có like-notification
--      chưa đọc từ cùng actor → unlike→re-like loop không spam owner nữa
--      (đồng bộ với pattern follow đã có).
-- ============================================================================

-- 1) Default status game mới = draft
ALTER TABLE games ALTER COLUMN status SET DEFAULT 'draft';

-- 2) Rating trung bình luôn trong thang 0..5 (chỉ row mới)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chk_games_rating_range'
    ) THEN
        ALTER TABLE games
            ADD CONSTRAINT chk_games_rating_range
            CHECK (rating_avg >= 0 AND rating_avg <= 5) NOT VALID;
    END IF;
END
$$;

-- 3) XP event amount khác 0 (chỉ row mới)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chk_xp_events_amount_nonzero'
    ) THEN
        ALTER TABLE xp_events
            ADD CONSTRAINT chk_xp_events_amount_nonzero
            CHECK (amount <> 0) NOT VALID;
    END IF;
END
$$;

-- 4) Like notification: chống spam unlike → re-like
CREATE OR REPLACE FUNCTION increment_like_count()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE games SET like_count = like_count + 1 WHERE id = NEW.game_id;
    -- Chỉ notify khi chưa có like-notification CHƯA ĐỌC từ cùng actor cho
    -- cùng game (unlike rồi like lại trong khi owner chưa đọc → không spam).
    IF NOT EXISTS (
        SELECT 1 FROM notifications n
        JOIN games g ON g.id = NEW.game_id
        WHERE n.user_id = g.user_id
          AND n.actor_id = NEW.user_id
          AND n.type = 'like'
          AND n.link = '/games/' || g.slug
          AND n.is_read = FALSE
          AND g.user_id != NEW.user_id
    ) THEN
        INSERT INTO notifications (user_id, actor_id, type, title, link)
        SELECT g.user_id, NEW.user_id, 'like', 'Có người vừa thích game của bạn',
            '/games/' || g.slug
        FROM games g WHERE g.id = NEW.game_id AND g.user_id != NEW.user_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
