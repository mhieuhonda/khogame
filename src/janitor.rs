use crate::repositories::{NotificationRepo, RepoRepo, SessionRepo, StatsRepo};
use crate::state::AppState;
use chrono::Timelike;
use chrono::Datelike;
use std::time::Duration;

/// Số ngày giữ notification ĐÃ ĐỌC trước khi xoá. Notification chưa đọc
/// không bao giờ bị xoá để không mất thông báo người dùng chưa kịp xem.
const NOTIFICATION_RETENTION_DAYS: i64 = 90;

/// Số ngày giữ `daily_stats` (chart dashboard chỉ dùng 7 ngày gần nhất).
const DAILY_STATS_RETENTION_DAYS: i64 = 90;

/// v3.0.0 — Số ngày giữ dòng email_queue ở trạng thái kết thúc
/// (sent/failed/skipped). Trước đây KHÔNG có job xoá nào → bảng phình
/// vô hạn (mỗi notification tạo 1 row dù SMTP không chạy).
const EMAIL_QUEUE_RETENTION_DAYS: i64 = 30;

/// v3.0.0 — Weekly digest gửi sáng thứ 2 giờ VN (tức là duy nhất 1 ngày
/// trong tuần job này có tác dụng — các ngày khác sleep-through).
/// Thời điểm chạy trong ngày: 8:00 VN = 01:00 UTC.
const DIGEST_DAY_MONDAY: u32 = 1; // chrono weekday().number_from_monday() = 1
const DIGEST_HOUR_VN: u32 = 8;

/// v3.0.0 — Số ngày giữ nhật ký `xp_events` (90 ngày đủ cho activity
/// feed + leaderboard mùa/tháng + heatmap 90 ngày). Tổng XP không phụ
/// thuộc bảng này (đã cache ở user_xp_totals) nên xoá an toàn tuyệt đối.
const XP_EVENTS_RETENTION_DAYS: i64 = 90;

/// Chu kỳ chạy dọn dẹp mặc định (6 giờ). Có thể override qua env
/// `JANITOR_INTERVAL_SECS` (tối thiểu 60s để tránh spam DB khi test).
const DEFAULT_INTERVAL_SECS: u64 = 6 * 3600;

/// v2.2.0 — Chu kỳ gửi email queue (2 phút). Ngắn hơn janitor cleanup
/// để email realtime cho user.
const EMAIL_FLUSH_INTERVAL_SECS: u64 = 120;

/// Số email gửi mỗi batch — giới hạn để không quá tải SMTP trong 1 lần.
const EMAIL_BATCH_SIZE: i64 = 25;

/// FIX v2.8.1 — Email bị claim (status='sending') rồi process crash/redeploy
/// (compose stop_grace_period 30s → SIGKILL khi SMTP chậm) sẽ KHÔNG BAO GIỜ
/// được gửi lại vì claim_pending chỉ chọn status='pending'. Requeue những
/// row kẹt 'sending' quá 10 phút (batch gửi tối đa vài giây — 10 phút nghĩa
/// là chắc chắn process đã chết) về 'pending' để lần flush kế tiếp xử lý.
const EMAIL_STUCK_SENDING_SECS: i64 = 600;

/// v2.9.1 — Chu kỳ job nền refresh metadata repo GitHub (số sao/fork...).
/// Mặc định 3 giờ/lần. Override qua env `REPO_REFRESH_INTERVAL_SECS`
/// (tối thiểu 300s — job gọi GitHub API, chu kỳ ngắn hơn dễ dính rate limit).
const REPO_REFRESH_INTERVAL_SECS: u64 = 3 * 3600;

/// v2.9.1 — Repo được coi là "stale" (cần refresh) khi `updated_at` cũ hơn
/// giá trị này (giây). 1 giờ: số sao hiển thị lệch tối đa ~1h + thời gian
/// batch xử lý — chấp nhận được cho trang danh sách repo.
const REPO_STALE_AFTER_SECS: i64 = 3600;

