-- ============================================================================
-- Migration 047 — v3.12.0: CHỐNG BÁO CÁO TRÙNG (audit logic L3)
-- ============================================================================
-- Bug gốc: submit_report check `has_reported` rồi INSERT là check-then-act
-- — 2 POST song song cùng user/game đều pass rồi cùng INSERT → 2 report
-- trùng, admin phải xử lý 2 lần.
--
-- Fix 2 lớp:
--   1. (migration này) unique partial index chặn dup ở tầng DB.
--   2. (code) INSERT ... ON CONFLICT ... WHERE status IN (...) DO NOTHING.
--
-- Phải dedupe dữ liệu cũ TRƯỚC khi tạo unique index (nếu đã có dup thì
-- CREATE UNIQUE INDEX sẽ fail → app exit lúc startup — bài học prod v3.10.0).
-- Giữ report MỚI NHẤT cho mỗi (game_id, reporter_id) đang active;
-- tie cùng timestamp giữ id lớn hơn (deterministic).
-- ============================================================================

DELETE FROM reports r
USING reports newer
WHERE r.game_id = newer.game_id
  AND r.reporter_id = newer.reporter_id
  AND r.status IN ('pending', 'reviewing')
  AND newer.status IN ('pending', 'reviewing')
  AND (r.created_at < newer.created_at
       OR (r.created_at = newer.created_at AND r.id < newer.id));

CREATE UNIQUE INDEX IF NOT EXISTS uq_reports_active_per_reporter
    ON reports (game_id, reporter_id)
    WHERE status IN ('pending', 'reviewing');

-- Guard: index phải tồn tại (fail sớm lúc migrate thay vì im lặng thiếu ràng buộc)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE indexname = 'uq_reports_active_per_reporter'
    ) THEN
        RAISE EXCEPTION 'Migration 047 FAILED: uq_reports_active_per_reporter not created';
    END IF;
END $$;
