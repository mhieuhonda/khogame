use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[sqlx(type_name = "repo_status", rename_all = "lowercase")]
pub enum RepoStatus {
    Pending,
    Approved,
    Hidden,
}

impl Default for RepoStatus {
    fn default() -> Self {
        RepoStatus::Approved
    }
}

impl RepoStatus {
    pub fn label(&self) -> &'static str {
        match self {
            RepoStatus::Pending => "Chờ duyệt",
            RepoStatus::Approved => "Đã duyệt",
            RepoStatus::Hidden => "Đã ẩn",
        }
    }
    pub fn color(&self) -> &'static str {
        match self {
            RepoStatus::Pending => "#f59e0b",
            RepoStatus::Approved => "#10b981",
            RepoStatus::Hidden => "#ef4444",
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
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.repo_name)
    }
    pub fn html_url(&self) -> String {
        format!("https://github.com/{}/{}", self.owner, self.repo_name)
    }
    pub fn description_or(&self) -> String {
        self.description.clone().unwrap_or_default()
    }
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
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.repo_name)
    }
    pub fn html_url(&self) -> String {
        format!("https://github.com/{}/{}", self.owner, self.repo_name)
    }
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