/// v2.9.1 — Số repo refresh tối đa MỖI chu kỳ. 100 repo × ~1.5s delay
/// ≈ 2.5 phút chạy — với interval 3h, một ngày xử lý được ~800 lượt,
/// dư sức theo kịp hàng trăm repo. Không token: quota 60 req/h theo IP
/// — batch 100 sẽ bị cắt ở quanh mốc 60 lượt (dừng khi gặp 403/429,
/// chu kỳ sau tiếp tục từ repo stale cũ nhất — tự phục hồi, không kẹt).
const REPO_REFRESH_BATCH_SIZE: i64 = 100;

/// v2.9.1 — Nghỉ giữa 2 lệnh gọi GitHub API trong 1 batch (milliseconds).
/// Giảm burst: 1.5s giữa các call là lịch sự với GitHub và gần như loại
/// trừ secondary rate limit "abuse detection" (gọi liên tiếp <100ms).
const REPO_REFRESH_DELAY_MS: u64 = 1500;

/// v2.9.1 — Job nền REFRESH METADATA REPO GITHUB (số sao, fork, issues...).
///
/// LỖI CŨ (báo bởi chủ site): "repo GitHub không cập nhật số sao" — metadata
/// chỉ được cập nhật khi chủ repo tự bấm "Làm mới" hoặc đăng lại, không có
/// cơ chế nào tự động. `RepoRepo::refresh_all_stars` từng được viết sẵn
/// (v0.x) nhưng KHÔNG BAO GIỜ được gọi — dead code.
///
/// Cơ chế:
/// 1. Mỗi `REPO_REFRESH_INTERVAL_SECS` (mặc định 3h) chọn tối đa
///    `REPO_REFRESH_BATCH_SIZE` repo approved có `updated_at` cũ hơn
///    `REPO_STALE_AFTER_SECS` (stale nhất trước).
/// 2. Gọi `GET /repos/{owner}/{repo}` cho từng repo (có GITHUB_TOKEN nếu
///    cấu hình — 5000 req/h thay vì 60), nghỉ `REPO_REFRESH_DELAY_MS`
///    giữa các call.
/// 3. Cập nhật stars/forks/issues/description/homepage/language/pushed_at.
/// 4. Gặp rate limit (403/429) → DỪNG cả batch (log warn), chu kỳ sau
///    chạy tiếp từ repo stale cũ nhất. 404 (repo xoá/riêng tư) → bỏ qua,
///    giữ dữ liệu cũ (không xoá repo của user vì repo chuyển private).
pub async fn run_repo_star_refresh(state: AppState) {
    let interval = Duration::from_secs(
        std::env::var("REPO_REFRESH_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v >= 300)
            .unwrap_or(REPO_REFRESH_INTERVAL_SECS),
    );
    tracing::info!(
        "Repo star refresh job khởi động: chu kỳ {} giây, batch tối đa {}, stale sau {} giây",
        interval.as_secs(),
        REPO_REFRESH_BATCH_SIZE,
        REPO_STALE_AFTER_SECS
    );
    // Chạy ngay 1 lượt khi khởi động (repo stale nhất được làm mới sớm),
    // rồi lặp theo chu kỳ.
    loop {
        refresh_stale_repos(&state).await;
        tokio::time::sleep(interval).await;
    }
}

/// Chạy MỘT lượt refresh các repo stale. Tách khỏi loop để log rõ ràng.
/// Ghi log tổng kết số repo cập nhật thành công / bỏ qua.
async fn refresh_stale_repos(state: &AppState) {
    let repos = match RepoRepo::list_stale_approved(
        &state.db,
        REPO_STALE_AFTER_SECS,
        REPO_REFRESH_BATCH_SIZE,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Repo refresh: lỗi truy vấn danh sách repo stale: {e}");
            return;
        }
    };
    if repos.is_empty() {
        return;
    }
    tracing::info!("Repo refresh: làm mới metadata {} repo stale", repos.len());
    let start = std::time::Instant::now();
    let mut updated: u32 = 0;
    let mut skipped: u32 = 0;
    for (id, owner, name) in repos {
        // Nghỉ giữa các call — call đầu tiên không cần nghỉ.
        if updated + skipped > 0 {
            tokio::time::sleep(Duration::from_millis(REPO_REFRESH_DELAY_MS)).await;
        }
        match crate::services::github::fetch_repo_meta(
            &state.http_client,
            state.config.github_token.as_ref(),
            &owner,
            &name,
        )
        .await
        {
            Ok(meta) => {
                if let Err(e) = RepoRepo::update_meta(
                    &state.db,
                    id,
                    meta.description.as_deref().unwrap_or(""),
                    meta.homepage.as_deref().unwrap_or(""),
                    meta.language.as_deref().unwrap_or(""),
                    meta.stargazers_count.unwrap_or(0),
                    meta.forks_count.unwrap_or(0),
                    meta.open_issues_count.unwrap_or(0),
                    meta.pushed_at,
                )
                .await
                {
                    tracing::warn!("Repo refresh: lỗi UPDATE DB cho {owner}/{name}: {e}");
                    skipped += 1;
                } else {
                    updated += 1;
                }
            }
            Err(e) if e.is_rate_limited() => {
                // Hết quota — dừng cả batch, chu kỳ sau chạy tiếp (repo chưa
                // refresh vẫn stale nên sẽ được chọn lại trước tiên).
                tracing::warn!(
                    "Repo refresh: GitHub rate limit ({e}) tại {owner}/{name} — dừng batch, chu kỳ sau tiếp tục (retry_after={:?})",
                    e.retry_after
                );
                break;
            }
            Err(e) => {
                // 404 (repo xoá/chuyển private), 5xx GitHub, network...
                // → bỏ qua repo này, giữ dữ liệu cũ, tiếp tục repo kế tiếp.
                tracing::debug!("Repo refresh: bỏ qua {owner}/{name}: {e}");
                skipped += 1;
            }
        }
    }
    tracing::info!(
        "Repo refresh hoàn tất: {updated} cập nhật, {skipped} bỏ qua (mất {:?})",
        start.elapsed()
    );
}

