use crate::error::AppResult;
use crate::models::comment::CommentWithUser;
use crate::models::CommentWithGame;
use sqlx::PgPool;
use uuid::Uuid;

pub struct CommentRepo;

impl CommentRepo {
    pub async fn create(
        pool: &PgPool,
        game_id: Uuid,
        user_id: Uuid,
        parent_id: Option<Uuid>,
        content: &str,
    ) -> AppResult<Uuid> {
        let id: Uuid = sqlx::query_scalar(
            r"INSERT INTO comments (game_id, user_id, parent_id, content)
              VALUES ($1, $2, $3, $4) RETURNING id",
        )
        .bind(game_id)
        .bind(user_id)
        .bind(parent_id)
        .bind(content)
        .fetch_one(pool)
        .await?;

        // games.comment_count được cập nhật bởi DB trigger
        // (trigger_comment_insert) — không tự cộng lại tại đây.

        // === Notification routing (sửa logic sai từ trước) ===
        // Trước đây: reply cũng thông báo cho CHỦ GAME với nội dung
        // "Có người vừa trả lời bình luận của bạn" — sai người nhận:
        // nếu B trả lời bình luận của A trên game của C thì C nhận thông
        // báo "trả lời bình luận của bạn" (nhầm!) còn A không nhận gì.
        // Đúng: reply → thông báo cho TÁC GIẢ bình luận cha; comment gốc
        // → thông báo cho chủ game. Một query lấy đủ 3 giá trị cần thiết.
        let info_row: Option<(Uuid, String, Option<Uuid>)> = sqlx::query_as(
            "SELECT g.user_id, g.slug, pc.user_id \
             FROM games g \
             LEFT JOIN comments pc ON pc.id = $2 \
             WHERE g.id = $1",
        )
        .bind(game_id)
        .bind(parent_id)
        .fetch_optional(pool)
        .await?;

        if let Some((owner_id, game_slug, parent_author_id)) = info_row {
            let link = format!("/games/{game_slug}");
            if parent_id.is_some() {
                // Reply: thông báo tác giả bình luận cha (nếu không phải
                // chính người reply)
                if let Some(pa) = parent_author_id {
                    if pa != user_id {
                        let _ = sqlx::query(
                            r"INSERT INTO notifications (user_id, actor_id, type, title, link)
                              VALUES ($1, $2, 'reply'::notification_type, $3, $4)",
                        )
                        .bind(pa)
                        .bind(user_id)
                        .bind("Có người vừa trả lời bình luận của bạn")
                        .bind(&link)
                        .execute(pool)
                        .await;
                    }
                    // Nếu chủ game khác cả người reply LẪN tác giả cha →
                    // chủ game cũng nên biết có hoạt động mới (comment msg)
                    if owner_id != user_id && owner_id != pa {
                        let _ = sqlx::query(
                            r"INSERT INTO notifications (user_id, actor_id, type, title, link)
                              VALUES ($1, $2, 'comment'::notification_type, $3, $4)",
                        )
                        .bind(owner_id)
                        .bind(user_id)
                        .bind("Có người vừa bình luận game của bạn")
                        .bind(&link)
                        .execute(pool)
                        .await;
                    }
                }
            } else if owner_id != user_id {
                // Comment gốc: thông báo chủ game
                let _ = sqlx::query(
                    r"INSERT INTO notifications (user_id, actor_id, type, title, link)
                      VALUES ($1, $2, 'comment'::notification_type, $3, $4)",
                )
                .bind(owner_id)
                .bind(user_id)
                .bind("Có người vừa bình luận game của bạn")
                .bind(&link)
                .execute(pool)
                .await;
            }
        }

        Ok(id)
    }

