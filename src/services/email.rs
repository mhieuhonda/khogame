//! Email transport — v2.2.0.
//!
//! Sử dụng [`lettre`] (SMTP client, rustls TLS) để gửi email từ `email_queue`.
//!
//! # Cấu hình
//!
//! Environment variables:
//! - `SMTP_HOST` — SMTP server (vd: `smtp.gmail.com`). Nếu trống → noop.
//! - `SMTP_PORT` — SMTP port (default: 587).
//! - `SMTP_USERNAME` — username để auth.
//! - `SMTP_PASSWORD` — password / app password.
//! - `SMTP_FROM` — From address (vd: `Louis Space <noreply@louis.space>`).
//! - `SMTP_TLS` — `starttls` (default) hoặc `implicit` (port 465) hoặc `none`.
//!
//! # Workflow
//!
//! 1. Trigger `trg_enqueue_email_on_notification` INSERT row vào `email_queue`
//!    mỗi khi có notification mới (nếu user bật `email_notifications`).
//! 2. `run_email_flusher` trong janitor gọi `flush_pending()` mỗi 2 phút.
//! 3. `flush_pending`:
//!    a. SELECT N row pending ORDER BY next_retry_at LIMIT batch_size
//!    b. Đánh dấu `status='sending'` (advisory lock tránh double-send).
//!    c. Compose body_html đầy đủ + body_text (text version).
//!    d. Gửi SMTP.
//!    e. UPDATE status='sent' OR 'failed' + last_error + attempts++.
//! 4. Nếu attempts >= 3 → status='failed' permanent, không retry.

use crate::error::{AppError, AppResult};
use sqlx::PgPool;
use uuid::Uuid;

/// 1 row trong `email_queue`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EmailQueueRow {
    pub id: Uuid,
    pub notification_id: Uuid,
    pub recipient: String,
    pub recipient_name: String,
    pub subject: String,
    pub body_html: String,
    pub body_text: String,
}

/// SMTP config load từ env 1 lần.
#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from: String,
    pub tls: SmtpTls,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtpTls {
    StartTls,
    Implicit,
    None,
}

