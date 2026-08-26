use crate::error::AppResult;
use crate::models::settings::{AdminLog, AdminLogWithAdmin, DailyStatRow, Setting};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

pub struct SettingsRepo;

impl SettingsRepo {
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn get(pool: &PgPool, key: &str) -> AppResult<Option<String>> {
        let v: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = $1")
            .bind(key)
            .fetch_optional(pool)
            .await?;
        Ok(v)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn set(
        pool: &PgPool,
        key: &str,
        value: &str,
        updated_by: Option<Uuid>,
    ) -> AppResult<()> {
        sqlx::query(
            r"INSERT INTO settings (key, value, updated_by)
               VALUES ($1, $2, $3)
               ON CONFLICT (key) DO UPDATE SET value = $2, updated_by = $3, updated_at = NOW()",
        )
        .bind(key)
        .bind(value)
        .bind(updated_by)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn get_all(pool: &PgPool) -> AppResult<Vec<Setting>> {
        let rows = sqlx::query_as::<_, Setting>(
            "SELECT key, value, updated_at, updated_by FROM settings ORDER BY key",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn get_map(pool: &PgPool, keys: &[&str]) -> AppResult<HashMap<String, String>> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT key, value FROM settings WHERE key = ANY($1)")
                .bind(keys.iter().map(std::string::ToString::to_string).collect::<Vec<_>>())
                .fetch_all(pool)
                .await?;
        Ok(rows.into_iter().collect())
    }
}

pub struct AdminLogRepo;

impl AdminLogRepo {
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn log(
        pool: &PgPool,
        admin_id: Uuid,
        action: &str,
        target_type: &str,
        target_id: &str,
        detail: &str,
        ip: Option<&str>,
    ) -> AppResult<()> {
        sqlx::query(
            r"INSERT INTO admin_logs (admin_id, action, target_type, target_id, detail, ip)
               VALUES ($1,$2,$3,$4,$5,$6)",
        )
        .bind(admin_id)
        .bind(action)
        .bind(target_type)
        .bind(target_id)
        .bind(detail)
        .bind(ip)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn list(pool: &PgPool, limit: i64, offset: i64) -> AppResult<Vec<AdminLogWithAdmin>> {
        let rows = sqlx::query_as::<_, AdminLogWithAdmin>(
            r"SELECT l.id, u.display_name as admin_name, u.username as admin_username,
                l.action, l.target_type, l.target_id, l.detail, l.ip, l.created_at
              FROM admin_logs l JOIN users u ON u.id = l.admin_id
              ORDER BY l.created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Tổng số dòng audit log — phân trang trang /admin/audit.
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn count(pool: &PgPool) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admin_logs")
            .fetch_one(pool)
            .await?;
        Ok(c)
    }

    #[allow(dead_code)]
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn find(pool: &PgPool, id: Uuid) -> AppResult<Option<AdminLog>> {
        let row = sqlx::query_as::<_, AdminLog>(
            "SELECT id, admin_id, action, target_type, target_id, detail, ip, created_at FROM admin_logs WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }
}

pub struct StatsRepo;

impl StatsRepo {
    /// Thống kê tổng hợp 7 ngày gần nhất cho dashboard chart
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn daily_last_7_days(pool: &PgPool) -> AppResult<Vec<DailyStatRow>> {
        let rows = sqlx::query_as::<_, DailyStatRow>(
            r"WITH days AS (
                  SELECT generate_series(CURRENT_DATE - INTERVAL '6 days', CURRENT_DATE, '1 day')::date AS day
               )
               SELECT d.day,
                 COALESCE((SELECT SUM(views) FROM daily_stats ds WHERE ds.day = d.day), 0)::bigint AS views,
                 COALESCE((SELECT SUM(downloads) FROM daily_stats ds WHERE ds.day = d.day), 0)::bigint AS downloads,
                 COALESCE((SELECT COUNT(*) FROM games g WHERE g.created_at::date = d.day), 0)::bigint AS new_games,
                 COALESCE((SELECT COUNT(*) FROM users u WHERE u.created_at::date = d.day), 0)::bigint AS new_users
               FROM days d ORDER BY d.day",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn record_view(pool: &PgPool, game_id: Uuid) -> AppResult<()> {
        sqlx::query(
            r"INSERT INTO daily_stats (day, game_id, views) VALUES (CURRENT_DATE, $1, 1)
               ON CONFLICT (day, game_id) DO UPDATE SET views = daily_stats.views + 1",
        )
        .bind(game_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn record_download(pool: &PgPool, game_id: Uuid) -> AppResult<()> {
        sqlx::query(
            r"INSERT INTO daily_stats (day, game_id, downloads) VALUES (CURRENT_DATE, $1, 1)
               ON CONFLICT (day, game_id) DO UPDATE SET downloads = daily_stats.downloads + 1",
        )
        .bind(game_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Xoá `daily_stats` cũ hơn `days` ngày. Chart dashboard chỉ dùng 7 ngày
    /// gần nhất — giữ 90 ngày làm biên độ phân tích, phần còn lại là rác
    /// làm bảng phình to vô hạn (mỗi game × mỗi ngày 1 dòng).
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub async fn cleanup_old_daily_stats(pool: &PgPool, days: i64) -> AppResult<u64> {
        let res = sqlx::query(
            "DELETE FROM daily_stats WHERE day < CURRENT_DATE - ($1 || ' days')::INTERVAL",
        )
        .bind(days.to_string())
        .execute(pool)
        .await?;
        Ok(res.rows_affected())
    }
}
