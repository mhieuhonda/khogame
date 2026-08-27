-- v2.2.0 — Email notification queue + related news helper
-- Email queue: notifications nào có user.preferences.email_notifications=true
-- và chưa được gửi sẽ được janitor pickup và gửi SMTP.

CREATE TABLE IF NOT EXISTS email_queue (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    notification_id UUID NOT NULL REFERENCES notifications(id) ON DELETE CASCADE,
    recipient       TEXT NOT NULL,        -- user email
    recipient_name  TEXT NOT NULL DEFAULT '',
    subject         TEXT NOT NULL,
    body_html       TEXT NOT NULL,
    body_text       TEXT NOT NULL DEFAULT '',
    status          TEXT NOT NULL DEFAULT 'pending'
                    CHECK (status IN ('pending', 'sending', 'sent', 'failed', 'skipped')),
    attempts        INTEGER NOT NULL DEFAULT 0,
    last_error      TEXT,
    queued_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    sent_at         TIMESTAMPTZ,
    next_retry_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_email_queue_pending
    ON email_queue (next_retry_at)
    WHERE status = 'pending' OR (status = 'failed' AND attempts < 3);

CREATE INDEX IF NOT EXISTS idx_email_queue_notification
    ON email_queue (notification_id);

-- Trigger: khi INSERT notification mới, INSERT tương ứng email_queue
-- nếu user có email + preferences.email_notifications=true.
-- Tránh duplicate: chỉ INSERT 1 row email_queue per notification.
CREATE OR REPLACE FUNCTION fn_enqueue_email_for_notification()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO email_queue (notification_id, recipient, recipient_name, subject, body_html, body_text)
    SELECT
        NEW.id,
        u.email,
        COALESCE(u.display_name, u.username),
        CASE NEW.type
            WHEN 'mention' THEN 'Có người nhắc đến bạn trên Louis Space'
            WHEN 'follow' THEN 'Bạn có người theo dõi mới'
            WHEN 'like' THEN 'Bài viết của bạn được yêu thích'
            WHEN 'comment' THEN 'Có bình luận mới trên bài viết của bạn'
            WHEN 'news_approval' THEN 'Tin tức của bạn đã được duyệt'
            WHEN 'news_rejection' THEN 'Tin tức của bạn bị từ chối'
            ELSE 'Bạn có thông báo mới trên Louis Space'
        END,
        -- Body HTML — full body được compose ở app layer; ở đây placeholder
        -- cho trigger-based path (mention, like, follow). Janitor sẽ fill
        -- body_html đầy đủ khi pickup.
        '',
        NEW.title
    FROM users u
    LEFT JOIN user_preferences up ON up.user_id = u.id
    WHERE u.id = NEW.user_id
      AND u.email IS NOT NULL
      AND u.email != ''
      AND COALESCE(up.email_notifications, TRUE) = TRUE
      AND NEW.user_id != NEW.actor_id;  -- không tự gửi email cho chính actor
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_enqueue_email_on_notification ON notifications;
CREATE TRIGGER trg_enqueue_email_on_notification
    AFTER INSERT ON notifications
    FOR EACH ROW
    EXECUTE FUNCTION fn_enqueue_email_for_notification();