impl SmtpConfig {
    /// Load từ env. Trả `None` nếu `SMTP_HOST` trống → email noop.
    /// Fail-fast (trả lỗi, không cho chạy tiếp) khi `SMTP_TLS=none` trong
    /// `RUST_ENV=prod` — plaintext SMTP trong prod làm lộ credentials +
    /// nội dung notification trên mạng. Phân biệt env theo đúng cách
    /// `config.rs` đang dùng (`RUST_ENV == "prod"`).
    /// # Errors
    ///
    /// Trả lỗi khi `SMTP_TLS=none` mà `RUST_ENV=prod`.
    fn from_env() -> AppResult<Option<Self>> {
        // Không dùng `ok()?` ở đây: hàm trả `Result` nên `?` trên Option
        // không compile — match tường minh để giữ nghĩa "thiếu là noop".
        let host_raw = match std::env::var("SMTP_HOST") {
            Ok(h) => h,
            Err(_) => return Ok(None),
        };
        let host = host_raw.trim().to_string();
        if host.is_empty() {
            return Ok(None);
        }
        let port: u16 = std::env::var("SMTP_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(587);
        let username = std::env::var("SMTP_USERNAME").ok().unwrap_or_default();
        let password = std::env::var("SMTP_PASSWORD").ok().unwrap_or_default();
        let from = std::env::var("SMTP_FROM")
            .ok()
            .unwrap_or_else(|| "Louis Space <noreply@louis.space>".to_string());
        let tls = match std::env::var("SMTP_TLS").ok().as_deref() {
            Some("implicit") => SmtpTls::Implicit,
            Some("none") => {
                // v3.8.0 FIX (security audit F16): SMTP plaintext trong prod
                // = credentials + nội dung notification đi qua mạng không
                // mã hoá. Fail-fast lúc khởi tạo khi RUST_ENV=prod (trả lỗi
                // để server KHÔNG chạy với cấu hình nguy hiểm), thay vì tắt
                // email im lặng như trước (operator tưởng "email tắt" nhưng
                // thật ra config sai nguy hiểm vẫn nằm đó). Chỉ cho phép ở
                // dev/test (SMTP local không TLS).
                if std::env::var("RUST_ENV").ok().as_deref() == Some("prod") {
                    return Err(AppError::Internal(anyhow::anyhow!(
                        "SMTP_TLS=none bị CẤM trong RUST_ENV=prod — credentials sẽ đi mạng không mã hoá. \
                         Dùng starttls (587) hoặc implicit (465)."
                    )));
                }
                SmtpTls::None
            }
            // Default là starttls (KHÔNG phải none): không cấu hình TLS
            // tường minh vẫn được mã hoá oportunistic.
            Some("starttls") | None | Some(_) => SmtpTls::StartTls,
        };
        Ok(Some(Self {
            host,
            port,
            username,
            password,
            from,
            tls,
        }))
    }
}

/// Lấy N email pending, đánh dấu `status='sending'` (transactional select-for-update).
/// Trả về `Vec<EmailQueueRow>` để caller gửi SMTP.
async fn claim_pending(pool: &PgPool, batch_size: i64) -> AppResult<Vec<EmailQueueRow>> {
    let rows = sqlx::query_as::<_, EmailQueueRow>(
        r"UPDATE email_queue SET status = 'sending', attempts = attempts + 1
          WHERE id IN (
            SELECT id FROM email_queue
            WHERE status = 'pending' AND next_retry_at <= NOW()
            ORDER BY next_retry_at ASC
            LIMIT $1
            FOR UPDATE SKIP LOCKED
          )
          RETURNING id, notification_id, recipient, recipient_name, subject, body_html, body_text",
    )
    .bind(batch_size)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Đánh dấu 1 email là đã gửi thành công.
async fn mark_sent(pool: &PgPool, id: Uuid) -> AppResult<()> {
    sqlx::query(
        "UPDATE email_queue SET status = 'sent', sent_at = NOW(), last_error = NULL WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Đánh dấu 1 email là thất bại. Nếu attempts >= max_attempts → permanent.
async fn mark_failed(pool: &PgPool, id: Uuid, error: &str, max_attempts: i32) -> AppResult<()> {
    // Exponential backoff: 1m, 5m, 25m (next_retry = NOW() * 5^n attempts)
    sqlx::query(
        r"UPDATE email_queue
          SET status = CASE WHEN attempts >= $2 THEN 'failed' ELSE 'pending' END,
              last_error = $3,
              next_retry_at = CASE WHEN attempts >= $2 THEN next_retry_at
                                   ELSE NOW() + (POWER(5, attempts) || ' minutes')::INTERVAL END
          WHERE id = $1",
    )
    .bind(id)
    .bind(max_attempts)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

/// FIX v2.8.1 — Phục hồi email kẹt trạng thái 'sending'.
///
/// `claim_pending` chuyển hàng loạt row sang 'sending' rồi mới gửi SMTP.
/// Nếu process crash / bị SIGKILL giữa batch (redeploy, OOM) thì các row
/// 'sending' mồ côi KHÔNG BAO GIỜ được đụng tới nữa (claim_pending chỉ
/// chọn 'pending') → email silently lost. Hàng này đưa row kẹt quá
/// `stuck_secs` về 'pending' (giữ nguyên attempts + backoff để không
/// spam vô hạn) để lần flush kế tiếp thử lại. Trả về số row đã requeue.
/// # Errors
///
/// Trả lỗi khi DB fail.
pub async fn requeue_stuck_sending(pool: &PgPool, stuck_secs: i64) -> AppResult<u64> {
    let res = sqlx::query(
        r"UPDATE email_queue
          SET status = 'pending', next_retry_at = NOW()
          WHERE status = 'sending'
            AND queued_at < NOW() - ($1 || ' seconds')::INTERVAL",
    )
    .bind(stuck_secs)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

/// Flush 1 batch email pending. Trả về (sent, failed, skipped).
/// Noop nếu SMTP_HOST chưa cấu hình.
/// # Errors
///
/// Trả lỗi khi DB fail (claim_pending/mark_*).
pub async fn flush_pending(pool: &PgPool, batch_size: i64) -> AppResult<(u64, u64, u64)> {
    // from_env fail-fast: SMTP_TLS=none trong prod trả lỗi → lan ra caller
    // (không noop im lặng để server chạy với config nguy hiểm).
    let smtp = match SmtpConfig::from_env()? {
        Some(s) => s,
        None => {
            // SMTP chưa cấu hình — mark pending thành 'skipped' để không
            // retry spam. v3.0.0 FIX: CHỈ mark các row cũ hơn 1 giờ và
            // chưa bị mark 'skipped' lần nào (last_error khác giá trị này)
            // — trước đây UPDATE toàn bộ pending mỗi 2 phút mỗi vòng
            // flusher = full-table write vô nghĩa, row vừa enqueue vòng
            // trước lại bị quét lại. Row mới chờ tới khi già 1h rồi mới
            // skip (1 lần duy nhất) → write volume = O(số row) thay vì
            // O(số vòng × số row).
            let skipped = sqlx::query(
                r"UPDATE email_queue SET status = 'skipped', last_error = 'SMTP not configured'
                  WHERE status = 'pending'
                    AND queued_at < NOW() - INTERVAL '1 hour'
                    AND last_error IS DISTINCT FROM 'SMTP not configured'
                  RETURNING 1",
            )
            .execute(pool)
            .await?
            .rows_affected();
            return Ok((0, 0, skipped));
        }
    };

    let rows = claim_pending(pool, batch_size).await?;
    if rows.is_empty() {
        return Ok((0, 0, 0));
    }

    let mut sent = 0u64;
    let mut failed = 0u64;
    for row in rows {
        let body_html = if row.body_html.is_empty() {
            compose_default_html(&row.subject, &row.body_text)
        } else {
            row.body_html.clone()
        };
        let body_text = if row.body_text.is_empty() {
            strip_html_to_text(&body_html)
        } else {
            row.body_text.clone()
        };

        match send_one(
            &smtp,
            &row.recipient,
            &row.recipient_name,
            &row.subject,
            &body_html,
            &body_text,
        )
        .await
        {
            Ok(()) => {
                mark_sent(pool, row.id).await?;
                sent += 1;
            }
            Err(e) => {
                tracing::warn!("Email gửi thất bại (id={}): {}", row.id, e);
                mark_failed(pool, row.id, &e.to_string(), 3).await?;
                failed += 1;
            }
        }
    }
    Ok((sent, failed, 0))
}

/// Sanitize display-name do user kiểm soát trước khi đưa vào `Mailbox`:
/// loại bỏ CR/LF + ký tự control (chống header injection qua tên), trim,
/// giới hạn 100 ký tự. Trả `None` khi tên rỗng sau sanitize (gửi không tên).
fn sanitize_display_name(name: &str) -> Option<String> {
    // is_control() đã bao gồm CR (\r), LF (\n), TAB và C1 controls.
    let cleaned: String = name.chars().filter(|c| !c.is_control()).collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Giới hạn 100 ký tự (đếm theo char, không cắt giữa codepoint).
    Some(trimmed.chars().take(100).collect())
}

/// Gửi 1 email SMTP.
#[cfg(feature = "email")]
async fn send_one(
    smtp: &SmtpConfig,
    to: &str,
    to_name: &str,
    subject: &str,
    body_html: &str,
    body_text: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use lettre::message::Mailbox;
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

    let from_mailbox: Mailbox = smtp
        .from
        .parse()
        .map_err(|e| format!("SMTP_FROM không hợp lệ: {e}"))?;
    // FIX (header injection qua display-name): KHÔNG ghép thô
    // `"{to_name} <{to_addr}>"` rồi parse chuỗi (to_name chứa CRLF/quote
    // có thể chèn header hoặc làm parse sai). Sanitize tên rồi dùng
    // `Mailbox::new` (tên và địa chỉ tách bạch, lettre tự quote/encode).
    // Áp dụng cho mọi Mailbox tạo từ input user trong file này.
    let to_addr: lettre::message::Address = to
        .trim()
        .parse()
        .map_err(|e| format!("Recipient email không hợp lệ ({to}): {e}"))?;
    let to_mailbox = Mailbox::new(sanitize_display_name(to_name), to_addr);

    let email = Message::builder()
        .from(from_mailbox)
        .to(to_mailbox)
        .subject(subject)
        .multipart(lettre::message::MultiPart::alternative_plain_html(
            body_text.to_string(),
            body_html.to_string(),
        ))?;

    let mut transport_builder = match smtp.tls {
        SmtpTls::StartTls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp.host)?,
        SmtpTls::Implicit => AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp.host)?,
        SmtpTls::None => {
            // Plain không TLS — chỉ dùng dev/test local
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&smtp.host).port(smtp.port)
        }
    };
    transport_builder = transport_builder.port(smtp.port);
    if !smtp.username.is_empty() {
        transport_builder = transport_builder.credentials(Credentials::new(
            smtp.username.clone(),
            smtp.password.clone(),
        ));
    }
    let transport = transport_builder.build();
    transport.send(email).await?;
    Ok(())
}

/// Fallback khi feature `email` tắt — không gửi được email, mark all failed.
#[cfg(not(feature = "email"))]
async fn send_one(
    _smtp: &SmtpConfig,
    _to: &str,
    _to_name: &str,
    _subject: &str,
    _body_html: &str,
    _body_text: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Err("Email feature disabled — rebuild với --features email".into())
}

/// Compose HTML wrapper cho email có body_html rỗng (vd trigger-based path).
fn compose_default_html(subject: &str, body_text: &str) -> String {
    // v3.4.2 (audit vòng 8 — latent hardening): escape HTML cho mọi giá trị
    // interpolate — hôm nay subject/body là chuỗi cố định, nhưng nếu sau này
    // notification title chứa nội dung user-created thì không thành vector
    // email-HTML-injection.
    let subject = escape_html(subject);
    let body_text = escape_html(body_text);
    format!(
        r#"<!DOCTYPE html>
<html lang="vi">
<head>
    <meta charset="utf-8">
    <title>{subject}</title>
</head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 600px; margin: 0 auto; padding: 20px; color: #1a1a1a;">
    <div style="background: #f8f9fa; padding: 20px; border-radius: 8px;">
        <h1 style="margin: 0 0 16px; font-size: 20px; color: #2563eb;">Louis Space</h1>
        <p style="margin: 0 0 12px; font-size: 16px; line-height: 1.6;">{body_text}</p>
        <hr style="border: none; border-top: 1px solid #e0e0e0; margin: 20px 0;">
        <p style="margin: 0; font-size: 12px; color: #888;">
            Bạn nhận được email này vì đã bật thông báo email trên Louis Space.<br>
            Vào <a href="https://louis.vangioitutien.com/profile/edit" style="color: #2563eb;">cài đặt hồ sơ</a> để tắt nếu không muốn nhận.
        </p>
    </div>
</body>
</html>"#
    )
}

/// Strip HTML → plain text (very basic, chỉ cho fallback).
fn strip_html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// v3.4.2 — escape HTML tối thiểu cho nội dung interpolate vào email HTML
/// (tránh email-HTML-injection nếu sau này subject/body chứa nội dung
/// user-created; hôm nay là chuỗi cố định — hardening phòng xa).
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html_to_text() {
        assert_eq!(
            strip_html_to_text("<p>Hello <strong>world</strong>!</p>"),
            "Hello world!"
        );
    }

    #[test]
    fn test_compose_default_html() {
        let html = compose_default_html("Test subject", "Hello body");
        assert!(html.contains("Louis Space"));
        assert!(html.contains("Test subject"));
        assert!(html.contains("Hello body"));
    }

    #[test]
    fn test_smtp_config_from_env_empty() {
        // Khi SMTP_HOST unset hoặc empty → Ok(None)
        std::env::remove_var("SMTP_HOST");
        assert!(SmtpConfig::from_env()
            .expect("Không được lỗi khi SMTP_HOST unset")
            .is_none());

        std::env::set_var("SMTP_HOST", "   ");
        assert!(SmtpConfig::from_env()
            .expect("Không được lỗi khi SMTP_HOST trống")
            .is_none());

        std::env::set_var("SMTP_HOST", "smtp.example.com");
        let cfg = SmtpConfig::from_env()
            .expect("Không được lỗi khi SMTP_HOST hợp lệ")
            .expect("Should parse");
        assert_eq!(cfg.host, "smtp.example.com");
        assert_eq!(cfg.port, 587); // default
        assert_eq!(cfg.tls, SmtpTls::StartTls); // default
    }

    #[test]
    fn test_smtp_tls_none_rejected_in_prod() {
        // Fail-fast: SMTP_TLS=none trong RUST_ENV=prod phải trả lỗi,
        // không được noop im lặng hay cho server chạy.
        std::env::set_var("SMTP_HOST", "smtp.example.com");
        std::env::set_var("SMTP_TLS", "none");
        std::env::set_var("RUST_ENV", "prod");
        assert!(SmtpConfig::from_env().is_err());
        // Ngoài prod vẫn cho phép (SMTP local dev).
        std::env::set_var("RUST_ENV", "dev");
        let cfg = SmtpConfig::from_env()
            .expect("dev không được lỗi")
            .expect("Should parse");
        assert_eq!(cfg.tls, SmtpTls::None);
        // Dọn env để không ảnh hưởng test khác.
        std::env::remove_var("SMTP_TLS");
        std::env::remove_var("RUST_ENV");
        std::env::remove_var("SMTP_HOST");
    }

    #[test]
    fn test_sanitize_display_name() {
        assert_eq!(sanitize_display_name(""), None);
        assert_eq!(sanitize_display_name("   "), None);
        // CR/LF + control bị loại (chống header injection).
        assert_eq!(
            sanitize_display_name("Nguyễn Văn A\r\nBcc: evil@x.com"),
            Some("Nguyễn Văn ABcc: evil@x.com".to_string())
        );
        assert_eq!(
            sanitize_display_name("A\u{0000}B\u{0007}C"),
            Some("ABC".to_string())
        );
        // Trim + giới hạn 100 ký tự.
        assert_eq!(
            sanitize_display_name("  An  "),
            Some("An".to_string())
        );
        let long = "a".repeat(150);
        assert_eq!(sanitize_display_name(&long).unwrap().chars().count(), 100);
    }
}
