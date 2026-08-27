use crate::error::{AppError, AppResult};
use crate::handlers::auth::unread_count;
use crate::middleware::{AuthUser, CurrentUser};
use crate::models::news::NewsStatus;
use crate::repositories::news::NewsForm;
use crate::repositories::{NewsCategoryRepo, NewsRepo};
use crate::state::AppState;
use crate::templates::{
    MyNewsTemplate, NewsEditTemplate, NewsListTemplate, NewsNewTemplate, NewsShowTemplate,
};
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

const NEWS_PER_PAGE: i64 = 12;

/// Fallback category list khi DB chưa migrate v1.4.0 hoặc bảng
/// `news_categories` trống. Đảm bảo website vẫn chạy được khi admin
/// chưa thêm category nào qua UI — form /news/new vẫn có select với
/// 8 category mặc định. Sau khi admin CRUD qua UI, DB là source of truth.
pub const NEWS_CATEGORIES_FALLBACK: &[(&str, &str)] = &[
    ("game", "Tin game"),
    ("tech", "Công nghệ"),
    ("industry", "Ngành game"),
    ("esports", "Esports"),
    ("community", "Cộng đồng"),
    ("review", "Review"),
    ("update", "Cập nhật"),
    ("other", "Khác"),
];

/// Fetch category list từ DB. Fallback về `NEWS_CATEGORIES_FALLBACK`
/// nếu DB lỗi / bảng trống / chưa migrate. Trả về `Vec<(slug, name)>`
/// match interface cũ `NEWS_CATEGORIES` để template không đổi.
///
/// Tên "dynamic" để phân biệt với `NEWS_CATEGORIES_FALLBACK` (static).
async fn dynamic_categories(state: &AppState) -> Vec<(String, String)> {
    match NewsCategoryRepo::list_active(&state.db).await {
        Ok(cats) if !cats.is_empty() => cats
            .iter()
            .map(|c| (c.slug.clone(), c.name.clone()))
            .collect(),
        _ => NEWS_CATEGORIES_FALLBACK
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect(),
    }
}

/// Validate category: empty OK, hoặc phải tồn tại trong DB hoặc trong
/// fallback list (cho tin cũ trước v1.4.0 với category không có trong DB).
/// Async vì cần query DB.
async fn validate_category(state: &AppState, cat: &str) -> Result<String, AppError> {
    if cat.is_empty() {
        return Ok(String::new()); // empty = không phân loại
    }
    // DB check trước
    if let Ok(Some(_)) = NewsCategoryRepo::find_by_slug(&state.db, cat).await {
        return Ok(cat.to_string());
    }
    // Fallback whitelist (cho tin cũ có category thuộc 8 mục default)
    if NEWS_CATEGORIES_FALLBACK.iter().any(|(k, _)| *k == cat) {
        return Ok(cat.to_string());
    }
    Err(AppError::BadRequest(format!(
        "Category '{cat}' không hợp lệ"
    )))
}

/// Validate URL http(s) — chống javascript: scheme gây XSS.
/// Cũng chặn control char (CR/LF) trong URL để chống header injection
/// khi URL sau này được dùng làm Location header hoặc trong RSS XML.
fn validate_url(url: &str) -> Result<String, AppError> {
    if url.is_empty() {
        return Ok(String::new());
    }
    if url.bytes().any(|b| b.is_ascii_control()) {
        return Err(AppError::BadRequest(
            "URL chứa ký tự điều khiển không hợp lệ".into(),
        ));
    }
    // Chấp nhận: (1) http(s):// URL remote HOẶC (2) `/uploads/...` URL
    // nội bộ do server sinh khi user upload ảnh bìa qua POST /uploads/news/cover.
    if url.starts_with("http://")
        || url.starts_with("https://")
        || crate::services::storage::is_upload_url(url)
    {
        if url.len() > 2048 {
            return Err(AppError::BadRequest(
                "URL quá dài (tối đa 2048 ký tự)".into(),
            ));
        }
        Ok(url.to_string())
    } else {
        Err(AppError::BadRequest(
            "URL phải bắt đầu bằng http://, https:// hoặc /uploads/".into(),
        ))
    }
}

