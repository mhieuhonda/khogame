use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[sqlx(type_name = "repo_status", rename_all = "lowercase")]
#[derive(Default)]
pub enum RepoStatus {
    Pending,
    #[default]
    Approved,
    Hidden,
}

impl RepoStatus {
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Pending => "Chờ duyệt",
            Self::Approved => "Đã duyệt",
            Self::Hidden => "Đã ẩn",
        }
    }
    #[must_use]
    pub const fn color(&self) -> &'static str {
        match self {
            Self::Pending => "#f59e0b",
            Self::Approved => "#10b981",
            Self::Hidden => "#ef4444",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GithubRepo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub game_id: Option<Uuid>,
    pub owner: String,
    pub repo_name: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub primary_language: Option<String>,
    pub stars: i32,
    pub forks: i32,
    pub open_issues: i32,
    pub pushed_at: Option<DateTime<Utc>>,
    pub status: RepoStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl GithubRepo {
    #[must_use]
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.repo_name)
    }
    #[must_use]
    pub fn html_url(&self) -> String {
        format!("https://github.com/{}/{}", self.owner, self.repo_name)
    }
    #[must_use]
    pub fn description_or(&self) -> String {
        self.description.clone().unwrap_or_default()
    }
    #[must_use]
    pub fn language_or(&self) -> String {
        self.primary_language.clone().unwrap_or_default()
    }
}

/// Repo kèm thông tin người đăng - cho danh sách
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GithubRepoCard {
    pub id: Uuid,
    pub owner: String,
    pub repo_name: String,
    pub description: Option<String>,
    pub primary_language: Option<String>,
    pub stars: i32,
    pub forks: i32,
    pub open_issues: i32,
    pub pushed_at: Option<DateTime<Utc>>,
    pub game_slug: Option<String>,
    pub game_title: Option<String>,
    pub author_name: String,
    pub author_username: String,
    pub author_avatar: Option<String>,
    pub status: RepoStatus,
    pub created_at: DateTime<Utc>,
}

impl GithubRepoCard {
    #[must_use]
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.repo_name)
    }
    #[must_use]
    pub fn html_url(&self) -> String {
        format!("https://github.com/{}/{}", self.owner, self.repo_name)
    }
    #[must_use]
    pub fn language_or(&self) -> String {
        self.primary_language.clone().unwrap_or_default()
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RepoForm {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub game_slug: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GithubApiRepo {
    pub full_name: Option<String>,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub language: Option<String>,
    pub stargazers_count: Option<i32>,
    pub forks_count: Option<i32>,
    pub open_issues_count: Option<i32>,
    pub pushed_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mọi trạng thái repo phải có label + màu cho badge admin UI.
    #[test]
    fn test_repo_status_labels_and_colors() {
        let all = [
            RepoStatus::Pending,
            RepoStatus::Approved,
            RepoStatus::Hidden,
        ];
        for s in &all {
            assert!(!s.label().is_empty());
            assert!(
                s.color().starts_with('#'),
                "color phải là hex css: {}",
                s.color()
            );
        }
        // Ba trạng thái phải có màu khác nhau để badge phân biệt được
        assert_ne!(RepoStatus::Pending.color(), RepoStatus::Approved.color());
        assert_ne!(RepoStatus::Approved.color(), RepoStatus::Hidden.color());
    }

    /// Default khi DB trả NULL/strange → Approved (repo hiện công khai theo
    /// chính sách auto-approve) — quan trọng để không làm mất repo hiện hữu.
    #[test]
    fn test_repo_status_default_is_approved() {
        assert_eq!(RepoStatus::default(), RepoStatus::Approved);
    }

    /// `full_name/html_url` dựng từ `owner/repo_name` — đúng format GitHub.
    #[test]
    fn test_github_repo_urls() {
        let r = GithubRepo {
            id: Uuid::nil(),
            user_id: Uuid::nil(),
            game_id: None,
            owner: "mhieuhonda".into(),
            repo_name: "khogame".into(),
            description: None,
            homepage: None,
            primary_language: Some("Rust".into()),
            stars: 42,
            forks: 7,
            open_issues: 3,
            pushed_at: None,
            status: RepoStatus::Approved,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert_eq!(r.full_name(), "mhieuhonda/khogame");
        assert_eq!(r.html_url(), "https://github.com/mhieuhonda/khogame");
        assert_eq!(r.description_or(), "");
        assert_eq!(r.language_or(), "Rust");
    }

    /// `RepoForm` với mọi field default (form rỗng) không panic serde.
    #[test]
    fn test_repo_form_default_deserialize() {
        let f: RepoForm = serde_json::from_str("{}").unwrap();
        assert_eq!(f.url, "");
        assert_eq!(f.description, "");
        assert!(f.game_slug.is_none());
    }
}
