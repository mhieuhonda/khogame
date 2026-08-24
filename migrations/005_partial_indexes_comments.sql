-- ============================================
-- 005: Partial index cho comments (top-level only)
-- ============================================
-- Mục đích: Tối ưu hoá query list top-level comments theo game.
-- Handler CommentRepo::list_by_game query:
--   WHERE c.game_id = $1 AND c.parent_id IS NULL
--   ORDER BY c.is_pinned DESC, c.created_at DESC
-- Index cũ (idx_comments_game) chỉ index trên (game_id, created_at DESC)
-- — Postgres vẫn phải filter parent_id IS NULL sau khi seek index.
-- Partial index chỉ chứa top-level comments → index nhỏ hơn + seek nhanh hơn.
-- Đồng thời thêm index cho replies theo parent_id (query list_replies).

CREATE INDEX IF NOT EXISTS idx_comments_toplevel
  ON comments (game_id, is_pinned DESC, created_at DESC)
  WHERE parent_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_comments_replies
  ON comments (parent_id, created_at DESC)
  WHERE parent_id IS NOT NULL;
