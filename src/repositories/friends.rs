//! Repository cho kết bạn (v3.14.0, migration 050).
//!
//! Quy ước: mỗi cặp user có TỐI ĐA 1 row `friendships` (UNIQUE
//! requester/addressee). Tra cứu "quan hệ giữa A và B" phải check cả 2
//! chiều (`requester=A AND addressee=B OR ngược lại`).
//!
//! Hiệu năng: mọi lookup dùng PK/UNIQUE + 2 index (requester, addressee)
//! — O(1), không scan.

use crate::error::AppResult;
use crate::models::{Friendship, FriendshipWithUser};
use sqlx::PgPool;
use uuid::Uuid;

pub struct FriendRepo;

impl FriendRepo {
    /// Quan hệ giữa 2 user (bất kể chiều), None nếu chưa từng tương tác.
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn between(pool: &PgPool, a: Uuid, b: Uuid) -> AppResult<Option<Friendship>> {
        let row = sqlx::query_as::<_, Friendship>(
            r"SELECT id, requester_id, addressee_id, status::text AS status, created_at, updated_at
              FROM friendships
              WHERE (requester_id = $1 AND addressee_id = $2)
                 OR (requester_id = $2 AND addressee_id = $1)",
        )
        .bind(a)
        .bind(b)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// Hai user có phải bạn bè (accepted) không — gate cho DM.
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn are_friends(pool: &PgPool, a: Uuid, b: Uuid) -> AppResult<bool> {
        let exists: Option<i32> = sqlx::query_scalar(
            r"SELECT 1 FROM friendships
              WHERE ((requester_id = $1 AND addressee_id = $2)
                 OR (requester_id = $2 AND addressee_id = $1))
                AND status = 'accepted'",
        )
        .bind(a)
        .bind(b)
        .fetch_optional(pool)
        .await?;
        Ok(exists.is_some())
    }

    /// Gửi lời mời kết bạn. Idempotent theo cặp: đã có row → trả row cũ
    /// (handler quyết định báo "đã gửi rồi" / "đã là bạn").
    ///
    /// v3.16.0 FIX (HIGH-1 — 2 row ngược chiều): UNIQUE chỉ chặn trùng cùng
    /// chiều nên lời mời ngược (B→A khi đã có A→B declined, hoặc race gửi
    /// đồng thời) tạo row thứ 2 → `between()` trả row bất kỳ, accept/cancel
    /// tác động sai row, lời mời ma treo vĩnh viễn. Giờ XÓA row ngược chiều
    /// trước khi insert → mỗi cặp tối đa 1 row (khe race ms còn lại được
    /// `respond()` dọn khi accept).
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn request(pool: &PgPool, requester: Uuid, addressee: Uuid) -> AppResult<Friendship> {
        let mut tx = pool.begin().await?;
        sqlx::query(r"DELETE FROM friendships WHERE requester_id = $1 AND addressee_id = $2")
            .bind(addressee)
            .bind(requester)
            .execute(&mut *tx)
            .await?;
        let row = sqlx::query_as::<_, Friendship>(
            r"INSERT INTO friendships (requester_id, addressee_id, status)
              VALUES ($1, $2, 'pending')
              ON CONFLICT (requester_id, addressee_id)
              DO UPDATE SET updated_at = NOW()
              RETURNING id, requester_id, addressee_id, status::text AS status, created_at, updated_at",
        )
        .bind(requester)
        .bind(addressee)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(row)
    }

    /// Gửi lại sau declined: xóa MỌI row của cặp (cùng + ngược chiều) rồi
    /// tạo lời mời pending mới — giữ invariant 1 row/cặp (xem `request`).
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn resend(pool: &PgPool, requester: Uuid, addressee: Uuid) -> AppResult<Friendship> {
        let mut tx = pool.begin().await?;
        sqlx::query(
            r"DELETE FROM friendships
              WHERE (requester_id = $1 AND addressee_id = $2)
                 OR (requester_id = $2 AND addressee_id = $1)",
        )
        .bind(requester)
        .bind(addressee)
        .execute(&mut *tx)
        .await?;
        let row = sqlx::query_as::<_, Friendship>(
            r"INSERT INTO friendships (requester_id, addressee_id, status)
              VALUES ($1, $2, 'pending')
              RETURNING id, requester_id, addressee_id, status::text AS status, created_at, updated_at",
        )
        .bind(requester)
        .bind(addressee)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(row)
    }

    /// Chấp nhận / từ chối lời mời (chỉ addressee của lời pending).
    /// Trả true nếu có row được đổi trạng thái.
    ///
    /// v3.16.0 FIX (HIGH-1): khi accept, dọn nốt row lạc cùng cặp (tồn kho
    /// từ race trước fix) để invariant 1 row/cặp được phục hồi.
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn respond(
        pool: &PgPool,
        friendship_id: Uuid,
        addressee: Uuid,
        accept: bool,
    ) -> AppResult<bool> {
        let status = if accept { "accepted" } else { "declined" };
        let mut tx = pool.begin().await?;
        let pair: Option<(Uuid, Uuid)> = sqlx::query_as(
            r"UPDATE friendships SET status = $1::friend_status, updated_at = NOW()
              WHERE id = $2 AND addressee_id = $3 AND status = 'pending'
              RETURNING requester_id, addressee_id",
        )
        .bind(status)
        .bind(friendship_id)
        .bind(addressee)
        .fetch_optional(&mut *tx)
        .await?;
        let changed = pair.is_some();
        if accept {
            if let Some((req, addr)) = pair {
                sqlx::query(
                    r"DELETE FROM friendships
                      WHERE id != $1
                        AND ((requester_id = $2 AND addressee_id = $3)
                          OR (requester_id = $3 AND addressee_id = $2))",
                )
                .bind(friendship_id)
                .bind(req)
                .bind(addr)
                .execute(&mut *tx)
                .await?;
            }
        }
        tx.commit().await?;
        Ok(changed)
    }

    /// Hủy lời mời đã gửi (chỉ requester, chỉ khi còn pending).
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn cancel(pool: &PgPool, friendship_id: Uuid, requester: Uuid) -> AppResult<bool> {
        let rows = sqlx::query(
            r"DELETE FROM friendships
              WHERE id = $1 AND requester_id = $2 AND status = 'pending'",
        )
        .bind(friendship_id)
        .bind(requester)
        .execute(pool)
        .await?
        .rows_affected();
        Ok(rows > 0)
    }

    /// Hủy kết bạn (xóa row accepted — 1 trong 2 bên đều được).
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn unfriend(pool: &PgPool, a: Uuid, b: Uuid) -> AppResult<bool> {
        let rows = sqlx::query(
            r"DELETE FROM friendships
              WHERE ((requester_id = $1 AND addressee_id = $2)
                 OR (requester_id = $2 AND addressee_id = $1))
                AND status = 'accepted'",
        )
        .bind(a)
        .bind(b)
        .execute(pool)
        .await?
        .rows_affected();
        Ok(rows > 0)
    }

    /// Chặn user: upsert thành blocked (ghi đè mọi trạng thái cũ, bất kể
    /// chiều — row duy nhất của cặp được chuẩn hoá về blocker→blocked).
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn block(pool: &PgPool, blocker: Uuid, blocked: Uuid) -> AppResult<()> {
        // Xóa row cũ (nếu ngược chiều) rồi insert chuẩn chiều blocker→blocked.
        let mut tx = pool.begin().await?;
        sqlx::query(
            r"DELETE FROM friendships
              WHERE (requester_id = $1 AND addressee_id = $2)
                 OR (requester_id = $2 AND addressee_id = $1)",
        )
        .bind(blocker)
        .bind(blocked)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r"INSERT INTO friendships (requester_id, addressee_id, status)
              VALUES ($1, $2, 'blocked')",
        )
        .bind(blocker)
        .bind(blocked)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Bỏ chặn (chỉ người đã chặn).
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn unblock(pool: &PgPool, blocker: Uuid, blocked: Uuid) -> AppResult<bool> {
        let rows = sqlx::query(
            r"DELETE FROM friendships
              WHERE requester_id = $1 AND addressee_id = $2 AND status = 'blocked'",
        )
        .bind(blocker)
        .bind(blocked)
        .execute(pool)
        .await?
        .rows_affected();
        Ok(rows > 0)
    }

    /// Danh sách bạn bè (accepted) của user + tìm kiếm theo tên.
    /// 1 query JOIN users — không N+1.
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    #[allow(clippy::too_many_arguments)]
    pub async fn list_friends(
        pool: &PgPool,
        user_id: Uuid,
        search: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<FriendshipWithUser>> {
        let pattern = format!(
            "%{}%",
            crate::utils::escape_like(
                &search
                    .unwrap_or_default()
                    .chars()
                    .take(50)
                    .collect::<String>()
            )
        );
        let rows = sqlx::query_as::<_, FriendshipWithUser>(
            r"SELECT f.id, f.requester_id, f.addressee_id, f.status::text AS status, f.created_at,
                (f.requester_id = $1) AS is_requester,
                u.username, u.display_name, u.avatar_url
              FROM friendships f
              JOIN users u ON u.id = CASE WHEN f.requester_id = $1 THEN f.addressee_id ELSE f.requester_id END
              WHERE ((f.requester_id = $1 OR f.addressee_id = $1))
                AND f.status = 'accepted'
                AND (u.username ILIKE $2 OR u.display_name ILIKE $2)
                AND NOT u.is_banned
              ORDER BY f.updated_at DESC
              LIMIT $3 OFFSET $4",
        )
        .bind(user_id)
        .bind(pattern)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Lời mời ĐẾN mình đang pending (inbox "Lời mời kết bạn").
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn list_incoming(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<FriendshipWithUser>> {
        let rows = sqlx::query_as::<_, FriendshipWithUser>(
            r"SELECT f.id, f.requester_id, f.addressee_id, f.status::text AS status, f.created_at,
                FALSE AS is_requester,
                u.username, u.display_name, u.avatar_url
              FROM friendships f
              JOIN users u ON u.id = f.requester_id
              WHERE f.addressee_id = $1 AND f.status = 'pending'
                AND NOT u.is_banned
              ORDER BY f.created_at DESC
              LIMIT 50",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Lời mời DO mình gửi đang pending.
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn list_outgoing(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<FriendshipWithUser>> {
        let rows = sqlx::query_as::<_, FriendshipWithUser>(
            r"SELECT f.id, f.requester_id, f.addressee_id, f.status::text AS status, f.created_at,
                TRUE AS is_requester,
                u.username, u.display_name, u.avatar_url
              FROM friendships f
              JOIN users u ON u.id = f.addressee_id
              WHERE f.requester_id = $1 AND f.status = 'pending'
                AND NOT u.is_banned
              ORDER BY f.created_at DESC
              LIMIT 50",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Đếm bạn bè (badge trang friends).
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn count_friends(pool: &PgPool, user_id: Uuid) -> AppResult<i64> {
        let n: i64 = sqlx::query_scalar(
            r"SELECT COUNT(*) FROM friendships
              WHERE (requester_id = $1 OR addressee_id = $1) AND status = 'accepted'",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(n)
    }

    /// Đếm lời mời đến đang pending (badge).
    ///
    /// # Errors
    /// Trả về lỗi khi DB fail.
    pub async fn count_incoming(pool: &PgPool, user_id: Uuid) -> AppResult<i64> {
        let n: i64 = sqlx::query_scalar(
            r"SELECT COUNT(*) FROM friendships WHERE addressee_id = $1 AND status = 'pending'",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(n)
    }
}