/// Task nền dọn dẹp dữ liệu tạm — chạy suốt vòng đời server.
///
/// Trước đây session hết hạn chỉ được dọn opportunistic khi có người
/// đăng nhập (auth.rs), nên nếu traffic thấp thì bảng sessions phình to
/// vô hạn trên prod. Janitor này đảm bảo dọn định kỳ bất kể traffic:
/// - `sessions` hết hạn (`expires_at` < `NOW()`)
/// - `notifications` đã đọc cũ hơn 90 ngày
/// - `daily_stats` cũ hơn 90 ngày (chart chỉ dùng 7 ngày)
pub async fn run_janitor(state: AppState) {
    let interval = Duration::from_secs(
        std::env::var("JANITOR_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v >= 60)
            .unwrap_or(DEFAULT_INTERVAL_SECS),
    );
    tracing::info!(
        "Janitor khởi động: chu kỳ {} giây, giữ notification {} ngày, daily_stats {} ngày",
        interval.as_secs(),
        NOTIFICATION_RETENTION_DAYS,
        DAILY_STATS_RETENTION_DAYS
    );
    loop {
        // Đo thời gian dọn dẹp — nếu quá lâu (VD có 10M session hết hạn),
        // lần sau sẽ chạy ngay lập tức khi sleep hết, gây DB load cao
        // liên tục. Log duration để admin quan sát; vẫn giữ hành vi cũ
        // (sleep interval giữa các lần) vì dọn dẹp là idempotent.
        let start = std::time::Instant::now();
        let (sessions, notifications, daily_stats, emails, xp_events) = do_cleanup(&state).await;
        let elapsed = start.elapsed();
        if sessions > 0
            || notifications > 0
            || daily_stats > 0
            || emails > 0
            || xp_events > 0
        {
            tracing::info!(
                "Janitor: đã xoá {} session hết hạn, {} notification cũ, {} dòng daily_stats cũ, {} email_queue kết thúc, {} xp_events cũ (mất {:?})",
                sessions,
                notifications,
                daily_stats,
                emails,
                xp_events,
                elapsed
            );
        } else if elapsed > Duration::from_secs(30) {
            // Không xoá gì nhưng tốn >30s — có thể DB chậm hoặc query
            // full-scan thiếu index. Cảnh báo để admin check.
            tracing::warn!(
                "Janitor: dọn dẹp mất {:?} nhưng không xoá gì — có thể DB slow",
                elapsed
            );
        }
        tokio::time::sleep(interval).await;
    }
}

/// v2.2.0 — Email queue flusher. Chạy song song với janitor cleanup,
/// chu kỳ ngắn hơn (2 phút) để email đến user nhanh.
/// Đọc `email_queue` WHERE status='pending' AND next_retry_at <= NOW(),
/// gửi SMTP, đánh dấu 'sent' hoặc 'failed' (retry 3 lần).
/// v3.0.0 — Job nền WEEKLY DIGEST: sáng thứ 2 (08:00 giờ VN), gom game
/// mới + tin mới trong 7 ngày qua gửi email tổng hợp cho user opt-in
/// (user_notification_prefs.weekly_digest = TRUE). Gửi qua notification
/// type 'system' → trigger 017 tự enqueue email (tôn trọng nút
/// email_notifications tổng của user). Idempotent theo tuần: chỉ chạy
/// khi là thứ 2 và >= 8h VN; xử lý cả tuần hiện tại 1 lần duy nhất
/// (dedup qua notification content cùng tuần).
pub async fn run_weekly_digest(state: AppState) {
    // Chu kỳ check 30 phút — rẻ (1 query time check mỗi 30 phút)
    let interval = std::time::Duration::from_secs(1800);
    loop {
        tokio::time::sleep(interval).await;
        let now_vn = chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(7 * 3600).expect("TZ +7"));
        if now_vn.weekday().number_from_monday() != DIGEST_DAY_MONDAY || now_vn.hour() < DIGEST_HOUR_VN {
            continue;
        }
        if let Err(e) = send_weekly_digest(&state).await {
            tracing::warn!("Weekly digest fail: {e}");
        }
    }
}