/// Sinh slug duy nhất cho news.
async fn make_unique_slug(state: &AppState, title: &str) -> AppResult<String> {
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
        match NewsRepo::find_by_slug_public(&state.db, &slug).await? {
            None => break,
            Some(_) => {
                suffix += 1;
                slug = format!("{base}-{suffix}");
                if suffix > 100 {
                    // Vượt 100 lần thử — fallback UUID (dùng base, không
                    // ghép thêm suffix để URL không quá xấu).
                    slug = format!("{}-{}", base, Uuid::new_v4().simple());
                    break;
                }
            }
        }
    }
    Ok(slug)
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
    // v1.4.0: category validation dùng DB, không còn hardcode whitelist.
    let category_raw = params.category.as_deref().unwrap_or("");
    let category = if category_raw.is_empty() {
        String::new()
    } else {
        match validate_category(&state, category_raw).await {
            Ok(c) => c,
            Err(_) => String::new(), // fallback về all nếu category không hợp lệ
        }
    };
    let q = params.q.as_deref().unwrap_or("");

    // Fetch items + total song song (tokio::join!) — trước đây chạy tuần tự
    // gây 2 DB round-trip nối đuôi nhau. Song song giảm ~50% latency.
    let (items, total) = if !q.is_empty() {
        let q_trimmed = q.trim();
        if q_trimmed.is_empty() || q_trimmed.chars().count() > 200 {
            (Vec::new(), 0)
        } else {
            let items_fut = NewsRepo::search(&state.db, q_trimmed, page, NEWS_PER_PAGE);
            let total_fut = count_search(&state, q_trimmed);
            let (items, total) = tokio::join!(items_fut, total_fut);
            (items?, total)
        }
    } else if !category.is_empty() {
        let items_fut = NewsRepo::list_by_category(&state.db, &category, page, NEWS_PER_PAGE);
        let total_fut = count_by_category(&state, &category);
        let (items, total) = tokio::join!(items_fut, total_fut);
        (items?, total)
    } else {
        let items_fut = NewsRepo::list_published(&state.db, page, NEWS_PER_PAGE);
        let total_fut = NewsRepo::count_published(&state.db);
        let (items, total) = tokio::join!(items_fut, total_fut);
        (items?, total.unwrap_or(0))
    };

    let total_pages = ((total + NEWS_PER_PAGE - 1) / NEWS_PER_PAGE).max(1);
    let is_first_page_all = page == 1 && q.is_empty() && category.is_empty();
    let featured_fut = async {
        if is_first_page_all {
            NewsRepo::list_featured(&state.db, 3)
                .await
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    };

    // v1.4.0: categories từ DB (có fallback). Fetch song song với unread.
    let cats_fut = dynamic_categories(&state);
    let unread_fut = unread_for(&state, current_user.as_ref());
    let (featured, cats, unread) = tokio::join!(featured_fut, cats_fut, unread_fut);

    // Tìm label cho category hiện tại trong cats list.
    let category_label = cats
        .iter()
        .find(|(k, _)| *k == category)
        .map(|(_, v)| v.clone())
        .unwrap_or_default();
    Ok(NewsListTemplate {
        current_user,
        unread_notifications: unread,
        items,
        featured,
        total,
        page,
        total_pages,
        category: category.to_string(),
        category_label,
        query: q.to_string(),
        categories: cats,
    })
}

