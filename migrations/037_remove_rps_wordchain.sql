-- ============================================
-- 037: v3.8.0 — XÓA HOÀN TOÀN 2 game mode đang "được xem xét":
-- RPS (Oẳn tù tì / Kéo búa bao) + Word Chain (Nối từ)
--
-- Quyết định của Hieu Louis: 2 chế độ đã tạm dừng từ v3.4.0
-- (ARCADE_UNDER_REVIEW) giờ được gỡ bỏ vĩnh viễn khỏi nền tảng
-- để tập trung phát triển các tính năng cốt lõi.
--
-- Migration này undo các phần DB của migration 024 + 026:
--   * DROP bảng rps_plays / word_chain_plays (024)
--   * DROP bảng rps_matches / word_chain_matches (026)
--   * DELETE 10 huy hiệu rps_* / word_chain_* khỏi catalog achievements
--     (user_achievements cascade xoá theo FK ON DELETE CASCADE)
--   * DELETE xp_events reason rps_win / word_chain (activity feed
--     không còn nhãn cho 2 reason này)
--
-- Idempotent: DROP IF EXISTS / DELETE không lỗi khi chạy lại.
-- ============================================

-- 1) Matchmaking tables (migration 026) — rps_matches trước word_chain_matches
DROP TABLE IF EXISTS rps_matches;
DROP TABLE IF EXISTS word_chain_matches;

-- 2) Play history tables (migration 024)
DROP TABLE IF EXISTS rps_plays;
DROP TABLE IF EXISTS word_chain_plays;

-- 3) Huy hiệu arcade rời khỏi catalog.
--    FK user_achievements.achievement_id → achievements(id) ON DELETE CASCADE
--    tự xoá các row user_achievements tương ứng (huy hiệu không còn nghĩa
--    là không ai giữ nữa — kể cả người đã đạt từ v3.1.0–v3.4.0).
DELETE FROM achievements WHERE id IN (
    'rps_first_win', 'rps_10_wins', 'rps_50_wins', 'rps_100_wins', 'rps_500_wins',
    'word_chain_first', 'word_chain_10', 'word_chain_50', 'word_chain_100', 'word_chain_500'
);

-- 4) Dọn activity feed: 2 reason XP không còn nhãn hiển thị (label()
--    fallback "có hoạt động mới" vẫn hoạt động nhưng giữ dữ liệu sạch).
DELETE FROM xp_events WHERE reason IN ('rps_win', 'word_chain');

-- 5) Xoá match dở nơi users có thể đang poll — không còn route để poll.
--    (Bảng đã DROP ở bước 1 nên không cần UPDATE gì thêm.)