/// Gửi digest 1 tuần (gọi từ run_weekly_digest).
/// # Errors
/// Trả lỗi khi DB fail.
async fn send_weekly_digest(state: &AppState) -> crate::error::AppResult<()> {
    // Game mới + tin mới 7 ngày qua
    let new_games: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM games WHERE status = 'published' AND published_at >= NOW() - INTERVAL '7 days'",
    )
    .fetch_one(&state.db)
    .await?;
    let new_news: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM news WHERE status = 'published' AND published_at >= NOW() - INTERVAL '7 days'",
    )
    .fetch_one(&state.db)
    .await?;
    if new_games == 0 && new_news == 0 {
        return Ok(()); // tuần trống — không gửi
    }
    let title = format!("📬 Tin tuần Louis Space: {new_games} game mới, {new_news} tin mới");
    // Chống gửi đôi trong cùng tuần: đếm notification cùng title tuần này
    let week_start = sqlx::query_scalar::<_, chrono::DateTime<chrono::Utc>>(
        "SELECT date_trunc('week', NOW() AT TIME ZONE 'Asia/Ho_Chi_Minh') AT TIME ZONE 'Asia/Ho_Chi_Minh'",
    )
    .fetch_one(&state.db)
    .await?;
    let already: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE type = 'system' AND title = $1 AND created_at >= $2",
    )
    .bind(&title)
    .bind(week_start)
    .fetch_one(&state.db)
    .await?;
    if already > 0 {
        return Ok(());
    }
    // Tạo notification 'system' cho mọi user opt-in digest + có email —
    // trigger 017 tự enqueue email (nếu email_notifications tổng vẫn bật)
    let res = sqlx::query(
        r"INSERT INTO notifications (user_id, type, title, content, link)
          SELECT u.id, 'system'::notification_type, $1,
                 'Bạn nhận email này vì đã bật Tổng hợp hằng tuần trong Tùy chọn thông báo. Tắt bất cứ lúc nào.',
                 '/news'
          FROM users u
          JOIN user_notification_prefs p ON p.user_id = u.id AND p.weekly_digest = TRUE
          WHERE u.is_banned = FALSE AND u.role <> 'ai_agent'
            AND u.email IS NOT NULL AND u.email <> ''",
    )
    .bind(&title)
    .execute(&state.db)
    .await?;
    tracing::info!(
        "Weekly digest: tạo {} notification/email cho user opt-in",
        res.rows_affected()
    );
    Ok(())
}