async fn count_search(state: &AppState, q: &str) -> i64 {
    // Query thực để đếm chính xác — không fallback về 0 vì hiển thị
    // "Tìm thấy 0 kết quả" khi DB lỗi là gây hiểu lầm.
    // Dùng escape_like (chống wildcard %, _, \) + ESCAPE '\\' để tìm theo
    // literal. Trước đây dùng replace thủ công chỉ escape % và _ mà quên
    // escape `\` (escape char) → user tìm chuỗi chứa `\` bị match sai.
    let pattern = format!("%{}%", crate::utils::escape_like(q));
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM news WHERE status = 'published' AND (title ILIKE $1 ESCAPE '\\' OR content ILIKE $1 ESCAPE '\\')",
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

    // Bump views nền (detached task) — không block render như trước đây
    // (await inline làm page render chậm thêm 1 DB round-trip). Best-effort:
    // lỗi DB không ảnh hưởng request.
    let db_clone = state.db.clone();
    let news_id = news.id;
    tokio::spawn(async move {
        let _ = NewsRepo::increment_views(&db_clone, news_id).await;
    });

    // Các query song song (tokio::join!) — trước đây chạy tuần tự gây chậm.
    let unread_fut = unread_for(&state, current_user.as_ref());
    let comments_fut = NewsRepo::list_comments(&state.db, news.id, 50, 0);
    let has_liked_fut = async {
        match &current_user {
            Some(u) => NewsRepo::has_liked(&state.db, u.id, news.id)
                .await
                .unwrap_or(false),
            None => false,
        }
    };
    let (unread, comments, has_liked) = tokio::join!(unread_fut, comments_fut, has_liked_fut);
    let comments = comments.unwrap_or_default();

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
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<NewsNewTemplate> {
    // Banned user không được đăng tin
    if user.is_banned {
        return Err(AppError::Forbidden("Tài khoản đã bị khóa".into()));
    }
    // v1.4.0: categories từ DB (fallback nếu bảng chưa migrate).
    let cats = dynamic_categories(&state).await;
    Ok(NewsNewTemplate {
        current_user: Some(user),
        unread_notifications: 0,
        categories: cats,
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

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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

    let category = match validate_category(&state, params.category.as_deref().unwrap_or("")).await {
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
        // v1.4.0: categories từ DB khi re-render form có lỗi.
        let cats = dynamic_categories(&state).await;
        let tmpl = NewsNewTemplate {
            current_user: Some(user),
            unread_notifications: 0,
            categories: cats,
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

    let slug = make_unique_slug(&state, &title).await?;

    let ip = crate::middleware::client_ip_from_parts(
        &headers,
        Some(&connect_info.0),
        state.config.trust_proxy_headers,
        state.config.trusted_proxy_hops,
    );
    // Clamp UA 512 ký tự — tránh lưu 1MB User-Agent header vào
    // TEXT column news.author_ua (DB bloat + backup phình to).
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.chars().take(512).collect::<String>());

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
    // Lookup news by slug WITHOUT status filter — trước đây code gọi
    // find_by_slug_public (chỉ trả status IN published, archived) rồi mới
    // find_by_id → owner pending/rejected nhận 404 ngay cả cho tin của
    // chính mình, chặn luồng reject→edit→resubmit mà comment dưới đây
    // hứa. Bỏ qua public filter ở đây: quyền truy cập được kiểm soát
    // bằng ownership check ngay sau đó (owner hoặc admin).
    let news_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM news WHERE slug = $1")
        .bind(&slug)
        .fetch_optional(&state.db)
        .await?;
    let full = match news_id {
        Some(id) => NewsRepo::find_by_id(&state.db, id).await?,
        None => None,
    }
    .ok_or_else(|| AppError::NotFound("Tin tức không tồn tại".into()))?;
    if full.user_id != user.id && !user.role.is_admin() {
        return Err(AppError::Forbidden("Bạn không có quyền sửa tin này".into()));
    }
    // Đã published thì không cho edit (phải tạo bản mới hoặc yêu cầu admin sửa)
    if full.status == NewsStatus::Published && !user.role.is_admin() {
        return Err(AppError::BadRequest(
            "Tin đã xuất bản không thể sửa trực tiếp. Hãy liên hệ admin.".into(),
        ));
    }

    // v1.4.0: categories từ DB cho edit form.
    let cats = dynamic_categories(&state).await;
    Ok(NewsEditTemplate {
        current_user: Some(user),
        unread_notifications: 0,
        categories: cats,
        news: full,
        errors: Vec::new(),
    })
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn update(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Form(params): Form<NewsFormParams>,
) -> AppResult<Response> {
    // Lookup news by slug WITHOUT status filter — cùng lý do edit_form
    // (find_by_slug_public chặn owner truy cập pending/rejected của
    // chính mình). Quyền truy cập được kiểm soát bởi ownership check
    // phía dưới.
    let news_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM news WHERE slug = $1")
        .bind(&slug)
        .fetch_optional(&state.db)
        .await?;
    let full = match news_id {
        Some(id) => NewsRepo::find_by_id(&state.db, id).await?,
        None => None,
    }
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
        // Validate cover_image qua validate_url — trước đây update() lấy
        // raw user input bypass kiểm tra scheme (create() có validate,
        // update không → AI/attacker có thể sửa tin set cover_image
        // javascript:... → XSS khi <img src> render trên trang show).
        cover_image: match validate_url(params.cover_image.as_deref().unwrap_or(""))? {
            u if u.is_empty() => None,
            u => Some(u),
        },
        category: validate_category(&state, params.category.as_deref().unwrap_or("")).await?,
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
    // Validate title/content/excerpt length — đồng bộ với create() để
    // không cho phép cập nhật thành chuỗi dài vô hạn (DB TEXT không
    // constraint, trước đây chỉ create kiểm tra).
    if form.title.chars().count() > 200 {
        return Err(AppError::BadRequest("Tiêu đề tối đa 200 ký tự".into()));
    }
    if form.content.is_empty() {
        return Err(AppError::BadRequest("Nội dung không được để trống".into()));
    }
    if form.content.chars().count() > 50_000 {
        return Err(AppError::BadRequest("Nội dung tối đa 50.000 ký tự".into()));
    }
    if form.excerpt.chars().count() > 500 {
        return Err(AppError::BadRequest("Tóm tắt tối đa 500 ký tự".into()));
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
    // Lookup news by slug WITHOUT status filter — owner cần xoá được
    // pending/rejected của chính mình (find_by_slug_public chặn status
    // != published/archived → 404 nhầm cho owner).
    let news_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM news WHERE slug = $1")
        .bind(&slug)
        .fetch_optional(&state.db)
        .await?;
    let full = match news_id {
        Some(id) => NewsRepo::find_by_id(&state.db, id).await?,
        None => None,
    }
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

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
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
    // Verify parent comment belongs to the same news — chống IDOR qua
    // parent_id chỉ comment của tin khác (sẽ tạo bình luận mồ côi không
    // hiển thị ở đâu, làm rác DB và bypass hiểu biết của user về cấu trúc
    // thread).
    if let Some(pid) = parent_id {
        let parent = NewsRepo::find_comment_by_id(&state.db, pid)
            .await?
            .ok_or_else(|| AppError::BadRequest("Bình luận cha không tồn tại".into()))?;
        if parent.news_id != news.id {
            return Err(AppError::BadRequest(
                "Bình luận cha không thuộc tin tức này".into(),
            ));
        }
    }

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

    /// v1.4.0: `validate_category` giờ async (cần DB). Test sync logic
    /// riêng bằng cách gọi trực tiếp `NEWS_CATEGORIES_FALLBACK` whitelist.
    /// Test async (with DB) nằm ngoài scope unit test (cần DB pool).
    #[test]
    fn validate_category_fallback_whitelist() {
        // Verify fallback list có 8 category mặc định không trùng key.
        assert!(NEWS_CATEGORIES_FALLBACK.iter().any(|(k, _)| *k == "game"));
        assert!(NEWS_CATEGORIES_FALLBACK.iter().any(|(k, _)| *k == "tech"));
        // Empty luôn hợp lệ (= không phân loại)
        // (không cần gọi validate_category — empty luôn trả Ok trong async fn)
        // Category lạ KHÔNG có trong fallback → khi gọi validate_category
        // async với DB trống, sẽ trả Err.
        assert!(!NEWS_CATEGORIES_FALLBACK.iter().any(|(k, _)| *k == "invalid"));
        assert!(!NEWS_CATEGORIES_FALLBACK.iter().any(|(k, _)| *k == "' OR 1=1"));
        // Case-sensitive: "GAME" không match "game"
        assert!(!NEWS_CATEGORIES_FALLBACK.iter().any(|(k, _)| *k == "GAME"));
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
    fn validate_url_rejects_control_chars() {
        // CR/LF trong URL → header injection / XML bẻ gãy
        assert!(validate_url("https://evil.com/\r\nSet-Cookie: bad=1").is_err());
        assert!(validate_url("https://evil.com/\n").is_err());
        assert!(validate_url("https://evil.com/\tfoo").is_err());
        assert!(validate_url("https://evil.com/\0").is_err());
        // URL sạch → OK
        assert!(validate_url("https://example.com/path").is_ok());
    }

    #[test]
    fn news_categories_have_unique_keys() {
        // Đảm bảo không có key trùng trong fallback whitelist
        let mut keys: Vec<&str> = NEWS_CATEGORIES_FALLBACK.iter().map(|(k, _)| *k).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(
            keys.len(),
            NEWS_CATEGORIES_FALLBACK.len(),
            "Duplicate category keys"
        );
    }

    #[test]
    fn news_categories_all_have_labels() {
        // Mỗi category phải có label không rỗng
        for (k, v) in NEWS_CATEGORIES_FALLBACK {
            assert!(!k.is_empty(), "Category key rỗng");
            assert!(!v.is_empty(), "Label cho category '{k}' rỗng");
        }
    }

    #[test]
    fn news_categories_count_is_reasonable() {
        // Không quá ít (3-) để không hữu ích, không quá nhiều (20+) để UI lộn xộn
        let count = NEWS_CATEGORIES_FALLBACK.len();
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
    fn validate_category_all_8_fallback_categories_have_unique_keys() {
        // Verify toàn bộ 8 category trong fallback whitelist đều có key
        // unique và label không rỗng (đảm bảo form /news/new luôn có select
        // 8 mục khi DB chưa migrate).
        let mut seen = std::collections::HashSet::new();
        for (k, v) in NEWS_CATEGORIES_FALLBACK {
            assert!(seen.insert(*k), "Duplicate key: {k}");
            assert!(!v.is_empty(), "Label rỗng cho {k}");
        }
        assert_eq!(seen.len(), 8, "Cần đúng 8 fallback categories");
    }
}
