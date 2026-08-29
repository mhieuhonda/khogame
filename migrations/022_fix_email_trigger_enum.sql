-- 022 — v2.9.0 CRITICAL FIX: trigger email queue dùng SAI tên enum
--
-- VẤN ĐỀ (bug có từ v2.2.0 — migration 017):
-- `fn_enqueue_email_for_notification()` có CASE NEW.type với các literal
-- 'news_approval' / 'news_rejection' — nhưng enum `notification_type`
-- (định nghĩa 001 + ADD VALUE ở 008/013) là 'news_approved' / 'news_rejected'.
--
-- Hậu quả: Postgres parse literal 'news_approval' thành notification_type
-- FAIL runtime. CASE chỉ lỗi khi KHÔNG match branch nào trước đó →
-- mọi notification loại system / review / reply / rating / report_status /
-- new_game / news_approved / news_rejected / news_comment / chat_mention
-- (cho user có email + bật email_notifications) đều ERROR → INSERT
-- notification bị ROLLBACK. App nuốt lỗi bằng `let _ =` nên prod im lặng
-- mất notification (đáng kể nhất: thông báo "tin đã được duyệt", "mở khóa
-- huy hiệu", review mới…).
--
-- FIX: tạo lại function với đúng tên enum. CASE MATCH clause cũng đổi sang
-- so sánh ::text để đời sau thêm enum value không phải sửa trigger nữa.

CREATE OR REPLACE FUNCTION fn_enqueue_email_for_notification()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO email_queue (notification_id, recipient, recipient_name, subject, body_html, body_text)
    SELECT
        NEW.id,
        u.email,
        COALESCE(u.display_name, u.username),
        CASE NEW.type::text
            WHEN 'mention' THEN 'Có người nhắc đến bạn trên Louis Space'
            WHEN 'follow' THEN 'Bạn có người theo dõi mới'
            WHEN 'like' THEN 'Bài viết của bạn được yêu thích'
            WHEN 'comment' THEN 'Có bình luận mới trên bài viết của bạn'
            WHEN 'news_approved' THEN 'Tin tức của bạn đã được duyệt'
            WHEN 'news_rejected' THEN 'Tin tức của bạn bị từ chối'
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
