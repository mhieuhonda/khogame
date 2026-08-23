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
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(3000),
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
        })
    }
}
