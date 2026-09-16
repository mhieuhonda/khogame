//! Repository cho hội thoại DM + nhóm chat (v3.14.0, migration 050).
//!
//! Hiệu năng:
//! - DM tra bằng `dm_key` UNIQUE (O(1)), không JOIN members.
//! - Inbox: 1 query duy nhất (LATERAL lấy tin cuối + unread), LIMIT 30.
//! - Thread: index `(conversation_id, created_at DESC)` + LIMIT 30.
//! - Mọi write kiểm tra membership TRƯỚC bằng query PK (rẻ).

use crate::error::AppResult;
use crate::models::{Conversation, DmMessageWithSender, GroupMemberInfo, InboxItem};
use sqlx::PgPool;
use uuid::Uuid;

/// Tối đa thành viên mỗi nhóm (chống spam/abuse tạo nhóm nghìn người).
pub const MAX_GROUP_MEMBERS: i64 = 50;
/// Số tin load mỗi lần poll thread.
pub const THREAD_LIMIT: i64 = 30;

pub struct DmRepo;

/// Tính `dm_key` chuẩn cho cặp user (sort để A:B == B:A).
#[must_use]
pub fn dm_key(a: Uuid, b: Uuid) -> String {
    if a.as_bytes() < b.as_bytes() {
        format!("{a}:{b}")
    } else {
        format!("{b}:{a}")
    }
}

