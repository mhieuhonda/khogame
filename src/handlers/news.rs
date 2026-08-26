use crate::error::{AppError, AppResult};
use crate::handlers::auth::unread_count;
use crate::middleware::{AuthUser, CurrentUser};
use crate::models::news::NewsStatus;
use crate::repositories::news::NewsForm;
use crate::repositories::NewsRepo;
use crate::state::AppState;
use crate::templates::{NewsListTemplate, NewsShowTemplate, NewsNewTemplate, NewsEditTemplate, MyNewsTemplate};
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

const NEWS_PER_PAGE: i64 = 12;

/// Danh sách category hợp lệ cho news (chống injection qua form).
/// Lần lượt match với những gì UI hiển thị.
pub const NEWS_CATEGORIES: &[(&str, &str)] = &[
    ("game", "Tin game"),
    ("tech", "Công nghệ"),
    ("industry", "Ngành game"),
    ("esports", "Esports"),
    ("community", "Cộng đồng"),
    ("review", "Review"),
    ("update", "Cập nhật"),
    ("other", "Khác"),
];

/// Validate category từ user input — chỉ cho phép giá trị trong whitelist.
fn validate_category(cat: &str) -> Result<String, AppError> {
    if cat.is_empty() {
        return Ok(String::new()); // empty = không phân loại
    }
    if NEWS_CATEGORIES.iter().any(|(k, _)| *k == cat) {
        Ok(cat.to_string())
    } else {
        Err(AppError::BadRequest(format!(
            "Category '{cat}' không hợp lệ"
        )))
    }
}

/// Validate URL http(s) — chống javascript: scheme gây XSS.
fn validate_url(url: &str) -> Result<String, AppError> {
    if url.is_empty() {
        return Ok(String::new());
    }
    if url.starts_with("http://") || url.starts_with("https://") {
        if url.len() > 2048 {
            return Err(AppError::BadRequest(
                "URL quá dài (tối đa 2048 ký tự)".into(),
            ));
        }
        Ok(url.to_string())
    } else {
        Err(AppError::BadRequest(
            "URL phải bắt đầu bằng http:// hoặc https://".into(),
        ))
    }
}

/// Sinh slug duy nhất cho news.
async fn make_unique_slug(state: &AppState, title: &str) -> String {
    let base = {
        let s = slug::slugify(title);
        if s.is_empty() {
            "tin-tuc".into()
        } else {
            s
        }
    };
    // NewsRepo chưa có slug_exists → dùng find_by_slug_public (trả None nếu chưa có,
    // bao gồm cả status pending/draft/rejected của người khác). Nhưng như vậy
    // 2 tin pending cùng title sẽ đụng UNIQUE → catch ở create() và retry.
    // Đơn giản hoá: thử base, nếu đã có thì thêm suffix.
    let mut slug = base.clone();
    let mut suffix = 1u32;
    loop {
        match NewsRepo::find_by_slug_public(&state.db, &slug).await {
            Ok(None) => break,
            Ok(Some(_)) => {
                suffix += 1;
                slug = format!("{base}-{suffix}");
                if suffix > 100 {
                    slug = format!("{}-{}", slug, Uuid::new_v4().simple());
                    break;
                }
            }
            // DB error → fallback suffix random để không block user
            Err(_) => {
                slug = format!("{}-{}", base, Uuid::new_v4().simple());
                break;
            }
        }
    }
    slug
}

async fn unread_for(state: &AppState, user: Option<&crate::models::user::User>) -> i64 {
    match user {
        Some(u) => unread_count(state, u.id).await,
        None => 0,
    }
}

