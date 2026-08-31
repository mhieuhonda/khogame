-- ============================================================================
-- Migration 033 — v3.5.1: chống farm quest bằng toggle like (audit 5-e F7)
-- ============================================================================
-- BUG: quest "like_game" (và heatmap) bump +1 mỗi lần like. Like là TOGGLE:
-- unlike xoá row → like lại INSERT mới → vòng lặp like/unlike 1 game duy
-- nhất hoàn thành quest "thích N game" mà không cần N game thật.
-- (XP vẫn bounded vì claim là one-shot — nhưng quest integrity bị phá.)
--
-- FIX: bảng marker `like_history` ghi (user, game) đã từng like CHƯA.
-- Quest chỉ bump khi INSERT vào like_history thành công (lần đầu tiên
-- user này thích game này). PK (user_id, game_id) — 1 row/user/game,
-- write cost 1 INSERT indexed nhỏ nhất có thể.
-- ============================================================================

CREATE TABLE IF NOT EXISTS like_history (
    user_id  UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    game_id  UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    liked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, game_id)
);
