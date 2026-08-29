-- 020 — v2.8.1 BUGFIX: chống spam notification khi toggle like/follow lặp lại
--
-- VẤN ĐỀ (tìm thấy khi audit v2.8.0):
-- Trigger `increment_like_count` (001_init.sql) insert notification 'like'
-- cho MỖI lần INSERT vào bảng `likes`. User gỡ like rồi like lại → 1 dòng
-- likes mới → 1 notification mới. Cả like lẫn follow đều nằm trong rate-limit
-- bucket 120/phút → kẻ xấu cycle like/unlike có thể dìm nạn nhân
-- ~60 notification/phút + đẩy email hàng loạt qua trigger email queue
-- (017) vì email_notifications mặc định TRUE.
--
-- FIX: chỉ insert notification khi CHƯA tồn tại thông báo CÙNG loại,
-- cùng actor, CHƯA ĐỌC cho cùng mục tiêu. Người nhận đã đọc thông báo cũ
-- thì lần like sau vẫn được thông báo (giữ hành vi hợp lý), nhưng không
-- thể bị flood nữa.
--
-- An toàn re-run: CREATE OR REPLACE FUNCTION là idempotent.

CREATE OR REPLACE FUNCTION increment_like_count()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE games SET like_count = like_count + 1 WHERE id = NEW.game_id;
    -- v2.8.1: dedup notification theo (user, actor, type='like', link, unread)
    INSERT INTO notifications (user_id, actor_id, type, title, link)
    SELECT g.user_id, NEW.user_id, 'like', 'Có người vừa thích game của bạn',
        '/games/' || g.slug
    FROM games g
    WHERE g.id = NEW.game_id
      AND g.user_id != NEW.user_id
      AND NOT EXISTS (
          SELECT 1 FROM notifications n
          WHERE n.user_id = g.user_id
            AND n.actor_id = NEW.user_id
            AND n.type = 'like'
            AND n.is_read = FALSE
            AND n.link = '/games/' || g.slug
      );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