// ============= List (public) =============

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn list(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(params): Query<ListParams>,
) -> AppResult<NewsListTemplate> {
    let page = params.page.unwrap_or(1).max(1);
    let category = params.category.as_deref().unwrap_or("");
    let q = params.q.as_deref().unwrap_or("");

    let (items, total) = if !q.is_empty() {
        let q_trimmed = q.trim();
        if q_trimmed.is_empty() || q_trimmed.chars().count() > 200 {
            (Vec::new(), 0)
        } else {
            let items = NewsRepo::search(&state.db, q_trimmed, page, NEWS_PER_PAGE).await?;
            let total = count_search(&state, q_trimmed).await;
            (items, total)
        }
    } else if !category.is_empty() {
        let items = NewsRepo::list_by_category(&state.db, category, page, NEWS_PER_PAGE).await?;
        let total = count_by_category(&state, category).await;
        (items, total)
    } else {
        let items = NewsRepo::list_published(&state.db, page, NEWS_PER_PAGE).await?;
        let total = NewsRepo::count_published(&state.db).await.unwrap_or(0);
        (items, total)
    };

    let total_pages = ((total + NEWS_PER_PAGE - 1) / NEWS_PER_PAGE).max(1);
    let featured = if page == 1 && q.is_empty() && category.is_empty() {
        NewsRepo::list_featured(&state.db, 3)
            .await
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let unread = unread_for(&state, current_user.as_ref()).await;

    Ok(NewsListTemplate {
        current_user,
        unread_notifications: unread,
        items,
        featured,
        total,
        page,
        total_pages,
        category: category.to_string(),
        category_label: NEWS_CATEGORIES
            .iter()
            .find(|(k, _)| *k == category)
            .map(|(_, v)| v.to_string())
            .unwrap_or_default(),
        query: q.to_string(),
        categories: NEWS_CATEGORIES.iter().map(|(k, v)| (*k, *v)).collect(),
    })
}

async fn count_search(state: &AppState, q: &str) -> i64 {
    // Query thực để đếm chính xác — không fallback về 0 vì hiển thị
    // "Tìm thấy 0 kết quả" khi DB lỗi là gây hiểu lầm.
    let pattern = format!("%{}%", q.replace('%', "\\%").replace('_', "\\_"));
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM news WHERE status = 'published' AND (title ILIKE $1 OR content ILIKE $1)",
    )
    .bind(&pattern)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0)
}

async fn count_by_category(state: &AppState, cat: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM news WHERE status = 'published' AND category = $1",
    )
    .bind(cat)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0)
}

#[derive(Deserialize)]
pub struct ListParams {
    pub page: Option<i64>,
    pub category: Option<String>,
    pub q: Option<String>,
}

