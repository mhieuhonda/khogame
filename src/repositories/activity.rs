//! v3.0.0 — Repository heatmap hoạt động (GitHub-style) + lịch điểm danh.
//!
//! `user_activity_days`: đếm hoạt động per user per ngày VN. Bump bằng
//! upsert +1 từ services/retention (fire-and-forget). Heatmap profile đọc
//! 90 ngày gần nhất; janitor giữ tối đa 180 ngày (dọn phần cũ nếu cần).

use crate::error::AppResult;
use crate::models::retention::{CalendarDay, HeatmapDay};
use chrono::Datelike;
use sqlx::PgPool;
use uuid::Uuid;

/// Số ngày heatmap hiển thị trên profile.
pub const HEATMAP_DAYS: i64 = 91;

pub struct ActivityRepo;

impl ActivityRepo {
    /// Bump +1 hoạt động hôm nay cho user (upsert).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn bump_today(pool: &PgPool, user_id: Uuid) -> AppResult<()> {
        let sql = format!(
            r#"INSERT INTO user_activity_days (user_id, day, activity_count)
               VALUES ($1, {}, 1)
               ON CONFLICT (user_id, day)
               DO UPDATE SET activity_count = user_activity_days.activity_count + 1"#,
            crate::utils::SQL_TODAY_VN
        );
        sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Heatmap 90 ngày gần nhất (bỏ ngày không hoạt động — client fill).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn heatmap(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<HeatmapDay>> {
        let rows = sqlx::query_as::<_, HeatmapDay>(
            "SELECT day, activity_count FROM user_activity_days
             WHERE user_id = $1 AND day >= CURRENT_DATE - $2
             ORDER BY day ASC",
        )
        .bind(user_id)
        .bind(HEATMAP_DAYS)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Lịch điểm danh 1 tháng (dùng cho leaderboard + profile).
    /// Trả về các ngày của tháng hiện tại theo giờ VN kèm trạng thái.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn checkin_calendar_month(
        pool: &PgPool,
        user_id: Uuid,
    ) -> AppResult<Vec<CalendarDay>> {
        let today = crate::utils::today_vn();
        let first = format!("{}-01", today.format("%Y-%m"));
        let first_day = chrono::NaiveDate::parse_from_str(&first, "%Y-%m-%d")
            .map_err(|_| crate::error::AppError::BadRequest("Lỗi tính ngày đầu tháng".into()))?;
        let days_in_month = {
            let next_month = if first_day.month() == 12 {
                chrono::NaiveDate::from_ymd_opt(first_day.year() + 1, 1, 1)
            } else {
                chrono::NaiveDate::from_ymd_opt(first_day.year(), first_day.month() + 1, 1)
            };
            let nm = next_month
                .ok_or_else(|| crate::error::AppError::BadRequest("Lỗi tính tháng kế".into()))?;
            (nm.signed_duration_since(first_day)).num_days() as u32
        };
        let checked: Vec<chrono::NaiveDate> = sqlx::query_scalar(
            "SELECT checkin_date FROM daily_checkins
             WHERE user_id = $1 AND checkin_date >= $2 AND checkin_date < $2 + INTERVAL '1 month'",
        )
        .bind(user_id)
        .bind(first_day)
        .fetch_all(pool)
        .await?;
        let mut out = Vec::with_capacity(days_in_month as usize);
        for d in 1..=days_in_month {
            let date = first_day
                .checked_add_days(chrono::Days::new(u64::from(d) - 1))
                .unwrap_or(first_day);
            out.push(CalendarDay {
                day: d,
                checked_in: checked.contains(&date),
                is_today: date == today,
                is_future: date > today,
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heatmap_days_constant() {
        // 91 = 13 tuần — lưới heatmap chuẩn 13 cột
        assert_eq!(HEATMAP_DAYS, 91);
    }
}
