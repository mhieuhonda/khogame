use std::env;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub base_url: String,
    pub database_url: String,
    // NOTE: SESSION_KEY bắt buộc ở startup. v3.9.0 — đã được DÙNG THẬT để
    // HMAC-sign cookie `ls_anon` (rate-limit identity, middleware.rs
    // anon_hmac từ v3.5.1) — đổi key giữa chừng làm mọi ls_anon cookie cũ
    // hết hiệu lực (user vào lại được, chỉ mất bucket cũ) nhưng KHÔNG mất
    // session đăng nhập. Vẫn là secret thật — không commit giá trị thật.
    pub session_key: String,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub google_redirect_uri: String,
    /// Email của quản trị viên tối cao (tự động lên admin khi đăng nhập).
    /// v3.4.2 FIX (audit "default superuser"): rỗng khi không set env —
    /// KHÔNG còn fallback cứng về Gmail cố định (fork/redeploy quên
    /// ADMIN_EMAIL = bất kỳ ai chiếm email đó cũng tự lên admin). Rỗng → bỏ qua
    /// auto-grant + error log một lần ở handler login.
    pub admin_email: String,
    /// GitHub token (optional) - tăng rate limit khi gọi GitHub API
    pub github_token: Option<String>,
    /// === AI Agent account system ===
    /// Secret dùng cho endpoint đăng ký AI Agent (POST /auth/ai/register).
    /// Chỉ admin biết secret này và chia sẻ out-of-band cho AI được phép.
    /// Bắt buộc phải có ở prod. Nếu rỗng → endpoint đăng ký bị vô hiệu
    /// (trả 503 Service Unavailable) để không ai vô tình để công khai.
    pub ai_agent_secret: String,
    /// Có bật tính năng AI Agent không? Bật tự động khi `ai_agent_secret` được set.
    pub ai_agent_enabled: bool,
    /// Số ngày sống của phiên AI Agent (mặc định 30).
    /// v3.4.2: giảm 90 → 30 — phiên web của AI không cần dài như trước
    /// (API token có TTL riêng), thu hẹp cửa sổ truy cập sau khi mật khẩu
    /// bị lộ/thu hồi (OWASP: credential revocation phải cắt được phiên).
    pub ai_agent_session_ttl_days: i64,
    /// Số ngày sống của API token `kgai_...` cấp khi /auth/ai/register
    /// (mặc định 365, tối đa 3650). v3.4.2: token không còn "sống mãi
    /// mãi" — audit: token lộ chỉ xoá được bằng SQL tay.
    pub ai_agent_token_ttl_days: i64,
    /// Quota upload (MB/ngày/user, mặc định 50). v3.4.2 — chống disk-fill
    /// DoS: trước đây không có quota, 4 endpoint upload ghi ~1.2GB/phút.
    pub upload_daily_quota_mb: i64,
    /// Có tin headers proxy (X-Forwarded-For / X-Real-IP — v3.9.0 KHÔNG còn
    /// tin CF-Connecting-IP: site không sau Cloudflare, header này client
    /// tự gắn được)
    /// khi xác định IP client không? Mặc định BẬT vì prod chạy sau
    /// Traefik/Coolify. Tắt khi expose trực tiếp internet — nếu không
    /// attacker tự set X-Forwarded-For để giả IP, chia bucket rate-limit
    /// riêng mỗi lần và lách giới hạn.
    pub trust_proxy_headers: bool,
    /// Số hop proxy TIN CẬY giữa client và app (mặc định 1).
    ///
    /// X-Forwarded-For có dạng `client, proxy1, proxy2...` — mỗi proxy append
    /// IP của hop trước đó vào CUỐI chuỗi. Real client IP nằm ở vị trí
    /// `số_phần_tử - hops` kể từ bên trái. Ví dụ:
    /// - 1 proxy (Traefik): `XFF = "client"` → lấy phần tử cuối (hops=1).
    /// - 2 proxy (Cloudflare → Traefik): `XFF = "client, cf_edge"` → lấy
    ///   phần tử KẾ TRƯỚC CUỐI (hops=2), vì phần tử cuối là IP edge của CF
    ///   (ai cũng giống nhau → y hệt bug "mọi user cùng 1 IP").
    ///
    /// Lấy nhầm phần tử cuối khi có ≥2 hop là nguyên nhân kinh điển của
    /// lỗi "toàn bộ user hiện cùng một IP" ở admin.
    pub trusted_proxy_hops: u8,
}