    pub async fn list_by_game(
        pool: &PgPool,
        game_id: Uuid,
        viewer_id: Option<Uuid>,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<CommentWithUser>> {
        let comments = sqlx::query_as::<_, CommentWithUser>(
            r"SELECT c.id, c.game_id, c.user_id, c.parent_id, c.content,
                c.like_count, c.is_pinned, c.created_at, c.updated_at,
                u.display_name as user_name, u.avatar_url as user_avatar,
                EXISTS(
                  SELECT 1 FROM comment_likes cl WHERE cl.comment_id = c.id AND cl.user_id = $2
                ) as is_liked
              FROM comments c
              JOIN users u ON u.id = c.user_id
              WHERE c.game_id = $1 AND c.parent_id IS NULL
              ORDER BY c.is_pinned DESC, c.created_at DESC
              LIMIT $3 OFFSET $4",
        )
        .bind(game_id)
        .bind(viewer_id.unwrap_or_else(Uuid::nil))
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        // Ghi chú kiến trúc: replies KHÔNG được nạp sẵn ở đây — UI dùng
        // HTMX lazy-load từng nhánh qua list_replies khi người dùng mở
        // "Xem N trả lời". Điều này giữ truy vấn chính O(top-level) thay vì
        // O(top-level × replies) cho game có hàng nghìn bình luận.
        Ok(comments)
    }

    pub async fn list_replies(
        pool: &PgPool,
        parent_id: Uuid,
        viewer_id: Option<Uuid>,
    ) -> AppResult<Vec<CommentWithUser>> {
        let comments = sqlx::query_as::<_, CommentWithUser>(
            r"SELECT c.id, c.game_id, c.user_id, c.parent_id, c.content,
                c.like_count, c.is_pinned, c.created_at, c.updated_at,
                u.display_name as user_name, u.avatar_url as user_avatar,
                EXISTS(
                  SELECT 1 FROM comment_likes cl WHERE cl.comment_id = c.id AND cl.user_id = $2
                ) as is_liked
              FROM comments c
              JOIN users u ON u.id = c.user_id
              WHERE c.parent_id = $1
              ORDER BY c.created_at ASC",
        )
        .bind(parent_id)
        .bind(viewer_id.unwrap_or_else(Uuid::nil))
        .fetch_all(pool)
        .await?;
        Ok(comments)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<CommentWithUser>> {
        let c = sqlx::query_as::<_, CommentWithUser>(
            r"SELECT c.id, c.game_id, c.user_id, c.parent_id, c.content,
                c.like_count, c.is_pinned, c.created_at, c.updated_at,
                u.display_name as user_name, u.avatar_url as user_avatar,
                FALSE as is_liked
              FROM comments c
              JOIN users u ON u.id = c.user_id
              WHERE c.id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(c)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn delete(pool: &PgPool, id: Uuid) -> AppResult<()> {
        // games.comment_count được giảm bởi DB trigger (trigger_comment_delete)
        sqlx::query("DELETE FROM comments WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn toggle_pin(pool: &PgPool, id: Uuid) -> AppResult<bool> {
        let pinned: bool = sqlx::query_scalar(
            "UPDATE comments SET is_pinned = NOT is_pinned WHERE id = $1 RETURNING is_pinned",
        )
        .bind(id)
        .fetch_one(pool)
        .await?;
        Ok(pinned)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn toggle_like(pool: &PgPool, comment_id: Uuid, user_id: Uuid) -> AppResult<bool> {
        // DELETE-first trong transaction — cùng pattern InteractionRepo.
        // Mẫu cũ (SELECT → INSERT + UPDATE like_count rời rạc) có race
        // double-click: 2 request cùng thấy 'chưa like' → cả hai chạy
        // UPDATE like_count + 1 trong khi INSERT thứ 2 là no-op →
        // like_count = 2 dù chỉ 1 dòng comment_likes.
        let mut tx = pool.begin().await?;
        let deleted =
            sqlx::query("DELETE FROM comment_likes WHERE comment_id = $1 AND user_id = $2")
                .bind(comment_id)
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
        if deleted.rows_affected() > 0 {
            sqlx::query("UPDATE comments SET like_count = like_count - 1 WHERE id = $1")
                .bind(comment_id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            Ok(false)
        } else {
            sqlx::query(
                "INSERT INTO comment_likes (comment_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(comment_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
            sqlx::query("UPDATE comments SET like_count = like_count + 1 WHERE id = $1")
                .bind(comment_id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            Ok(true)
        }
    }

    /// Sửa bình luận (chỉ trong 5 phút đầu).
    /// Trả về lỗi `NotFound` nếu bình luận không tồn tại, không thuộc user, hoặc đã quá 5 phút.
    pub async fn update_content(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        content: &str,
    ) -> AppResult<CommentWithUser> {
        // Kiểm tra quyền và thời hạn trước khi update để phân biệt lỗi rõ ràng
        let existing: Option<(Uuid, Uuid, chrono::DateTime<chrono::Utc>)> =
            sqlx::query_as(r"SELECT id, user_id, created_at FROM comments WHERE id = $1")
                .bind(id)
                .fetch_optional(pool)
                .await?;

        let row = existing
            .ok_or_else(|| crate::error::AppError::NotFound("Bình luận không tồn tại".into()))?;
        if row.1 != user_id {
            return Err(crate::error::AppError::Forbidden(
                "Bạn không có quyền sửa bình luận này".into(),
            ));
        }
        if row.2 < chrono::Utc::now() - chrono::Duration::minutes(5) {
            return Err(crate::error::AppError::Forbidden(
                "Đã quá hạn 5 phút chỉnh sửa bình luận".into(),
            ));
        }

        sqlx::query(
            r"UPDATE comments SET content = $1, updated_at = NOW()
              WHERE id = $2 AND user_id = $3",
        )
        .bind(content)
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
        Self::find_by_id(pool, id)
            .await?
            .ok_or_else(|| crate::error::AppError::NotFound("Bình luận không tồn tại".into()))
    }

    /// Tìm user được @mention trong nội dung.
    /// Gom toàn bộ username rồi truy vấn MỘT lần với `= ANY($1)` —
    /// trước đây mỗi @mention là một round-trip DB riêng (N+1),
    /// comment @tag 10 người = 10 truy vấn.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find_mentions(
        pool: &PgPool,
        content: &str,
        exclude_user: Uuid,
    ) -> AppResult<Vec<Uuid>> {
        // Tách username: strip @ đầu, cắt ký tự dấu câu cuối (giữ chữ/số/_)
        let mut usernames: Vec<String> = Vec::new();
        for w in content.split_whitespace() {
            if let Some(username) = w.strip_prefix('@') {
                let username =
                    username.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_');
                if !username.is_empty() && !usernames.iter().any(|u| u == username) {
                    usernames.push(username.to_string());
                }
            }
        }
        if usernames.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT DISTINCT id FROM users WHERE username = ANY($1) AND id != $2 AND NOT is_banned",
        )
        .bind(&usernames)
        .bind(exclude_user)
        .fetch_all(pool)
        .await?;
        Ok(ids)
    }

    /// Danh sách bình luận mới nhất cho admin (phân trang)
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn list_recent(
        pool: &PgPool,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<CommentWithGame>> {
        let rows = sqlx::query_as::<_, CommentWithGame>(
            r"SELECT c.id, c.game_id, c.user_id, c.parent_id, c.content,
                c.like_count, c.is_pinned, c.created_at, c.updated_at,
                u.display_name as user_name, u.avatar_url as user_avatar,
                g.title as game_title, g.slug as game_slug
              FROM comments c
              JOIN users u ON u.id = c.user_id
              JOIN games g ON g.id = c.game_id
              ORDER BY c.created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Tổng số bình luận toàn site — phân trang admin comments.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count_all(pool: &PgPool) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM comments")
            .fetch_one(pool)
            .await?;
        Ok(c)
    }
}
