use crate::error::AppResult;
use crate::models::notification::{NotificationWithActor, ToastItem};
use sqlx::PgPool;
use uuid::Uuid;

pub struct NotificationRepo;

impl NotificationRepo {
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn list_for_user(
        pool: &PgPool,
        user_id: Uuid,
        limit: i64,
        offset: i64,
        only_unread: bool,
    ) -> AppResult<Vec<NotificationWithActor>> {
        let sql = if only_unread {
            r"SELECT n.id, n.user_id, n.actor_id, n.type, n.title, n.content, n.link, n.is_read, n.created_at,
                u.display_name as actor_name, u.avatar_url as actor_avatar
              FROM notifications n
              LEFT JOIN users u ON u.id = n.actor_id
              WHERE n.user_id = $1 AND n.is_read = FALSE
              ORDER BY n.created_at DESC LIMIT $2 OFFSET $3"
        } else {
            r"SELECT n.id, n.user_id, n.actor_id, n.type, n.title, n.content, n.link, n.is_read, n.created_at,
                u.display_name as actor_name, u.avatar_url as actor_avatar
              FROM notifications n
              LEFT JOIN users u ON u.id = n.actor_id
              WHERE n.user_id = $1
              ORDER BY n.created_at DESC LIMIT $2 OFFSET $3"
        };
        let items = sqlx::query_as::<_, NotificationWithActor>(sql)
            .bind(user_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;
        Ok(items)
    }

    /// Tổng số notification của user (phân trang trang thông báo).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count_for_user(pool: &PgPool, user_id: Uuid) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
        Ok(c)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn unread_count(pool: &PgPool, user_id: Uuid) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND is_read = FALSE",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(c)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn mark_read(pool: &PgPool, id: Uuid, user_id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE notifications SET is_read = TRUE WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Lấy 1 notification của đúng user (để re-render item HTMX sau khi
    /// `mark_read` mà không phải fetch cả danh sách).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_for_user(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
    ) -> AppResult<Option<NotificationWithActor>> {
        let n = sqlx::query_as::<_, NotificationWithActor>(
            r"SELECT n.id, n.user_id, n.actor_id, n.type, n.title, n.content, n.link, n.is_read, n.created_at,
                u.display_name as actor_name, u.avatar_url as actor_avatar
              FROM notifications n
              LEFT JOIN users u ON u.id = n.actor_id
              WHERE n.id = $1 AND n.user_id = $2",
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        Ok(n)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn mark_all_read(pool: &PgPool, user_id: Uuid) -> AppResult<()> {
        sqlx::query(
            "UPDATE notifications SET is_read = TRUE WHERE user_id = $1 AND is_read = FALSE",
        )
        .bind(user_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn create_system(
        pool: &PgPool,
        user_id: Uuid,
        title: &str,
        content: &str,
        link: &str,
    ) -> AppResult<()> {
        sqlx::query(
            r"INSERT INTO notifications (user_id, type, title, content, link)
              VALUES ($1, 'system'::notification_type, $2, $3, $4)",
        )
        .bind(user_id)
        .bind(title)
        .bind(content)
        .bind(link)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Gửi thông báo hệ thống tới toàn bộ người dùng đang hoạt động.
    /// Loại trừ AI Agent: tài khoản bot không bao giờ đọc notification
    /// (endpoint /notifications là giao diện người) — mỗi lần broadcast
    /// tạo N dòng chết trong bảng, phình bảng và làm số liệu 'đã gửi'
    /// theo SAO số user thật.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn broadcast(
        pool: &PgPool,
        title: &str,
        content: &str,
        link: &str,
    ) -> AppResult<u64> {
        let res = sqlx::query(
            r"INSERT INTO notifications (user_id, type, title, content, link)
               SELECT id, 'system'::notification_type, $1, $2, $3 FROM users
               WHERE NOT is_banned AND role != 'ai_agent'",
        )
        .bind(title)
        .bind(content)
        .bind(link)
        .execute(pool)
        .await?;
        Ok(res.rows_affected())
    }

    /// Thông báo mention tới 1 user
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn create_mention(
        pool: &PgPool,
        user_id: Uuid,
        actor_id: Uuid,
        game_slug: &str,
    ) -> AppResult<()> {
        sqlx::query(
            r"INSERT INTO notifications (user_id, actor_id, type, title, link)
              VALUES ($1, $2, 'mention'::notification_type, $3, $4)",
        )
        .bind(user_id)
        .bind(actor_id)
        .bind("Có người nhắc đến bạn trong một bình luận")
        .bind(format!("/games/{game_slug}"))
        .execute(pool)
        .await?;
        Ok(())
    }

    /// v3.14.0 — Thông báo lời mời kết bạn (link tới trang bạn bè).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn create_friend_request(
        pool: &PgPool,
        user_id: Uuid,
        actor_id: Uuid,
        actor_name: &str,
    ) -> AppResult<()> {
        sqlx::query(
            r"INSERT INTO notifications (user_id, actor_id, type, title, link)
              VALUES ($1, $2, 'friend_request'::notification_type, $3, '/friends')",
        )
        .bind(user_id)
        .bind(actor_id)
        .bind(format!("{actor_name} muốn kết bạn với bạn"))
        .execute(pool)
        .await?;
        Ok(())
    }

    /// v3.14.0 — Thông báo tin nhắn riêng mới (link tới thread).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn create_dm(
        pool: &PgPool,
        user_id: Uuid,
        actor_id: Uuid,
        actor_name: &str,
        link: &str,
    ) -> AppResult<()> {
        sqlx::query(
            r"INSERT INTO notifications (user_id, actor_id, type, title, link)
              VALUES ($1, $2, 'dm'::notification_type, $3, $4)",
        )
        .bind(user_id)
        .bind(actor_id)
        .bind(format!("{actor_name} đã nhắn tin cho bạn"))
        .bind(link)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// v3.14.0 — Batch: báo tin nhắn mới tới MỌI thành viên hội thoại trừ
    /// người gửi (1 query INSERT..SELECT — nhóm 50 người vẫn 1 round-trip).
    /// Bỏ qua user bị ban (không đọc notification).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn create_dm_batch(
        pool: &PgPool,
        conversation_id: Uuid,
        actor_id: Uuid,
        title: &str,
        link: &str,
    ) -> AppResult<()> {
        sqlx::query(
            r"INSERT INTO notifications (user_id, actor_id, type, title, link)
              SELECT m.user_id, $2, 'dm'::notification_type, $3, $4
              FROM chat_members m
              JOIN users u ON u.id = m.user_id
              WHERE m.conversation_id = $1
                AND m.user_id != $2
                AND NOT u.is_banned",
        )
        .bind(conversation_id)
        .bind(actor_id)
        .bind(title)
        .bind(link)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// v3.16.0 — Thông báo mở khóa huy hiệu (type riêng để toast realtime
    /// lọc được — trước đây dùng chung 'system').
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn create_achievement(
        pool: &PgPool,
        user_id: Uuid,
        title: &str,
        content: &str,
    ) -> AppResult<()> {
        sqlx::query(
            r"INSERT INTO notifications (user_id, type, title, content, link)
              VALUES ($1, 'achievement'::notification_type, $2, $3, '/achievements')",
        )
        .bind(user_id)
        .bind(title)
        .bind(content)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// v3.16.0 — Thông báo lên cấp (type riêng để toast realtime lọc được).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn create_level_up(
        pool: &PgPool,
        user_id: Uuid,
        title: &str,
        content: &str,
        link: &str,
    ) -> AppResult<()> {
        sqlx::query(
            r"INSERT INTO notifications (user_id, type, title, content, link)
              VALUES ($1, 'level_up'::notification_type, $2, $3, $4)",
        )
        .bind(user_id)
        .bind(title)
        .bind(content)
        .bind(link)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// v3.16.0 — Lấy + đánh dấu announced cho toast huy hiệu/lên cấp chưa
    /// báo (1 CTE atomic: SELECT ids → UPDATE → RETURNING rows — không bao
    /// giờ báo trùng, đa thiết bị/tab OK). Tối đa 5/lần poll.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn take_toasts(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<ToastItem>> {
        let rows = sqlx::query_as::<_, ToastItem>(
            r"WITH q AS (
                 SELECT id FROM notifications
                 WHERE user_id = $1 AND announced = FALSE AND is_read = FALSE
                   AND type IN ('achievement'::notification_type, 'level_up'::notification_type)
                 ORDER BY created_at DESC LIMIT 5
               ),
               upd AS (
                 UPDATE notifications SET announced = TRUE WHERE id IN (SELECT id FROM q)
               )
               SELECT n.id, n.type::text AS kind, n.title, n.content, n.link
               FROM notifications n WHERE n.id IN (SELECT id FROM q)
               ORDER BY n.created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// v2.2.0 — Batch mention tới nhiều user trong 1 query.
    /// Trước đây comment mention 10 user = 10 sequential INSERT (N+1).
    /// Giờ là 1 INSERT ... SELECT FROM unnest(...) — giảm round-trip DB.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn create_mentions_batch(
        pool: &PgPool,
        user_ids: &[Uuid],
        actor_id: Uuid,
        game_slug: &str,
    ) -> AppResult<()> {
        if user_ids.is_empty() {
            return Ok(());
        }
        // v3.0.0 — lọc theo prefs: user tắt 'mention' không nhận (LEFT JOIN
        // prefs — vắng row = TRUE, hành vi cũ bảo toàn)
        sqlx::query(
            r"INSERT INTO notifications (user_id, actor_id, type, title, link)
              SELECT u, $2, 'mention'::notification_type, $3, $4
              FROM unnest($1::uuid[]) AS u
              LEFT JOIN user_notification_prefs p ON p.user_id = u
              WHERE COALESCE(p.inapp_mention, TRUE)",
        )
        .bind(user_ids)
        .bind(actor_id)
        .bind("Có người nhắc đến bạn trong một bình luận")
        .bind(format!("/games/{game_slug}"))
        .execute(pool)
        .await?;
        Ok(())
    }

    /// v2.2.0 — Batch mention cho news comments (link khác với game comments).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn create_mentions_batch_news(
        pool: &PgPool,
        user_ids: &[Uuid],
        actor_id: Uuid,
        link: &str,
    ) -> AppResult<()> {
        if user_ids.is_empty() {
            return Ok(());
        }
        // v3.0.0 — lọc theo prefs: user tắt 'mention' không nhận (LEFT JOIN
        // prefs — vắng row = TRUE, hành vi cũ bảo toàn)
        sqlx::query(
            r"INSERT INTO notifications (user_id, actor_id, type, title, link)
              SELECT u, $2, 'mention'::notification_type, $3, $4
              FROM unnest($1::uuid[]) AS u
              LEFT JOIN user_notification_prefs p ON p.user_id = u
              WHERE COALESCE(p.inapp_mention, TRUE)",
        )
        .bind(user_ids)
        .bind(actor_id)
        .bind("Có người nhắc đến bạn trong một bình luận tin tức")
        .bind(link)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Xoá notification ĐÃ ĐỌC cũ hơn `days` ngày. Notification chưa đọc
    /// được giữ nguyên toàn bộ. Trả về số dòng đã xoá.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn cleanup_read_older_than(pool: &PgPool, days: i64) -> AppResult<u64> {
        let res = sqlx::query(
            r"DELETE FROM notifications
               WHERE is_read = TRUE AND created_at < NOW() - ($1 || ' days')::INTERVAL",
        )
        .bind(days.to_string())
        .execute(pool)
        .await?;
        Ok(res.rows_affected())
    }
}
