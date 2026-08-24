use std::env;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub base_url: String,
    pub database_url: String,
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
    /// Có bật tính năng AI Agent không? Bật tự động khi ai_agent_secret được set.
    pub ai_agent_enabled: bool,
    /// Số ngày sống của phiên AI Agent (mặc định 90).
    pub ai_agent_session_ttl_days: i64,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let ai_agent_secret = env::var("AI_AGENT_SECRET")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_default();
        let ai_agent_enabled = !ai_agent_secret.is_empty();
        Ok(Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            base_url: env::var("BASE_URL")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "http://localhost:3000".into()),
            database_url: env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL is required"))?,
            session_key: env::var("SESSION_KEY")
                .map_err(|_| anyhow::anyhow!("SESSION_KEY is required"))?,
            google_client_id: env::var("GOOGLE_CLIENT_ID")
                .map_err(|_| anyhow::anyhow!("GOOGLE_CLIENT_ID is required"))?,
            google_client_secret: env::var("GOOGLE_CLIENT_SECRET")
                .map_err(|_| anyhow::anyhow!("GOOGLE_CLIENT_SECRET is required"))?,
            google_redirect_uri: env::var("GOOGLE_REDIRECT_URI")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "http://localhost:3000/auth/google/callback".into()),
            admin_email: env::var("ADMIN_EMAIL")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "khongdich.admin@gmail.com".into()),
            github_token: env::var("GITHUB_TOKEN").ok().filter(|s| !s.is_empty()),
            ai_agent_secret,
            ai_agent_enabled,
            ai_agent_session_ttl_days: env::var("AI_AGENT_SESSION_TTL_DAYS")
                .ok()
                .and_then(|d| d.parse().ok())
                .filter(|d| *d > 0)
                .unwrap_or(90),
        })
    }
}