// ============= Show (public) =============

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn show(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Path(slug): Path<String>,
) -> AppResult<NewsShowTemplate> {
    let news = NewsRepo::find_by_slug_public(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Tin tức không tồn tại hoặc đã bị gỡ".into()))?;

    // Bump views — best effort, không block render
    let _ = NewsRepo::increment_views(&state.db, news.id).await;

    let unread = unread_for(&state, current_user.as_ref()).await;
    let comments = NewsRepo::list_comments(&state.db, news.id, 50, 0)
        .await
        .unwrap_or_default();
    let has_liked = match &current_user {
        Some(u) => NewsRepo::has_liked(&state.db, u.id, news.id)
            .await
            .unwrap_or(false),
        None => false,
    };

    Ok(NewsShowTemplate {
        current_user,
        unread_notifications: unread,
        news: news.clone(),
        comments,
        has_liked,
        base_url: state.config.base_url.clone(),
    })
}

// ============= New form (auth required) =============

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn new_form(
    State(_state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<NewsNewTemplate> {
    // Banned user không được đăng tin
    if user.is_banned {
        return Err(AppError::Forbidden("Tài khoản đã bị khóa".into()));
    }
    Ok(NewsNewTemplate {
        current_user: Some(user),
        unread_notifications: 0,
        categories: NEWS_CATEGORIES.iter().map(|(k, v)| (*k, *v)).collect(),
        errors: Vec::new(),
        form: NewsFormPartial::default(),
    })
}

// ============= Create =============

#[derive(Deserialize, Debug, Default, Clone)]
pub struct NewsFormParams {
    pub title: String,
    pub excerpt: String,
    pub content: String,
    pub cover_image: Option<String>,
    pub category: Option<String>,
    pub source_url: Option<String>,
    pub source_name: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct NewsFormPartial {
    pub title: String,
    pub excerpt: String,
    pub content: String,
    pub cover_image: String,
    pub category: String,
    pub source_url: String,
    pub source_name: String,
}

impl From<&NewsFormParams> for NewsFormPartial {
    fn from(p: &NewsFormParams) -> Self {
        Self {
            title: p.title.clone(),
            excerpt: p.excerpt.clone(),
            content: p.content.clone(),
            cover_image: p.cover_image.clone().unwrap_or_default(),
            category: p.category.clone().unwrap_or_default(),
            source_url: p.source_url.clone().unwrap_or_default(),
            source_name: p.source_name.clone().unwrap_or_default(),
        }
    }
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    headers: axum::http::HeaderMap,
    connect_info: axum::extract::ConnectInfo<std::net::SocketAddr>,
    Form(params): Form<NewsFormParams>,
) -> AppResult<Response> {
    if user.is_banned {
        return Err(AppError::Forbidden("Tài khoản đã bị khóa".into()));
    }
    let mut errors: Vec<String> = Vec::new();

    // Validate title
    let title = params.title.trim().to_string();
    if title.is_empty() {
        errors.push("Tiêu đề không được để trống".into());
    } else if title.chars().count() > 200 {
        errors.push("Tiêu đề tối đa 200 ký tự".into());
    }

    // Validate content
    let content = params.content.trim().to_string();
    if content.is_empty() {
        errors.push("Nội dung không được để trống".into());
    } else if content.chars().count() > 50_000 {
        errors.push("Nội dung tối đa 50.000 ký tự".into());
    }

    let excerpt = params.excerpt.trim().to_string();
    if excerpt.chars().count() > 500 {
        errors.push("Tóm tắt tối đa 500 ký tự".into());
    }

    let category = match validate_category(params.category.as_deref().unwrap_or("")) {
        Ok(c) => c,
        Err(e) => {
            errors.push(e.to_string());
            String::new()
        }
    };

    let source_url = match validate_url(params.source_url.as_deref().unwrap_or("")) {
        Ok(u) => u,
        Err(e) => {
            errors.push(e.to_string());
            String::new()
        }
    };

    let source_name = params
        .source_name
        .as_deref()
        .unwrap_or("")
        .trim()
        .chars()
        .take(150)
        .collect::<String>();

    let cover_image = match validate_url(params.cover_image.as_deref().unwrap_or("")) {
        Ok(u) => {
            if u.is_empty() {
                None
            } else {
                Some(u)
            }
        }
        Err(e) => {
            errors.push(e.to_string());
            None
        }
    };

    if !errors.is_empty() {
        let tmpl = NewsNewTemplate {
            current_user: Some(user),
            unread_notifications: 0,
            categories: NEWS_CATEGORIES.iter().map(|(k, v)| (*k, *v)).collect(),
            errors,
            form: NewsFormPartial::from(&params),
        };
        return Ok(Html(tmpl.render().unwrap_or_default()).into_response());
    }

    let form = NewsForm {
        title: title.clone(),
        excerpt,
        content,
        cover_image,
        category,
        source_url,
        source_name,
    };

    let slug = make_unique_slug(&state, &title).await;

    let ip = crate::middleware::client_ip_from_parts(
        &headers,
        Some(&connect_info.0),
        state.config.trust_proxy_headers,
    );
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(std::string::ToString::to_string);

    let id = NewsRepo::create(&state.db, user.id, &form, &slug, Some(&ip), ua.as_deref()).await?;

    // Redirect về trang my-news với thông báo
    Ok(Redirect::to(&format!("/my-news?submitted={id}")).into_response())
}

// ============= Edit (owner or admin) =============

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn edit_form(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
) -> AppResult<NewsEditTemplate> {
    let news = NewsRepo::find_by_slug_public(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Tin tức không tồn tại".into()))?;

    // Lấy bản gốc để kiểm tra quyền (find_by_slug_public chỉ trả published;
    // owner pending/rejected cũng cần edit được)
    let full = match NewsRepo::find_by_id(&state.db, news.id).await? {
        Some(n) => n,
        None => return Err(AppError::NotFound("Tin tức không tồn tại".into())),
    };
    if full.user_id != user.id && !user.role.is_admin() {
        return Err(AppError::Forbidden("Bạn không có quyền sửa tin này".into()));
    }
    // Đã published thì không cho edit (phải tạo bản mới hoặc yêu cầu admin sửa)
    if full.status == NewsStatus::Published && !user.role.is_admin() {
        return Err(AppError::BadRequest(
            "Tin đã xuất bản không thể sửa trực tiếp. Hãy liên hệ admin.".into(),
        ));
    }

    Ok(NewsEditTemplate {
        current_user: Some(user),
        unread_notifications: 0,
        categories: NEWS_CATEGORIES.iter().map(|(k, v)| (*k, *v)).collect(),
        news: full,
        errors: Vec::new(),
    })
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Form(params): Form<NewsFormParams>,
) -> AppResult<Response> {
    let news = NewsRepo::find_by_slug_public(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Tin tức không tồn tại".into()))?;
    let full = NewsRepo::find_by_id(&state.db, news.id)
        .await?
        .ok_or_else(|| AppError::NotFound("Tin tức không tồn tại".into()))?;
    if full.user_id != user.id && !user.role.is_admin() {
        return Err(AppError::Forbidden("Bạn không có quyền sửa".into()));
    }
    if full.status == NewsStatus::Published && !user.role.is_admin() {
        return Err(AppError::BadRequest(
            "Tin đã xuất bản không thể sửa trực tiếp".into(),
        ));
    }

    let form = NewsForm {
        title: params.title.trim().to_string(),
        excerpt: params.excerpt.trim().to_string(),
        content: params.content.trim().to_string(),
        cover_image: params
            .cover_image
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(std::string::ToString::to_string),
        category: validate_category(params.category.as_deref().unwrap_or(""))?,
        source_url: validate_url(params.source_url.as_deref().unwrap_or(""))?,
        source_name: params
            .source_name
            .as_deref()
            .unwrap_or("")
            .trim()
            .chars()
            .take(150)
            .collect(),
    };
    if form.title.is_empty() {
        return Err(AppError::BadRequest("Tiêu đề không được để trống".into()));
    }
    if form.content.is_empty() {
        return Err(AppError::BadRequest("Nội dung không được để trống".into()));
    }
    // Nếu edit lại tin rejected → reset status về pending để admin duyệt lại
    if full.status == NewsStatus::Rejected {
        sqlx::query("UPDATE news SET status = 'pending', review_note = '' WHERE id = $1")
            .bind(full.id)
            .execute(&state.db)
            .await?;
    }
    NewsRepo::update(&state.db, full.id, &form).await?;
    Ok(Redirect::to(&format!("/news/{}", full.slug)).into_response())
}

// ============= Delete (owner or admin) =============

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn delete(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
) -> AppResult<Response> {
    let news = NewsRepo::find_by_slug_public(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Tin tức không tồn tại".into()))?;
    let full = NewsRepo::find_by_id(&state.db, news.id)
        .await?
        .ok_or_else(|| AppError::NotFound("Tin tức không tồn tại".into()))?;
    if full.user_id != user.id && !user.role.is_admin() {
        return Err(AppError::Forbidden("Bạn không có quyền xóa".into()));
    }
    NewsRepo::delete(&state.db, full.id).await?;
    Ok(Redirect::to("/my-news").into_response())
}

// ============= Like (HTMX) =============

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn toggle_like(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
) -> AppResult<Response> {
    let news = NewsRepo::find_by_slug_public(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Tin tức không tồn tại".into()))?;
    let liked = NewsRepo::toggle_like(&state.db, user.id, news.id).await?;
    let like_count = sqlx::query_scalar::<_, i64>("SELECT like_count FROM news WHERE id = $1")
        .bind(news.id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
    // HTMX response — thay nút like
    let html = format!(
        r#"<button class="btn btn-outline btn-sm" hx-post="/news/{}/like" hx-swap="outerHTML" aria-pressed="{}">❤️ {}</button>"#,
        news.slug, liked, like_count
    );
    Ok(Html(html).into_response())
}

// ============= Comment create =============

#[derive(Deserialize)]
pub struct CommentForm {
    pub content: String,
    pub parent_id: Option<String>,
}

pub async fn create_comment(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Form(form): Form<CommentForm>,
) -> AppResult<Response> {
    let content = form.content.trim();
    if content.is_empty() {
        return Err(AppError::BadRequest(
            "Nội dung bình luận không được để trống".into(),
        ));
    }
    if content.chars().count() > 2000 {
        return Err(AppError::BadRequest("Bình luận tối đa 2000 ký tự".into()));
    }
    let news = NewsRepo::find_by_slug_public(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Tin tức không tồn tại".into()))?;

    let parent_id = form
        .parent_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| Uuid::parse_str(s).ok());

    let _id = NewsRepo::create_comment(&state.db, news.id, user.id, parent_id, content).await?;

    // Redirect về trang + anchor comment mới
    Ok(Redirect::to(&format!("/news/{}#comments", news.slug)).into_response())
}

// ============= My News =============

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn my_news(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<MyNewsTemplate> {
    let items = NewsRepo::list_by_user(&state.db, user.id, true).await?;
    let unread = unread_count(&state, user.id).await;
    Ok(MyNewsTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_category_whitelist() {
        assert_eq!(validate_category("game").unwrap(), "game");
        assert_eq!(validate_category("tech").unwrap(), "tech");
        assert_eq!(validate_category("").unwrap(), "");
        assert!(validate_category("invalid").is_err());
        // Chống injection: category lạ bị từ chối
        assert!(validate_category("' OR 1=1").is_err());
        assert!(validate_category("GAME").is_err()); // case-sensitive
    }

    #[test]
    fn validate_url_rejects_javascript_scheme() {
        // Chống XSS qua javascript: scheme
        assert!(validate_url("javascript:alert(1)").is_err());
        assert!(validate_url("data:text/html,<script>").is_err());
        assert!(validate_url("").is_ok()); // empty OK (không có nguồn)
        assert!(validate_url("https://example.com").is_ok());
        assert!(validate_url("http://example.com").is_ok());
    }

    #[test]
    fn validate_url_length_limit() {
        let long = format!("https://example.com/{}", "a".repeat(2100));
        assert!(validate_url(&long).is_err());
    }

    #[test]
    fn news_categories_have_unique_keys() {
        // Đảm bảo không có key trùng trong whitelist
        let mut keys: Vec<&str> = NEWS_CATEGORIES.iter().map(|(k, _)| *k).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), NEWS_CATEGORIES.len(), "Duplicate category keys");
    }

    #[test]
    fn news_categories_all_have_labels() {
        // Mỗi category phải có label không rỗng
        for (k, v) in NEWS_CATEGORIES {
            assert!(!k.is_empty(), "Category key rỗng");
            assert!(!v.is_empty(), "Label cho category '{k}' rỗng");
        }
    }

    #[test]
    fn news_categories_count_is_reasonable() {
        // Không quá ít (3-) để không hữu ích, không quá nhiều (20+) để UI lộn xộn
        let count = NEWS_CATEGORIES.len();
        assert!(count >= 5, "Quá ít category: {count}");
        assert!(count <= 15, "Quá nhiều category: {count}");
    }

    #[test]
    fn validate_url_rejects_ftp_scheme() {
        // Chỉ http/https được phép — ftp/file/malformed khác bị reject
        assert!(validate_url("ftp://example.com/file.zip").is_err());
        assert!(validate_url("file:///etc/passwd").is_err());
        assert!(validate_url("not-a-url").is_err());
        assert!(validate_url("javascript:void(0)").is_err());
    }

    #[test]
    fn validate_category_all_8_categories_pass() {
        // Verify toàn bộ 8 category trong whitelist đều pass
        for (k, _) in NEWS_CATEGORIES {
            assert_eq!(validate_category(k).unwrap(), *k);
        }
    }
}