impl DmRepo {
    /// Tìm DM của cặp user theo `dm_key` (O(1)).
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn find_dm(pool: &PgPool, a: Uuid, b: Uuid) -> AppResult<Option<Conversation>> {
        let row = sqlx::query_as::<_, Conversation>(
            r"SELECT id, kind, dm_key, name, avatar_url, created_by, created_at, updated_at
              FROM chat_conversations WHERE kind = 'dm' AND dm_key = $1",
        )
        .bind(dm_key(a, b))
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// Lấy hoặc tạo DM cho cặp user (atomic — 2 request đồng thời không
    /// tạo trùng nhờ UNIQUE(dm_key) + ON CONFLICT).
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn get_or_create_dm(
        pool: &PgPool,
        creator: Uuid,
        other: Uuid,
    ) -> AppResult<Conversation> {
        let key = dm_key(creator, other);
        let mut tx = pool.begin().await?;
        let conv = sqlx::query_as::<_, Conversation>(
            r"INSERT INTO chat_conversations (kind, dm_key, created_by)
              VALUES ('dm', $1, $2)
              ON CONFLICT (dm_key) DO UPDATE SET updated_at = chat_conversations.updated_at
              RETURNING id, kind, dm_key, name, avatar_url, created_by, created_at, updated_at",
        )
        .bind(&key)
        .bind(creator)
        .fetch_one(&mut *tx)
        .await?;
        // Thêm 2 thành viên (idempotent — vào lại DM cũ không lỗi).
        sqlx::query(
            r"INSERT INTO chat_members (conversation_id, user_id, role)
              VALUES ($1, $2, 'member'), ($1, $3, 'member')
              ON CONFLICT DO NOTHING",
        )
        .bind(conv.id)
        .bind(creator)
        .bind(other)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(conv)
    }

    /// Tạo nhóm chat (creator = owner). `member_ids` đã validate là bạn bè
    /// ở handler — repo chỉ insert.
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn create_group(
        pool: &PgPool,
        creator: Uuid,
        name: &str,
        member_ids: &[Uuid],
    ) -> AppResult<Conversation> {
        let mut tx = pool.begin().await?;
        let conv = sqlx::query_as::<_, Conversation>(
            r"INSERT INTO chat_conversations (kind, name, created_by)
              VALUES ('group', $1, $2)
              RETURNING id, kind, dm_key, name, avatar_url, created_by, created_at, updated_at",
        )
        .bind(name)
        .bind(creator)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            r"INSERT INTO chat_members (conversation_id, user_id, role)
              VALUES ($1, $2, 'owner')",
        )
        .bind(conv.id)
        .bind(creator)
        .execute(&mut *tx)
        .await?;
        // Thêm thành viên (bỏ trùng + bỏ chính creator) — 1 query unnest.
        let others: Vec<Uuid> = member_ids
            .iter()
            .copied()
            .filter(|id| *id != creator)
            .collect();
        if !others.is_empty() {
            sqlx::query(
                r"INSERT INTO chat_members (conversation_id, user_id, role)
                  SELECT $1, unnest($2::uuid[]), 'member'
                  ON CONFLICT DO NOTHING",
            )
            .bind(conv.id)
            .bind(&others)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(conv)
    }

    /// Vai trò của user trong hội thoại (None = không phải thành viên).
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn member_role(
        pool: &PgPool,
        conversation_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<Option<String>> {
        let role: Option<String> = sqlx::query_scalar(
            r"SELECT role FROM chat_members WHERE conversation_id = $1 AND user_id = $2",
        )
        .bind(conversation_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        Ok(role)
    }

    /// Lấy hội thoại theo id.
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn find_conversation(
        pool: &PgPool,
        conversation_id: Uuid,
    ) -> AppResult<Option<Conversation>> {
        let row = sqlx::query_as::<_, Conversation>(
            r"SELECT id, kind, dm_key, name, avatar_url, created_by, created_at, updated_at
              FROM chat_conversations WHERE id = $1",
        )
        .bind(conversation_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// Inbox của user: hội thoại của mình + preview tin cuối + unread —
    /// TẤT CẢ trong 1 query (LATERAL subquery, không N+1).
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn inbox(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<InboxItem>> {
        let rows = sqlx::query_as::<_, InboxItem>(
            r"SELECT c.id, c.kind, c.name, c.avatar_url, c.updated_at,
                m.role AS my_role,
                (SELECT COUNT(*) FROM chat_members WHERE conversation_id = c.id) AS member_count,
                last.content AS last_content,
                last.image_url AS last_image,
                lu.display_name AS last_sender_name,
                last.created_at AS last_at,
                (SELECT COUNT(*) FROM dm_messages d
                  WHERE d.conversation_id = c.id
                    AND d.is_deleted = FALSE
                    AND d.sender_id != $1
                    AND d.created_at > COALESCE(m.last_read_at, 'epoch'::timestamptz)
                ) AS unread,
                ou.username AS other_username,
                ou.display_name AS other_display_name,
                ou.avatar_url AS other_avatar_url
              FROM chat_conversations c
              JOIN chat_members m ON m.conversation_id = c.id AND m.user_id = $1
              LEFT JOIN LATERAL (
                SELECT d.content, d.image_url, d.sender_id, d.created_at
                FROM dm_messages d
                WHERE d.conversation_id = c.id AND d.is_deleted = FALSE
                ORDER BY d.created_at DESC LIMIT 1
              ) last ON TRUE
              LEFT JOIN users lu ON lu.id = last.sender_id
              LEFT JOIN LATERAL (
                SELECT u.username, u.display_name, u.avatar_url
                FROM chat_members m2
                JOIN users u ON u.id = m2.user_id
                WHERE m2.conversation_id = c.id AND m2.user_id != $1
                LIMIT 1
              ) ou ON c.kind = 'dm'
              ORDER BY c.updated_at DESC
              LIMIT 30",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Tổng số tin chưa đọc mọi hội thoại (badge menu).
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn total_unread(pool: &PgPool, user_id: Uuid) -> AppResult<i64> {
        let n: i64 = sqlx::query_scalar(
            r"SELECT COUNT(*) FROM dm_messages d
              JOIN chat_members m ON m.conversation_id = d.conversation_id AND m.user_id = $1
              WHERE d.is_deleted = FALSE
                AND d.sender_id != $1
                AND d.created_at > COALESCE(m.last_read_at, 'epoch'::timestamptz)",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(n)
    }

    /// N tin mới nhất của thread (DESC — caller reverse để render cũ→mới).
    /// Chỉ gọi sau khi đã check membership ở handler.
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn thread(
        pool: &PgPool,
        conversation_id: Uuid,
        limit: i64,
    ) -> AppResult<Vec<DmMessageWithSender>> {
        let rows = sqlx::query_as::<_, DmMessageWithSender>(
            r"SELECT d.id, d.conversation_id, d.sender_id, d.content, d.image_url,
                d.is_deleted, d.created_at,
                u.username, u.display_name, u.avatar_url, u.role::text AS role
              FROM dm_messages d
              JOIN users u ON u.id = d.sender_id
              WHERE d.conversation_id = $1
              ORDER BY d.created_at DESC
              LIMIT $2",
        )
        .bind(conversation_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Gửi tin (text và/hoặc ảnh) + bump `updated_at` hội thoại (inbox sort).
    /// Trả message đã JOIN sender (render ngay, không query lại).
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn send(
        pool: &PgPool,
        conversation_id: Uuid,
        sender_id: Uuid,
        content: &str,
        image_url: Option<&str>,
    ) -> AppResult<DmMessageWithSender> {
        let mut tx = pool.begin().await?;
        let msg = sqlx::query_as::<_, DmMessageWithSender>(
            r"WITH inserted AS (
                 INSERT INTO dm_messages (conversation_id, sender_id, content, image_url)
                 VALUES ($1, $2, $3, $4)
                 RETURNING id, conversation_id, sender_id, content, image_url, is_deleted, created_at
               )
               SELECT i.id, i.conversation_id, i.sender_id, i.content, i.image_url,
                 i.is_deleted, i.created_at,
                 u.username, u.display_name, u.avatar_url, u.role::text AS role
               FROM inserted i JOIN users u ON u.id = i.sender_id",
        )
        .bind(conversation_id)
        .bind(sender_id)
        .bind(content)
        .bind(image_url)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("UPDATE chat_conversations SET updated_at = NOW() WHERE id = $1")
            .bind(conversation_id)
            .execute(&mut *tx)
            .await?;
        // Người gửi đã "đọc" tới tin của mình (không tự tính unread).
        sqlx::query(
            r"UPDATE chat_members SET last_read_at = NOW()
              WHERE conversation_id = $1 AND user_id = $2",
        )
        .bind(conversation_id)
        .bind(sender_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(msg)
    }

    /// Đánh dấu đã đọc tới mốc `upto` (mốc mới nhất đã fetch/render).
    ///
    /// v3.16.0 FIX (LOW-10): trước đây `last_read_at = NOW()` — tin đến
    /// đúng khe hở giữa SELECT và UPDATE bị đánh dấu đã đọc dù chưa render.
    /// Giờ chốt theo max(created_at) đã fetch + GREATEST chống thụt lùi.
    /// `upto=None` (vừa gửi tin) → NOW().
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn mark_read(
        pool: &PgPool,
        conversation_id: Uuid,
        user_id: Uuid,
        upto: Option<chrono::DateTime<chrono::Utc>>,
    ) -> AppResult<()> {
        match upto {
            Some(t) => {
                sqlx::query(
                    r"UPDATE chat_members
                      SET last_read_at = GREATEST(COALESCE(last_read_at, 'epoch'::timestamptz), $3)
                      WHERE conversation_id = $1 AND user_id = $2",
                )
                .bind(conversation_id)
                .bind(user_id)
                .bind(t)
                .execute(pool)
                .await?;
            }
            None => {
                sqlx::query(
                    r"UPDATE chat_members SET last_read_at = NOW()
                      WHERE conversation_id = $1 AND user_id = $2",
                )
                .bind(conversation_id)
                .bind(user_id)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    /// Soft-delete tin của chính mình (hoặc staff xóa mọi tin).
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn soft_delete(
        pool: &PgPool,
        message_id: Uuid,
        sender_id: Option<Uuid>,
    ) -> AppResult<bool> {
        let rows = if let Some(uid) = sender_id {
            sqlx::query(
                r"UPDATE dm_messages SET is_deleted = TRUE
                  WHERE id = $1 AND sender_id = $2 AND is_deleted = FALSE",
            )
            .bind(message_id)
            .bind(uid)
            .execute(pool)
            .await?
            .rows_affected()
        } else {
            sqlx::query(
                r"UPDATE dm_messages SET is_deleted = TRUE
                  WHERE id = $1 AND is_deleted = FALSE",
            )
            .bind(message_id)
            .execute(pool)
            .await?
            .rows_affected()
        };
        Ok(rows > 0)
    }

    /// Thêm thành viên vào nhóm (owner/admin gọi) — chỉ thêm bạn bè của
    /// người mời (check ở handler), bỏ qua người đã trong nhóm.
    /// Trả số row thực thêm.
    ///
    /// v3.16.0 FIX (MED-3 — race vượt cap): 2 admin thêm đồng thời cùng đọc
    /// count rồi cùng INSERT → vượt 50. Giờ khóa advisory theo conversation
    /// trong tx + cắt input vừa đúng slot còn lại → cap không bao giờ vỡ.
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn add_members(
        pool: &PgPool,
        conversation_id: Uuid,
        user_ids: &[Uuid],
    ) -> AppResult<u64> {
        if user_ids.is_empty() {
            return Ok(0);
        }
        let mut tx = pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(format!("group-add:{conversation_id}"))
            .execute(&mut *tx)
            .await?;
        let count: i64 =
            sqlx::query_scalar(r"SELECT COUNT(*) FROM chat_members WHERE conversation_id = $1")
                .bind(conversation_id)
                .fetch_one(&mut *tx)
                .await?;
        let room = (MAX_GROUP_MEMBERS - count).max(0) as usize;
        let take = user_ids.len().min(room);
        if take == 0 {
            return Ok(0);
        }
        let n = sqlx::query(
            r"INSERT INTO chat_members (conversation_id, user_id, role)
              SELECT $1, unnest($2::uuid[]), 'member'
              ON CONFLICT DO NOTHING",
        )
        .bind(conversation_id)
        .bind(&user_ids[..take])
        .execute(&mut *tx)
        .await?
        .rows_affected();
        tx.commit().await?;
        Ok(n)
    }

    /// Đếm thành viên nhóm (enforce `MAX_GROUP_MEMBERS` ở handler).
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn member_count(pool: &PgPool, conversation_id: Uuid) -> AppResult<i64> {
        let n: i64 =
            sqlx::query_scalar(r"SELECT COUNT(*) FROM chat_members WHERE conversation_id = $1")
                .bind(conversation_id)
                .fetch_one(pool)
                .await?;
        Ok(n)
    }

    /// Xóa thành viên khỏi nhóm (owner/admin xóa member; owner không bị xóa).
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn remove_member(
        pool: &PgPool,
        conversation_id: Uuid,
        target: Uuid,
    ) -> AppResult<bool> {
        let rows = sqlx::query(
            r"DELETE FROM chat_members
              WHERE conversation_id = $1 AND user_id = $2 AND role != 'owner'",
        )
        .bind(conversation_id)
        .bind(target)
        .execute(pool)
        .await?
        .rows_affected();
        Ok(rows > 0)
    }

    /// Rời nhóm (member/admin rời; owner phải chuyển quyền hoặc xóa nhóm —
    /// handler chặn owner rời khi còn thành viên khác).
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn leave(pool: &PgPool, conversation_id: Uuid, user_id: Uuid) -> AppResult<bool> {
        let rows =
            sqlx::query(r"DELETE FROM chat_members WHERE conversation_id = $1 AND user_id = $2")
                .bind(conversation_id)
                .bind(user_id)
                .execute(pool)
                .await?
                .rows_affected();
        Ok(rows > 0)
    }

    /// Xóa nhóm (chỉ owner) — CASCADE xóa members + messages.
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn delete_group(pool: &PgPool, conversation_id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM chat_conversations WHERE id = $1 AND kind = 'group'")
            .bind(conversation_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Đổi tên nhóm (owner/admin).
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn rename_group(pool: &PgPool, conversation_id: Uuid, name: &str) -> AppResult<()> {
        sqlx::query(
            r"UPDATE chat_conversations SET name = $1, updated_at = NOW()
              WHERE id = $2 AND kind = 'group'",
        )
        .bind(name)
        .bind(conversation_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Danh sách thành viên nhóm kèm info hiển thị.
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn members(pool: &PgPool, conversation_id: Uuid) -> AppResult<Vec<GroupMemberInfo>> {
        let rows = sqlx::query_as::<_, GroupMemberInfo>(
            r"SELECT m.user_id, m.role, m.joined_at, u.username, u.display_name, u.avatar_url
              FROM chat_members m JOIN users u ON u.id = m.user_id
              WHERE m.conversation_id = $1
              ORDER BY CASE m.role WHEN 'owner' THEN 0 WHEN 'admin' THEN 1 ELSE 2 END, m.joined_at",
        )
        .bind(conversation_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Đối phương trong DM (member còn lại) — để render tên thread + gate
    /// "vẫn còn là bạn không".
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn dm_other(
        pool: &PgPool,
        conversation_id: Uuid,
        me: Uuid,
    ) -> AppResult<Option<GroupMemberInfo>> {
        let row = sqlx::query_as::<_, GroupMemberInfo>(
            r"SELECT m.user_id, m.role, m.joined_at, u.username, u.display_name, u.avatar_url
              FROM chat_members m JOIN users u ON u.id = m.user_id
              WHERE m.conversation_id = $1 AND m.user_id != $2
              LIMIT 1",
        )
        .bind(conversation_id)
        .bind(me)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }
}