impl AppConfig {
    /// # Errors
    ///
    /// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
    pub fn from_env() -> anyhow::Result<Self> {
        let ai_agent_secret = env::var("AI_AGENT_SECRET")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_default();
        let ai_agent_enabled = !ai_agent_secret.is_empty();
        // Trim trailing slash trên BASE_URL để đồng nhất với `templates::init_base_url`
        // (trim_end_matches('/')) — trước đây config KHÔNG trim nên GOOGLE_REDIRECT_URI
        // check `strip_prefix(&base_url)` fail nếu BASE_URL có `/` cuối (vd `https://x.com/`
        // vs redirect `https://x.com/auth/...`). Cookie SameSite=None path cũng lợi từ
        // base_url nhất quán.
        let base_url = env::var("BASE_URL")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "http://localhost:3000".into())
            .trim_end_matches('/')
            .to_string();
        // Cảnh báo prod nếu BASE_URL là http://localhost — cookie sẽ
        // không có Secure, og:image trỏ về localhost, không nên chạy
        // production như vậy. Chỉ warn chứ không fail để dev/test vẫn OK.
        if base_url.starts_with("http://localhost")
            && env::var("RUST_ENV").ok().as_deref() == Some("prod")
        {
            tracing::warn!(
                "BASE_URL={base_url} trong RUST_ENV=prod — cookie sẽ KHÔNG có Secure, \
                 og:image trỏ localhost. Cài BASE_URL=https://domain.prod."
            );
        }
        let google_redirect_uri = env::var("GOOGLE_REDIRECT_URI")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "http://localhost:3000/auth/google/callback".into());
        // Bảo mật: GOOGLE_REDIRECT_URI phải nằm dưới BASE_URL. Nếu
        // attacker kiểm soát env và set redirect_uri = http://evil.com,
        // Google sẽ gửi OAuth code thẳng cho evil.com. Fail-fast ở startup
        // dễ hơn so với phát hiện sau khi user bị redirect.
        let redirect_path = google_redirect_uri
            .strip_prefix(&base_url)
            .filter(|p| p.starts_with('/'));
        if redirect_path.is_none()
            && base_url != "http://localhost:3000"
            && !google_redirect_uri.starts_with("http://localhost")
        {
            return Err(anyhow::anyhow!(
                "GOOGLE_REDIRECT_URI ({google_redirect_uri}) phải nằm dưới BASE_URL ({base_url}) \
                 — nếu không, OAuth code có thể bị gửi sang domain khác."
            ));
        }
        Ok(Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            // PORT: nếu set nhưng parse fail (vd `PORT=abc`), warn rồi fallback 3000
            // thay vì silent default — operator dễ nhận ra config sai.
            port: env::var("PORT")
                .ok()
                .map_or(3000, |raw| match raw.trim().parse::<u16>() {
                    Ok(p) => p,
                    Err(_) => {
                        tracing::warn!(
                            "PORT={raw:?} không hợp lệ (phải là u16 0-65535), fallback 3000"
                        );
                        3000
                    }
                }),
            base_url,
            database_url: env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL is required"))?,
            // SESSION_KEY là secret dùng để HMAC-sign cookie trong tương lai (hiện
            // chưa bật nhưng field bắt buộc để sẵn sàng). Validate length ≥32 bytes
            // ngay từ startup: RFC 2104 HMAC-SHA256 khuyến nghị key ≥ block size
            // (64 bytes) nhưng ≥32 bytes (256 bit) đã đủ entropy. Fail-fast
            // thay vì im lặng dùng key yếu (`"dev"`, `"secret"`) — operator
            // không_để_ý sẽ bị lộ nếu sau này bật HMAC.
            session_key: {
                let k = env::var("SESSION_KEY")
                    .map_err(|_| anyhow::anyhow!("SESSION_KEY is required"))?;
                if k.len() < 32 {
                    return Err(anyhow::anyhow!(
                        "SESSION_KEY phải có tối thiểu 32 bytes (hiện {} bytes) — \
                         tạo bằng `openssl rand -hex 32` hoặc `head -c 32 /dev/urandom | base64`. \
                         Key yếu (<32 bytes) không đủ entropy cho HMAC-SHA256.",
                        k.len()
                    ));
                }
                k
            },
            google_client_id: env::var("GOOGLE_CLIENT_ID")
                .map_err(|_| anyhow::anyhow!("GOOGLE_CLIENT_ID is required"))?,
            google_client_secret: env::var("GOOGLE_CLIENT_SECRET")
                .map_err(|_| anyhow::anyhow!("GOOGLE_CLIENT_SECRET is required"))?,
            google_redirect_uri,
            admin_email: {
                let e = env::var("ADMIN_EMAIL")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_default();
                if e.is_empty() {
                    // Log MỘT LẦN ở startup (thay vì mỗi lần login như bản
                    // đầu v3.4.2 — audit vòng 5: log noise).
                    tracing::error!(
                        "ADMIN_EMAIL chưa được set — TỪ CHỐI tự cấp quyền admin \
                         khi Google login. Set ADMIN_EMAIL để bootstrap quản trị viên."
                    );
                }
                e
            },
            github_token: env::var("GITHUB_TOKEN").ok().filter(|s| !s.is_empty()),
            ai_agent_secret,
            ai_agent_enabled,
            ai_agent_session_ttl_days: env::var("AI_AGENT_SESSION_TTL_DAYS")
                .ok()
                .and_then(|d| d.parse::<i64>().ok())
                .filter(|d| *d > 0)
                // Cắt trên 365 ngày — chống overflow INTERVAL Postgres khi
                // user set giá trị i64::MAX qua env, đồng thời tránh phiên
                // AI Agent sống quá lâu (vô hiệu hoá chính sách xoay vòng).
                .map(|d| d.min(365))
                .unwrap_or(30),
            ai_agent_token_ttl_days: env::var("AI_AGENT_TOKEN_TTL_DAYS")
                .ok()
                .and_then(|d| d.parse::<i64>().ok())
                .filter(|d| *d > 0)
                .map(|d| d.min(3650))
                .unwrap_or(365),
            upload_daily_quota_mb: env::var("UPLOAD_DAILY_QUOTA_MB")
                .ok()
                .and_then(|d| d.parse::<i64>().ok())
                .filter(|d| *d > 0 && *d <= 10_000)
                .unwrap_or(50),
            trust_proxy_headers: {
                let v = env::var("TRUST_PROXY_HEADERS").ok().is_none_or(|v| {
                    let v = v.trim().to_ascii_lowercase();
                    !(v == "0" || v == "false" || v == "no" || v == "off")
                });
                if v {
                    // Cảnh báo khi bật: nếu app expose trực tiếp internet
                    // (không có Traefik/Coolify/CDN), attacker có thể tự
                    // set X-Forwarded-For để giả IP, lách rate-limit bucket.
                    // Bật chỉ an toàn khi chạy sau reverse proxy kiểm soát được.
                    tracing::warn!(
                        "TRUST_PROXY_HEADERS=BẬT (mặc định). Nếu app expose trực tiếp \
                         internet (không có Traefik/CDN), set TRUST_PROXY_HEADERS=false \
                         để không bị giả IP qua X-Forwarded-For."
                    );
                }
                v
            },
            trusted_proxy_hops: {
                let h = env::var("TRUSTED_PROXY_HOPS")
                    .ok()
                    .and_then(|v| v.trim().parse::<u8>().ok())
                    .filter(|v| *v > 0 && *v <= 10)
                    .unwrap_or(1);
                if h > 1 {
                    tracing::info!(
                        "TRUSTED_PROXY_HOPS={h} — bỏ qua {h} hop proxy cuối khi đọc \
                         X-Forwarded-For (chuỗi proxy: client → ... → app)."
                    );
                }
                h
            },
        })
    }
}
