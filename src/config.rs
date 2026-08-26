use std::env;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub base_url: String,
    pub database_url: String,
    // NOTE: SESSION_KEY vẫn được yêu cầu ở startup (bảo vệ khỏi quên
    // set khi deploy) nhưng chưa được dùng để HMAC-sign cookies. Khi
    // thêm HMAC signing, dùng field này — KHÔNG xoá trước khi đó.
    #[allow(dead_code)]
    pub session_key: String,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub google_redirect_uri: String,
    /// Email của quản trị viên tối cao (tự động lên admin khi đăng nhập)
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
    /// Số ngày sống của phiên AI Agent (mặc định 90).
    pub ai_agent_session_ttl_days: i64,
    /// Có tin headers proxy (X-Forwarded-For / X-Real-IP / CF-Connecting-IP)
    /// khi xác định IP client không? Mặc định BẬT vì prod chạy sau
    /// Traefik/Coolify. Tắt khi expose trực tiếp internet — nếu không
    /// attacker tự set X-Forwarded-For để giả IP, chia bucket rate-limit
    /// riêng mỗi lần và lách giới hạn.
    pub trust_proxy_headers: bool,
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
        let base_url = env::var("BASE_URL")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "http://localhost:3000".into());
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
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            base_url,
            database_url: env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL is required"))?,
            session_key: env::var("SESSION_KEY")
                .map_err(|_| anyhow::anyhow!("SESSION_KEY is required"))?,
            google_client_id: env::var("GOOGLE_CLIENT_ID")
                .map_err(|_| anyhow::anyhow!("GOOGLE_CLIENT_ID is required"))?,
            google_client_secret: env::var("GOOGLE_CLIENT_SECRET")
                .map_err(|_| anyhow::anyhow!("GOOGLE_CLIENT_SECRET is required"))?,
            google_redirect_uri,
            admin_email: env::var("ADMIN_EMAIL")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "khongdich.admin@gmail.com".into()),
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
                .unwrap_or(90),
            trust_proxy_headers: {
                let v = env::var("TRUST_PROXY_HEADERS")
                    .ok()
                    .is_none_or(|v| {
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
        })
    }
}