pub async fn run_email_flusher(state: AppState) {
    let interval = Duration::from_secs(
        std::env::var("EMAIL_FLUSH_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v >= 30)
            .unwrap_or(EMAIL_FLUSH_INTERVAL_SECS),
    );
    tracing::info!(
        "Email flusher khởi động: chu kỳ {} giây",
        interval.as_secs()
    );
    loop {
        // FIX v2.8.1: phục hồi email kẹt 'sending' (process chết giữa batch)
        // trước khi flush — xem comment EMAIL_STUCK_SENDING_SECS.
        match crate::services::email::requeue_stuck_sending(&state.db, EMAIL_STUCK_SENDING_SECS)
            .await
        {
            Ok(n) if n > 0 => {
                tracing::info!(
                    "Email flusher: requeue {} email kẹt trạng thái 'sending' sau crash",
                    n
                );
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!("Email flusher: lỗi requeue email kẹt 'sending': {}", e);
            }
        }
        match crate::services::email::flush_pending(&state.db, EMAIL_BATCH_SIZE).await {
            Ok((sent, failed, skipped)) => {
                if sent > 0 || failed > 0 {
                    tracing::info!(
                        "Email flusher: đã gửi {}, thất bại {}, skip {}",
                        sent,
                        failed,
                        skipped
                    );
                }
            }
            Err(e) => {
                tracing::warn!("Email flusher lỗi: {}", e);
            }
        }
        tokio::time::sleep(interval).await;
    }
}

/// Thực hiện một vòng dọn dẹp, trả về (sessions, notifications, `daily_stats`,
/// email_queue kết thúc, xp_events) đã xoá.
async fn do_cleanup(
    state: &AppState,
) -> (u64, u64, u64, u64, u64) {
    let sessions = SessionRepo::cleanup_expired(&state.db)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("Janitor: lỗi dọn sessions: {}", e);
            0
        });
    let notifications =
        NotificationRepo::cleanup_read_older_than(&state.db, NOTIFICATION_RETENTION_DAYS)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("Janitor: lỗi dọn notifications: {}", e);
                0
            });
    let daily_stats = StatsRepo::cleanup_old_daily_stats(&state.db, DAILY_STATS_RETENTION_DAYS)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("Janitor: lỗi dọn daily_stats: {}", e);
            0
        });
    // v3.0.0 — dọn email_queue ở trạng thái kết thúc (sent/failed/skipped):
    // trước đây không job nào xoá → bảng lớn dần vô hạn. Row 'pending'
    // KHÔNG BAO GIỜ bị xoá ở đây (email chưa gửi phải được giữ lại).
    let emails = cleanup_email_queue(&state.db, EMAIL_QUEUE_RETENTION_DAYS)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("Janitor: lỗi dọn email_queue: {}", e);
            0
        });
    // v3.0.0 — dọn xp_events quá 90 ngày: tổng XP sống ở user_xp_totals
    // (bảng cache tăng dần) nên xoá log cũ không ảnh hưởng số dư. Giữ 90
    // ngày đủ cho activity feed, leaderboard mùa/tháng và heatmap.
    let xp_events = cleanup_xp_events(&state.db, XP_EVENTS_RETENTION_DAYS)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("Janitor: lỗi dọn xp_events: {}", e);
            0
        });
    (sessions, notifications, daily_stats, emails, xp_events)
}

