-- v2.2.0 — Performance indexes + email queue enhancements
-- Composite index cho NewsRepo::list_published ORDER BY clause
-- (trước đây chỉ có idx_news_published_at đơn cột, sort is_featured +
-- created_at tiebreaker phải thực hiện trong memory).
CREATE INDEX IF NOT EXISTS idx_news_list_published
    ON news (is_featured DESC, published_at DESC NULLS LAST, created_at DESC)
    WHERE status = 'published';

-- Composite index cho news_comments list top-level (mỗi news article
-- load 50 top-level comments). Trước đây chỉ có idx_news_comments_news_id
-- không partial theo parent_id.
CREATE INDEX IF NOT EXISTS idx_news_comments_toplevel
    ON news_comments (news_id, is_pinned DESC, created_at DESC)
    WHERE parent_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_news_comments_replies
    ON news_comments (parent_id, created_at ASC)
    WHERE parent_id IS NOT NULL;

-- Composite index cho email_queue status + next_retry_at
-- (đã có idx_email_queue_pending, nhưng bổ diễn index đơn cột cho status
-- để admin query "SELECT * FROM email_queue WHERE status = 'failed'")
-- và index cho recipient để user report spam check.
CREATE INDEX IF NOT EXISTS idx_email_queue_status
    ON email_queue (status);

CREATE INDEX IF NOT EXISTS idx_email_queue_recipient
    ON email_queue (recipient);
