//! v2.9.1 — GitHub REST API client dùng chung.
//!
//! Trước đây logic gọi `GET /repos/{owner}/{repo}` nằm riêng trong
//! `handlers/repos.rs` — chỉ đăng repo/làm mới thủ công mới cập nhật
//! metadata. Background job refresh số sao (janitor) cần CÙNG một lệnh
//! gọi nên tách ra service dùng chung, tránh 2 bản copy lệch nhau.
//!
//! Error model tách 2 lớp:
//! - [`GithubApiError`] — lỗi thấp cấp (network, HTTP status, JSON sai
//!   schema) kèm `status` + `retry_after` để từng caller tự quyết định:
//!   handler map sang 4xx/5xx thân thiện user, janitor log + skip.

use crate::models::repo::GithubApiRepo;

/// Lỗi gọi GitHub API — caller tự map sang response/log phù hợp.
#[derive(Debug, thiserror::Error)]
#[error("GitHub API error (status={status:?}): {message}")]
pub struct GithubApiError {
    /// HTTP status GitHub trả (`None` = không gọi được — network/DNS/timeout).
    pub status: Option<u16>,
    /// Giây GitHub yêu cầu chờ khi rate limit (header `Retry-After`).
    pub retry_after: Option<u64>,
    /// Thông điệp ngắn cho log / message user.
    pub message: String,
}

impl GithubApiError {
    /// Có phải đang bị GitHub rate limit (403/429) không — job nền dùng
    /// để DỪNG cả batch thay vì đốt tiếp quota.
    #[must_use]
    pub fn is_rate_limited(&self) -> bool {
        matches!(self.status, Some(403) | Some(429))
    }
}

/// Gọi `GET https://api.github.com/repos/{owner}/{repo}` lấy metadata.
///
/// # Errors
///
/// Trả về [`GithubApiError`] khi network fail, status != 200, hoặc JSON
/// không deserialize được (GitHub đổi schema).
pub async fn fetch_repo_meta(
    client: &reqwest::Client,
    token: Option<&String>,
    owner: &str,
    repo: &str,
) -> Result<GithubApiRepo, GithubApiError> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}");
    let mut req = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(token) = token {
        req = req.bearer_auth(token);
    }
    let resp = req.send().await.map_err(|e| GithubApiError {
        status: None,
        retry_after: None,
        message: format!("không kết nối được GitHub API: {e}"),
    })?;
    // Retry-After (giây) — GitHub trả khi rate limit secondary/429.
    let retry_after = resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok());
    let status = resp.status().as_u16();
    if status != 200 {
        return Err(GithubApiError {
            status: Some(status),
            retry_after,
            message: format!("GitHub API trả HTTP {status} cho {owner}/{repo}"),
        });
    }
    resp.json::<GithubApiRepo>()
        .await
        .map_err(|e| GithubApiError {
            status: Some(200),
            retry_after: None,
            message: format!("GitHub API trả JSON không deserialize được: {e}"),
        })
}

#[cfg(test)]
mod tests {
    use super::GithubApiError;

    #[test]
    fn test_rate_limited_detection() {
        let e = |s: Option<u16>| GithubApiError {
            status: s,
            retry_after: None,
            message: "x".into(),
        };
        assert!(e(Some(403)).is_rate_limited());
        assert!(e(Some(429)).is_rate_limited());
        assert!(!e(Some(404)).is_rate_limited());
        assert!(!e(Some(200)).is_rate_limited());
        assert!(!e(None).is_rate_limited());
    }
}