/// Xoá email_queue ở trạng thái kết thúc (sent/failed/skipped) cũ hơn
/// `days` ngày. `pending`/`sending` luôn được giữ lại.
/// # Errors
///
/// Trả lỗi khi DB fail.
async fn cleanup_email_queue(pool: &sqlx::PgPool, days: i64) -> crate::error::AppResult<u64> {
    let res = sqlx::query(
        "DELETE FROM email_queue
         WHERE status IN ('sent', 'failed', 'skipped')
           AND queued_at < NOW() - ($1 || ' days')::INTERVAL",
    )
    .bind(days.to_string())
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

/// Xoá nhật ký xp_events cũ hơn `days` ngày (an toàn — total XP nằm ở
/// user_xp_totals, không phụ thuộc bảng log).
/// # Errors
///
/// Trả lỗi khi DB fail.
async fn cleanup_xp_events(pool: &sqlx::PgPool, days: i64) -> crate::error::AppResult<u64> {
    let res = sqlx::query("DELETE FROM xp_events WHERE created_at < NOW() - ($1 || ' days')::INTERVAL")
        .bind(days.to_string())
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}

/// v2.2.0 — Test compile-time guards
#[cfg(test)]
mod tests {
    use super::{
        DAILY_STATS_RETENTION_DAYS, DEFAULT_INTERVAL_SECS, EMAIL_BATCH_SIZE,
        EMAIL_FLUSH_INTERVAL_SECS, EMAIL_QUEUE_RETENTION_DAYS, EMAIL_STUCK_SENDING_SECS,
        NOTIFICATION_RETENTION_DAYS, REPO_REFRESH_BATCH_SIZE, REPO_REFRESH_DELAY_MS,
        REPO_REFRESH_INTERVAL_SECS, REPO_STALE_AFTER_SECS, XP_EVENTS_RETENTION_DAYS,
    };

    /// Compile-time guards: nếu ai đổi hằng số janitor thành giá trị vô lý
    /// (retention âm, interval quá ngắn spam DB) thì build fail ngay.
    const _: () = {
        assert!(NOTIFICATION_RETENTION_DAYS > 0);
        assert!(NOTIFICATION_RETENTION_DAYS < 3650);
        assert!(DAILY_STATS_RETENTION_DAYS >= 7); // phải >= cửa sổ chart 7 ngày
        assert!(DEFAULT_INTERVAL_SECS >= 3600);
        assert!(EMAIL_FLUSH_INTERVAL_SECS >= 30);
        assert!(EMAIL_BATCH_SIZE > 0 && EMAIL_BATCH_SIZE <= 100);
        // v2.8.1 — email kẹt 'sending' phải requeue sau thời gian hợp lý
        // (vài phút batch → 10 phút là chắc chắn process đã chết).
        assert!(EMAIL_STUCK_SENDING_SECS >= 300);
        // v3.0.0 — guards retention mới:
        assert!(EMAIL_QUEUE_RETENTION_DAYS >= 7); // email log phải sống đủ lâu để audit
        assert!(XP_EVENTS_RETENTION_DAYS >= 30); // heatmap/season cần cửa sổ tối thiểu 30 ngày
        // v2.9.1 — guards cho job refresh repo:
        assert!(REPO_REFRESH_INTERVAL_SECS >= 300); // tránh spam GitHub API
        assert!(REPO_STALE_AFTER_SECS >= 300); // stale "hợp lý", không quét liên tục
        assert!(REPO_STALE_AFTER_SECS < REPO_REFRESH_INTERVAL_SECS as i64); // mỗi chu kỳ phải có repo đáng quét
        assert!(REPO_REFRESH_BATCH_SIZE > 0 && REPO_REFRESH_BATCH_SIZE <= 500);
        assert!(REPO_REFRESH_DELAY_MS >= 500); // lịch sự với GitHub API
    };
}
