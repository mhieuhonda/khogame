use crate::error::AppResult;
use crate::models::report::{Report, ReportReason, ReportWithGame};
use sqlx::PgPool;
use uuid::Uuid;

pub struct ReportRepo;

impl ReportRepo {
    pub async fn create(
        pool: &PgPool,
        game_id: Uuid,
        reporter_id: Uuid,
        reason: &ReportReason,
        description: &str,
    ) -> AppResult<Uuid> {
        let id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO reports (game_id, reporter_id, reason, description)
              VALUES ($1, $2, $3, $4) RETURNING id"#,
        )
        .bind(game_id)
        .bind(reporter_id)
        .bind(reason)
        .bind(description)
        .fetch_one(pool)
        .await?;

        // Notify all admins/moderators
        let staff: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM users WHERE role IN ('admin', 'moderator')",
        )
        .fetch_all(pool)
        .await?;
        for sid in staff {
            let _ = sqlx::query(
                r#"INSERT INTO notifications (user_id, type, title, content, link)
                  VALUES ($1, 'report_status'::notification_type, $2, $3, $4)"#,
            )
            .bind(sid)
            .bind("Có báo cáo mới cần xử lý")
            .bind(format!("Lý do: {}", reason.label()))
            .bind(format!("/admin/reports"))
            .execute(pool)
            .await;
        }

        Ok(id)
    }

    pub async fn has_reported(
        pool: &PgPool,
        game_id: Uuid,
        reporter_id: Uuid,
    ) -> AppResult<bool> {
        let r: Option<i32> = sqlx::query_scalar(
            "SELECT 1 FROM reports WHERE game_id = $1 AND reporter_id = $2 AND status IN ('pending', 'reviewing')",
        )
        .bind(game_id)
        .bind(reporter_id)
        .fetch_optional(pool)
        .await?;
        Ok(r.is_some())
    }

    pub async fn list(
        pool: &PgPool,
        status_filter: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<ReportWithGame>> {
        let items = if let Some(status) = status_filter {
            sqlx::query_as::<_, ReportWithGame>(
                r#"SELECT r.id, r.game_id, g.title as game_title, g.slug as game_slug,
                    r.reporter_id, u.display_name as reporter_name,
                    r.reason, r.description, r.status, r.created_at, r.resolved_at
                  FROM reports r
                  JOIN games g ON g.id = r.game_id
                  JOIN users u ON u.id = r.reporter_id
                  WHERE r.status::text = $1
                  ORDER BY r.created_at DESC LIMIT $2 OFFSET $3"#,
            )
            .bind(status)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as::<_, ReportWithGame>(
                r#"SELECT r.id, r.game_id, g.title as game_title, g.slug as game_slug,
                    r.reporter_id, u.display_name as reporter_name,
                    r.reason, r.description, r.status, r.created_at, r.resolved_at
                  FROM reports r
                  JOIN games g ON g.id = r.game_id
                  JOIN users u ON u.id = r.reporter_id
                  ORDER BY r.created_at DESC LIMIT $1 OFFSET $2"#,
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
        };
        Ok(items)
    }

    pub async fn resolve(
        pool: &PgPool,
        id: Uuid,
        resolver_id: Uuid,
        status: &str,
        resolution: &str,
    ) -> AppResult<()> {
        sqlx::query(
            r#"UPDATE reports SET status = $1::report_status, resolution = $2,
              resolved_by = $3, resolved_at = NOW() WHERE id = $4"#,
        )
        .bind(status)
        .bind(resolution)
        .bind(resolver_id)
        .bind(id)
        .execute(pool)
        .await?;

        // Notify reporter
        let reporter_id: Uuid = sqlx::query_scalar(
            "SELECT reporter_id FROM reports WHERE id = $1",
        )
        .bind(id)
        .fetch_one(pool)
        .await?;
        let game_slug: String = sqlx::query_scalar(
            "SELECT g.slug FROM reports r JOIN games g ON g.id = r.game_id WHERE r.id = $1",
        )
        .bind(id)
        .fetch_one(pool)
        .await?;
        let _ = sqlx::query(
            r#"INSERT INTO notifications (user_id, type, title, content, link)
              VALUES ($1, 'report_status'::notification_type, $2, $3, $4)"#,
        )
        .bind(reporter_id)
        .bind("Báo cáo của bạn đã được xử lý")
        .bind(resolution)
        .bind(format!("/games/{}", game_slug))
        .execute(pool)
        .await;

        Ok(())
    }

    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<Report>> {
        let r = sqlx::query_as::<_, Report>(
            r#"SELECT id, game_id, reporter_id, reason, description, status,
              resolved_by, resolution, created_at, resolved_at
            FROM reports WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(r)
    }

    pub async fn count_pending(pool: &PgPool) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM reports WHERE status = 'pending'")
            .fetch_one(pool)
            .await?;
        Ok(c)
    }
}
