use crate::error::{AppError, AppResult};
use crate::handlers::auth::unread_count;
use crate::middleware::{AuthUser, CurrentUser};
use crate::models::game::{GameForm, GameStatus, Platform};
use crate::models::report::ReportReason;
use crate::repositories::{CategoryRepo, GameRepo, InteractionRepo, NewsRepo, ReportRepo, TagRepo};
use crate::state::AppState;
use crate::templates::{IndexTemplate, NewGameTemplate, GameShowTemplate, EditGameTemplate, ReportModalPartial, GameListTemplate, CategoriesPageTemplate, SearchTemplate, MyGamesTemplate};
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

/// Helper to get unread notification count for an optional user
async fn unread_for(state: &AppState, user: Option<&crate::models::user::User>) -> i64 {
    match user {
        Some(u) => unread_count(state, u.id).await,
        None => 0,
    }
}

// ============= Home =============
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn home(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<IndexTemplate> {
    // 7 truy vấn độc lập chạy SONG SONG bằng join! — trước đây chạy tuần
    // tự, latency trang chủ = tổng thời gian 7 query. PgPool nội bộ là
    // Arc nên clone rẻ; mỗi future mượn connection riêng từ pool.
    // Thêm news queries (latest 3 published + count) cho section "Tin tức" ở homepage.
    let (
        featured_res,
        latest_res,
        trending_res,
        top_rated_res,
        categories_res,
        tags_res,
        total_res,
        latest_news_res,
        news_count_res,
    ) = tokio::join!(
        GameRepo::featured(&state.db, 6, 0),
        GameRepo::list_published(&state.db, 12, 0, "latest"),
        GameRepo::list_published(&state.db, 12, 0, "trending"),
        GameRepo::list_published(&state.db, 12, 0, "top_rated"),
        CategoryRepo::list_with_counts(&state.db),
        TagRepo::popular(&state.db, 20),
        GameRepo::count_published(&state.db),
        NewsRepo::list_published(&state.db, 1, 3),
        NewsRepo::count_published(&state.db),
    );
    let featured_games = featured_res.unwrap_or_default();
    let latest_games = latest_res?;
    let trending_games = trending_res?;
    let top_rated_games = top_rated_res?;
    let categories = categories_res?;
    let popular_tags = tags_res.unwrap_or_default();
    let total_games = total_res.unwrap_or(0);
    let latest_news = latest_news_res.unwrap_or_default();
    let total_news = news_count_res.unwrap_or(0);
    let unread = unread_for(&state, current_user.as_ref()).await;

    // JSON-LD schema.org/WebSite với SearchAction — giúp Google hiển thị
    // sitelinks searchbox ngay trên kết quả tìm kiếm (rich result).
    let json_ld = serde_json::json!({
        "@context": "https://schema.org",
        "@type": "WebSite",
        "name": "Louis Space",
        "url": state.config.base_url,
        "description": "Nền tảng chia sẻ game độc lập & tin tức cộng đồng Việt Nam",
        "inLanguage": "vi-VN",
        "potentialAction": {
            "@type": "SearchAction",
            "target": {
                "@type": "EntryPoint",
                "urlTemplate": format!("{}/search?q={{search_term_string}}", state.config.base_url),
            },
            "query-input": "required name=search_term_string",
        }
    });
    let json_ld = format!(
        "<script type=\"application/ld+json\">\n{}\n</script>",
        serde_json::to_string_pretty(&json_ld).unwrap_or_default()
    );

    Ok(IndexTemplate {
        current_user,
        unread_notifications: unread,
        featured_games,
        latest_games,
        trending_games,
        top_rated_games,
        categories,
        popular_tags,
        total_games,
        platforms_count: Platform::all().len(),
        json_ld,
        latest_news,
        total_news,
    })
}

/// Validate tất cả URL trong `GameForm` — dùng chung cho `create_game` và
/// `update_game` để tránh lặp code. Bao gồm:
/// - 5 link tải (Android, iOS, Windows, Linux, macOS) — chỉ http(s),
///   ≤ 2048 ký tự, chống XSS qua `javascript:` khi HTMX HX-Redirect.
/// - `cover_image` và `trailer_url` — chỉ http(s), ≤ 2048 ký tự.
/// - screenshot URLs (mỗi dòng 1 URL) — chỉ http(s), ≤ 2048 ký tự.
/// - title ≤ 200 ký tự (được dùng làm slug + hiển thị).
/// - tag count ≤ 20, mỗi tag ≤ 50 ký tự (chống lạm dụng).
fn validate_game_form(form: &GameForm) -> AppResult<()> {
    if form.title.trim().is_empty() {
        return Err(AppError::BadRequest("Tiêu đề không được để trống".into()));
    }
    if form.title.chars().count() > 200 {
        return Err(AppError::BadRequest("Tiêu đề tối đa 200 ký tự".into()));
    }
    // Excerpt ≤ 500 (khớp maxlength trong form — DB TEXT không constraint)
    if form.excerpt.chars().count() > 500 {
        return Err(AppError::BadRequest("Mô tả ngắn tối đa 500 ký tự".into()));
    }
    // Content không được rỗng — kiểm tra ở đây (dùng chung create/update)
    // vì trước đây chỉ create kiểm tra: update có thể xoá trắng nội dung.
    if form.content.trim().is_empty() {
        return Err(AppError::BadRequest("Nội dung không được để trống".into()));
    }
    // Content ≤ 50_000 — đủ cho mô tả game cực chi tiết có Markdown,
    // chặn payload hàng trăm MB làm phình DB & trang render chậm
    // (DB TEXT chấp nhận tới 1GB).
    if form.content.chars().count() > 50_000 {
        return Err(AppError::BadRequest(
            "Nội dung chi tiết tối đa 50.000 ký tự".into(),
        ));
    }
    // Metadata phụ ≤ 100 ký tự mỗi trường (version/developer/publisher/file_size)
    for (label, v) in [
        ("Phiên bản", &form.version),
        ("Nhà phát triển", &form.developer),
        ("Nhà phát hành", &form.publisher),
        ("Dung lượng", &form.file_size),
    ] {
        if v.chars().count() > 100 {
            return Err(AppError::BadRequest(format!("{label} tối đa 100 ký tự")));
        }
    }
    // Ngôn ngữ: tối đa 20, mỗi ngôn ngữ ≤ 50 ký tự
    let langs = form.languages_vec();
    if langs.len() > 20 {
        return Err(AppError::BadRequest("Tối đa 20 ngôn ngữ hỗ trợ".into()));
    }
    for l in &langs {
        if l.chars().count() > 50 {
            return Err(AppError::BadRequest("Mỗi ngôn ngữ tối đa 50 ký tự".into()));
        }
    }
    if !crate::utils::is_safe_url(&form.cover_image) {
        return Err(AppError::BadRequest(
            "Cover image URL phải là http:// hoặc https://".into(),
        ));
    }
    if form.cover_image.len() > 2048 {
        return Err(AppError::BadRequest(
            "Cover image URL quá dài (tối đa 2048 ký tự)".into(),
        ));
    }
    if !crate::utils::is_safe_url(&form.trailer_url) {
        return Err(AppError::BadRequest(
            "Trailer URL phải là http:// hoặc https://".into(),
        ));
    }
    // Validate screenshots (mỗi dòng 1 URL)
    for (i, line) in form.screenshots.lines().enumerate() {
        let url = line.trim();
        if url.is_empty() {
            continue;
        }
        if !crate::utils::is_safe_url(url) {
            return Err(AppError::BadRequest(format!(
                "Screenshot #{} URL phải là http:// hoặc https://",
                i + 1
            )));
        }
        if url.len() > 2048 {
            return Err(AppError::BadRequest(format!(
                "Screenshot #{} URL quá dài (tối đa 2048 ký tự)",
                i + 1
            )));
        }
    }
    // Validate tag count & length
    let tags_vec = form.tags_vec();
    if tags_vec.len() > 20 {
        return Err(AppError::BadRequest("Tối đa 20 tag mỗi game".into()));
    }
    for t in &tags_vec {
        if t.chars().count() > 50 {
            return Err(AppError::BadRequest("Mỗi tag tối đa 50 ký tự".into()));
        }
    }
    // Validate 5 link tải — chỉ http(s), ≤ 2048 ký tự
    for (label, url) in [
        ("Android", form.android_link.as_deref()),
        ("iOS", form.ios_link.as_deref()),
        ("Windows", form.windows_link.as_deref()),
        ("Linux", form.linux_link.as_deref()),
        ("macOS", form.macos_link.as_deref()),
    ] {
        if let Some(u) = url.filter(|s| !s.is_empty()) {
            let lower = u.to_ascii_lowercase();
            if !(lower.starts_with("http://") || lower.starts_with("https://")) {
                return Err(AppError::BadRequest(format!(
                    "Link tải {label} phải là http:// hoặc https:// (đã chặn javascript: và các scheme nguy hiểm)"
                )));
            }
            if u.len() > 2048 {
                return Err(AppError::BadRequest(format!(
                    "Link tải {label} quá dài (tối đa 2048 ký tự)"
                )));
            }
        }
    }
    Ok(())
}

/// Validate `category_id` trong form: nếu có thì phải tồn tại thật trong DB.
/// Form là \<select\> nhưng POST crafted vẫn gửi UUID lạ được → FK
/// violation → 500. Validate trước cho lỗi 400 sạch với thông điệp rõ.
async fn validate_category(state: &AppState, form: &GameForm) -> AppResult<()> {
    if let Some(cid) = form
        .category_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        let exists = CategoryRepo::find_by_id(&state.db, cid).await?.is_some();
        if !exists {
            return Err(AppError::BadRequest(
                "Thể loại không tồn tại. Vui lòng chọn lại.".into(),
            ));
        }
    }
    Ok(())
}

// ============= New game form =============
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn new_game_form(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<NewGameTemplate> {
    let categories = CategoryRepo::list_all(&state.db).await?;
    let unread = unread_count(&state, user.id).await;
    Ok(NewGameTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        categories,
    })
}

// ============= Create game =============
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn create_game(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Form(form): Form<GameForm>,
) -> AppResult<Redirect> {
    validate_game_form(&form)?;
    validate_category(&state, &form).await?;
    if form
        .android_link
        .as_deref().is_none_or(|s| s.is_empty())
        && form.ios_link.as_deref().is_none_or(|s| s.is_empty())
        && form
            .windows_link
            .as_deref().is_none_or(|s| s.is_empty())
        && form
            .linux_link
            .as_deref().is_none_or(|s| s.is_empty())
        && form
            .macos_link
            .as_deref().is_none_or(|s| s.is_empty())
    {
        return Err(AppError::BadRequest("Phải có ít nhất một link tải".into()));
    }

    // Sinh slug duy nhất: thử base, rồi base-2, base-3... tránh 500 do
    // trùng ràng buộc UNIQUE khi có nhiều game cùng tên.
    // Vòng 1: dò bằng SELECT EXISTS (nhanh, không đổi dữ liệu).
    // Nếu INSERT vẫn dính unique violation (race: 2 request đồng thời
    // check cùng lúc rồi cùng INSERT — TOCTOU), catch Conflict và dò
    // tiếp suffix mới thay vì trả 500 cho user.
    let base_slug = {
        let s = slug::slugify(&form.title);
        if s.is_empty() {
            "game".into()
        } else {
            s
        }
    };
    let mut slug = base_slug.clone();
    let mut suffix = 1u32;
    loop {
        // Trả DB error thay vì unwrap_or(true) — trước đây transient
        // DB error (network glitch, connection reset) bị coi là "slug
        // tồn tại", rồi retry 100 lần với suffix số đếm rồi break với
        // UUID random → user nhận URL xấu (my-game-101-{uuid}) mà
        // không biết DB đang lỗi.
        match GameRepo::slug_exists(&state.db, &slug).await {
            Ok(false) => break,            // slug free → dùng được
            Ok(true) => {}                 // trùng → thử suffix kế
            Err(e) => return Err(AppError::from(e)),
        }
        suffix += 1;
        slug = format!("{base_slug}-{suffix}");
        if suffix > 100 {
            // Vượt 100 lần thử — khả năng cao là logic slug có vấn đề
            // hoặc ai đó cố tình register 100 game cùng tên. Fallback
            // UUID để user vẫn tạo được, không bắt retry mãi.
            slug = format!("{}-{}", base_slug, uuid::Uuid::new_v4().simple());
            break;
        }
    }

    // INSERT với retry khi unique violation (race TOCTOU giữa EXISTS
    // và INSERT). Thử tối đa 3 lần, mỗi lần thêm suffix ngẫu nhiên.
    let mut id = None;
    for attempt in 0..3 {
        let candidate = if attempt == 0 {
            slug.clone()
        } else {
            format!("{}-{}", base_slug, uuid::Uuid::new_v4().simple())
        };
        match GameRepo::create(&state.db, user.id, &form, &candidate).await {
            Ok(new_id) => {
                id = Some(new_id);
                slug = candidate;
                break;
            }
            Err(AppError::Conflict(_)) if attempt < 2 => {
                tracing::warn!(
                    "Slug '{}' trùng do race condition (lần thử {}), thử slug khác",
                    candidate,
                    attempt + 1
                );
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    let id = id.ok_or_else(|| {
        AppError::Internal(anyhow::anyhow!(
            "Không tạo được slug duy nhất sau 3 lần thử"
        ))
    })?;
    tracing::info!("Game created: {} ({})", id, slug);

    Ok(Redirect::to(&format!("/games/{slug}")))
}

// ============= Show game =============
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn show_game(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<GameShowTemplate> {
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;

    let is_owner = current_user
        .as_ref()
        .is_some_and(|u| u.id == game.user_id);
    // Staff (admin/moderator) được xem game ẩn/nháp để kiểm duyệt báo cáo
    let is_staff = current_user
        .as_ref()
        .is_some_and(|u| u.role.is_staff());
    if !is_owner && !is_staff && !matches!(game.status, GameStatus::Published) {
        return Err(AppError::NotFound("Game không tồn tại".into()));
    }

    let mut game = game;
    if !is_owner {
        // View/download counters là best-effort analytics — KHÔNG block
        // render trang: spawn task nền ghi 2 UPDATE, template hiển thị
        // số đã +1 thủ công. Trước đây await tuần tự 2 query này cộng
        // thẳng vào latency TTFB của mọi lượt xem game.
        let db = state.db.clone();
        let game_id = game.id;
        tokio::spawn(async move {
            let _ = GameRepo::increment_view_count(&db, game_id).await;
            let _ = crate::repositories::StatsRepo::record_view(&db, game_id).await;
        });
        game.view_count += 1;
    }

    // 5 truy vấn độc lập (author/links/screenshots/tags/category) chạy
    // song song — trước đây tuần tự, trang game chịu tổng 5 round-trip.
    let (author_res, links_res, screenshots_res, tags_res, category_res) = tokio::join!(
        crate::repositories::UserRepo::find_by_id(&state.db, game.user_id),
        GameRepo::get_links(&state.db, game.id),
        GameRepo::get_screenshots(&state.db, game.id),
        GameRepo::get_tags(&state.db, game.id),
        async {
            match game.category_id {
                Some(cat_id) => CategoryRepo::find_by_id(&state.db, cat_id).await,
                None => Ok(None),
            }
        },
    );
    let author = author_res?.ok_or_else(|| AppError::NotFound("Tác giả không tồn tại".into()))?;
    let links = links_res?;
    let screenshots = screenshots_res?;
    let tags = tags_res?;
    let category = category_res?;
    let comments = crate::repositories::CommentRepo::list_by_game(
        &state.db,
        game.id,
        current_user.as_ref().map(|u| u.id),
        50,
        0,
    )
    .await?;
    let related_games = GameRepo::related(&state.db, game.id, game.category_id, 6).await?;

    // 4 check trạng thái tương tác (like/bookmark/follow/rating) độc lập
    // — join! chạy đồng thời thay vì 4 lần chờ tuần tự.
    let (is_liked, is_bookmarked, is_following_author, user_rating) =
        if let Some(ref u) = current_user {
            let (liked_res, bm_res, following_res, rating_res) = tokio::join!(
                InteractionRepo::is_liked(&state.db, game.id, u.id),
                InteractionRepo::is_bookmarked(&state.db, game.id, u.id),
                async {
                    if u.id == author.id {
                        Ok(false)
                    } else {
                        InteractionRepo::is_following(&state.db, u.id, author.id).await
                    }
                },
                InteractionRepo::get_user_rating(&state.db, game.id, u.id),
            );
            (
                liked_res.unwrap_or(false),
                bm_res.unwrap_or(false),
                following_res.unwrap_or(false),
                rating_res.unwrap_or(None),
            )
        } else {
            (false, false, false, None)
        };

    let unread = unread_for(&state, current_user.as_ref()).await;

    // Structured data (JSON-LD) cho SEO: schema.org/VideoGame —
    // giúp Google hiển thị rich snippet (rating, lượt tải, v.v.) trên
    // kết quả tìm kiếm. Serialize trước bằng serde_json thay vì cố
    // render JSON trong askama (không có filter json_encode ở 0.16).
    let json_ld = build_game_json_ld(
        &state.config.base_url,
        &game,
        &author,
        &links,
        &tags,
        category.as_ref(),
    );
    // Breadcrumb JSON-LD riêng — Google hiển thị breadcrumb thay URL trần
    // trong kết quả tìm kiếm (rich result), tăng CTR đáng kể.
    let breadcrumb_ld = build_breadcrumb_json_ld(&state.config.base_url, &game, category.as_ref());

    Ok(GameShowTemplate {
        current_user,
        unread_notifications: unread,
        game,
        author,
        links,
        screenshots,
        tags,
        category,
        comments,
        related_games,
        is_liked,
        is_bookmarked,
        is_following_author,
        is_owner,
        user_rating,
        base_url: state.config.base_url.clone(),
        json_ld: format!("{json_ld}\n{breadcrumb_ld}"),
    })
}

/// Dựng JSON-LD schema.org/BreadcrumbList: Trang chủ › [Thể loại] › Tên game.
/// Đồng bộ markup với \<nav class="breadcrumb"\> trong template show.html.
fn build_breadcrumb_json_ld(
    base_url: &str,
    game: &crate::models::game::Game,
    category: Option<&crate::models::category::Category>,
) -> String {
    let mut items = vec![serde_json::json!({
        "@type": "ListItem",
        "position": 1,
        "name": "Trang chủ",
        "item": base_url,
    })];
    if let Some(cat) = category {
        items.push(serde_json::json!({
            "@type": "ListItem",
            "position": 2,
            "name": cat.name,
            "item": format!("{}/c/{}", base_url, cat.slug),
        }));
    }
    items.push(serde_json::json!({
        "@type": "ListItem",
        "position": items.len() + 1,
        "name": game.title,
    }));
    let ld = serde_json::json!({
        "@context": "https://schema.org",
        "@type": "BreadcrumbList",
        "itemListElement": items,
    });
    format!(
        "<script type=\"application/ld+json\">\n{}\n</script>",
        serde_json::to_string_pretty(&ld).unwrap_or_default()
    )
}

/// Dựng JSON-LD schema.org/VideoGame để nhúng vào \<head\> của trang game.
/// Dùng `serde_json::Value` để tránh lỗi cú pháp JSON (escape không đúng).
/// Trả về tag <script type="application/ld+json">...</script> hoàn chỉnh.
fn build_game_json_ld(
    base_url: &str,
    game: &crate::models::game::Game,
    author: &crate::models::user::User,
    links: &[crate::models::game::GameLink],
    tags: &[String],
    category: Option<&crate::models::category::Category>,
) -> String {
    use serde_json::{json, Value};
    let mut root = json!({
        "@context": "https://schema.org",
        "@type": "VideoGame",
        "name": game.title,
        "url": format!("{}/games/{}", base_url, game.slug),
        "author": {
            "@type": "Person",
            "name": author.display_name,
            "url": format!("{}/u/{}", base_url, author.username),
        },
        "publisher": {
            "@type": "Organization",
            "name": "Louis Space",
            "url": base_url,
        },
        "operatingSystem": links.iter().map(|l| l.platform.label()).collect::<Vec<_>>(),
        "interactionStatistic": [
            json!({
                "@type": "InteractionCounter",
                "interactionType": "https://schema.org/WatchAction",
                "userInteractionCount": game.view_count,
            }),
            json!({
                "@type": "InteractionCounter",
                "interactionType": "https://schema.org/DownloadAction",
                "userInteractionCount": game.download_count,
            }),
            json!({
                "@type": "InteractionCounter",
                "interactionType": "https://schema.org/LikeAction",
                "userInteractionCount": game.like_count,
            }),
            json!({
                "@type": "InteractionCounter",
                "interactionType": "https://schema.org/CommentAction",
                "userInteractionCount": game.comment_count,
            }),
        ],
    });
    let obj = root.as_object_mut().unwrap();
    if !game.excerpt_or().is_empty() {
        obj.insert("description".into(), json!(game.excerpt_or()));
    }
    if let Some(url) = game.cover_image.as_deref().filter(|s| !s.is_empty()) {
        obj.insert("image".into(), json!(url));
    }
    if game.rating_count > 0 {
        obj.insert(
            "aggregateRating".into(),
            json!({
                "@type": "AggregateRating",
                "ratingValue": game.rating_avg_f64(),
                "ratingCount": game.rating_count,
                "bestRating": 5,
                "worstRating": 1,
            }),
        );
    }
    if let Some(d) = game.release_date {
        obj.insert(
            "datePublished".into(),
            json!(d.format("%Y-%m-%d").to_string()),
        );
    }
    if let Some(cat) = category {
        obj.insert("genre".into(), json!(cat.name));
    }
    if !tags.is_empty() {
        obj.insert(
            "keywords".into(),
            Value::Array(tags.iter().map(|t| json!(t)).collect()),
        );
    }
    let pretty = serde_json::to_string_pretty(&root).unwrap_or_else(|_| "{}".into());
    format!(
        "<script type=\"application/ld+json\">\n{pretty}\n</script>"
    )
}

// ============= Edit game =============
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn edit_game_form(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
) -> AppResult<EditGameTemplate> {
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    if game.user_id != user.id && !user.role.is_staff() {
        return Err(AppError::Forbidden("Bạn không có quyền chỉnh sửa".into()));
    }
    // 4 query độc lập (categories/links/screenshots/tags) chạy song song.
    let (categories_res, links_res, screenshots_res, tags_res) = tokio::join!(
        CategoryRepo::list_all(&state.db),
        GameRepo::get_links(&state.db, game.id),
        GameRepo::get_screenshots(&state.db, game.id),
        GameRepo::get_tags(&state.db, game.id),
    );
    let categories = categories_res?;
    let links = links_res?;
    let screenshots = screenshots_res?;
    let tags = tags_res?;
    let unread = unread_count(&state, user.id).await;
    Ok(EditGameTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        categories,
        game,
        links,
        screenshots,
        tags,
    })
}

// ============= Update game =============
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn update_game(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Form(form): Form<GameForm>,
) -> AppResult<Redirect> {
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    if game.user_id != user.id && !user.role.is_staff() {
        return Err(AppError::Forbidden("Bạn không có quyền chỉnh sửa".into()));
    }
    // Validate tất cả URL & length — dùng chung với create_game
    validate_game_form(&form)?;
    validate_category(&state, &form).await?;

    GameRepo::update(&state.db, game.id, &form).await?;
    Ok(Redirect::to(&format!("/games/{slug}")))
}

// ============= Delete game =============
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn delete_game(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
) -> AppResult<Redirect> {
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    if game.user_id != user.id && !user.role.is_staff() {
        return Err(AppError::Forbidden("Bạn không có quyền xóa".into()));
    }
    GameRepo::delete(&state.db, game.id).await?;
    Ok(Redirect::to("/"))
}

// ============= Download (hidden link redirect) =============
#[derive(Deserialize)]
pub struct DownloadForm {
    pub platform: String,
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn download_game(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    headers: axum::http::HeaderMap,
    connect_info: axum::extract::ConnectInfo<std::net::SocketAddr>,
    Form(form): Form<DownloadForm>,
) -> AppResult<Response> {
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    // Chỉ tải game đã xuất bản (owner/staff vẫn tải được game của mình
    // để test). Trước đây POST /games/{slug}/download không kiểm tra
    // status — ai biết slug đều tải được game ẩn/nháp.
    if game.user_id != user.id && !user.role.is_staff() && game.status != GameStatus::Published {
        return Err(AppError::NotFound("Game không tồn tại".into()));
    }
    let platform = Platform::from_str(&form.platform)
        .ok_or_else(|| AppError::BadRequest("Nền tảng không hợp lệ".into()))?;
    let url = GameRepo::get_link_for_platform(&state.db, game.id, &platform)
        .await?
        .ok_or_else(|| AppError::NotFound("Link tải không tồn tại".into()))?;

    // Ghi analytics với chuỗi enum CHUẨN (đã parse) — form.platform thô
    // có thể là "ANDROID"/"Mac OS" vẫn parse được nhờ from_str lowercase
    // nhưng cast $3::platform_type trong INSERT sẽ fail ngầm (let _ =
    // nuốt error) → mất dòng stats mà không ai biết.
    //
    // IP client ghi kèm để phân tích fraud (1 IP tải cùng game hàng
    // trăm lần = bump download_count ảo). Tôn trọng TRUST_PROXY_HEADERS
    // như rate-limit middleware.
    let ip = crate::middleware::client_ip_from_parts(
        &headers,
        Some(&connect_info.0),
        state.config.trust_proxy_headers,
    );
    // Analytics download chạy nền — response redirect cho user ngay
    // (trước đây 4 await tuần tự cộng vào thời gian chờ bấm nút Tải).
    // Redirect URL đã có sẵn trong `url`, không phụ thuộc kết quả ghi.
    {
        let db = state.db.clone();
        let game_id = game.id;
        let user_id = user.id;
        let platform_str = platform.as_str().to_string();
        tokio::spawn(async move {
            let _ = InteractionRepo::record_download(
                &db,
                game_id,
                Some(user_id),
                &platform_str,
                Some(&ip),
            )
            .await;
            let _ = GameRepo::increment_download_count(&db, game_id).await;
            let _ = crate::repositories::StatsRepo::record_download(&db, game_id).await;
        });
    }

    Ok((
        StatusCode::OK,
        [("X-Redirect", url.as_str()), ("HX-Redirect", url.as_str())],
        "",
    )
        .into_response())
}

// ============= Report form (modal HTML) =============
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn report_form(
    State(state): State<Arc<AppState>>,
    AuthUser(_user): AuthUser,
    Path(slug): Path<String>,
) -> AppResult<Html<String>> {
    let _ = state;
    let partial = ReportModalPartial { slug: &slug };
    Ok(Html(partial.render()?))
}

// ============= Submit report =============
#[derive(Deserialize)]
pub struct ReportForm {
    pub reason: String,
    pub description: Option<String>,
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn submit_report(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
    Form(form): Form<ReportForm>,
) -> AppResult<Html<String>> {
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    // Không nhận báo cáo cho game chưa xuất bản — owner/staff xem được
    // game đó nhưng không cần báo cáo (báo cáo là cơ chế kiểm duyệt nội
    // dung công khai).
    if game.user_id != user.id && !user.role.is_staff() && game.status != GameStatus::Published {
        return Err(AppError::NotFound("Game không tồn tại".into()));
    }

    let reason = ReportReason::from_str(&form.reason)
        .ok_or_else(|| AppError::BadRequest("Lý do không hợp lệ".into()))?;

    if ReportRepo::has_reported(&state.db, game.id, user.id).await? {
        return Ok(Html(
            "<div class='alert alert-warning'>Bạn đã báo cáo game này rồi.</div>".into(),
        ));
    }
    // Validate description length — chống lạm dụng (DB field TEXT không
    // có constraint). 2000 ký tự là đủ cho mô tả chi tiết báo cáo.
    let description = form.description.unwrap_or_default();
    let description = description.trim();
    if description.chars().count() > 2000 {
        return Err(AppError::BadRequest(
            "Mô tả báo cáo tối đa 2000 ký tự".into(),
        ));
    }

    ReportRepo::create(&state.db, game.id, user.id, &reason, description).await?;

    Ok(Html(
        "<div class='alert alert-success'>✓ Báo cáo đã được gửi. Cảm ơn bạn đã đóng góp!</div>"
            .into(),
    ))
}

// ============= List pages =============
#[derive(Deserialize, Default)]
pub struct ListQuery {
    pub sort: Option<String>,
    pub page: Option<i64>,
}

async fn build_list_template(
    state: &AppState,
    current_user: Option<crate::models::user::User>,
    title: &str,
    list_type: &str,
    default_sort: &str,
    q: ListQuery,
) -> AppResult<GameListTemplate> {
    let sort = q.sort.unwrap_or_else(|| default_sort.to_string());
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 24;
    let offset = (page - 1) * per_page;

    // Trang "featured" chỉ liệt kê game nổi bật, không phải toàn bộ game.
    // games + total độc lập — join! song song.
    let (games_res, total_res) = if list_type == "featured" {
        tokio::join!(
            GameRepo::featured(&state.db, per_page, offset),
            GameRepo::count_featured(&state.db),
        )
    } else {
        tokio::join!(
            GameRepo::list_published(&state.db, per_page, offset, &sort),
            GameRepo::count_published(&state.db),
        )
    };
    let games = games_res?;
    let total = total_res.unwrap_or(0);
    let unread = unread_for(state, current_user.as_ref()).await;
    let base = if list_type == "all" {
        "/games".to_string()
    } else {
        format!("/games/{list_type}")
    };
    Ok(GameListTemplate {
        current_user,
        unread_notifications: unread,
        title: title.into(),
        games,
        total,
        page,
        per_page,
        sort,
        list_type: list_type.into(),
        base_url: base,
        site_url: state.config.base_url.clone(),
        category: None,
        tag: None,
    })
}

/// Danh mục đầy đủ tại /games (trước đây chỉ có POST → GET trả 405)
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn list_all(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(q): Query<ListQuery>,
) -> AppResult<GameListTemplate> {
    build_list_template(&state, current_user, "🎮 Tất cả game", "all", "latest", q).await
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn list_latest(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(q): Query<ListQuery>,
) -> AppResult<GameListTemplate> {
    build_list_template(
        &state,
        current_user,
        "🎮 Game mới nhất",
        "latest",
        "latest",
        q,
    )
    .await
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn list_trending(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(q): Query<ListQuery>,
) -> AppResult<GameListTemplate> {
    build_list_template(
        &state,
        current_user,
        "🔥 Đang thịnh hành",
        "trending",
        "trending",
        q,
    )
    .await
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn list_top_rated(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(q): Query<ListQuery>,
) -> AppResult<GameListTemplate> {
    build_list_template(
        &state,
        current_user,
        "⭐ Đánh giá cao nhất",
        "top-rated",
        "top_rated",
        q,
    )
    .await
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn list_downloads(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(q): Query<ListQuery>,
) -> AppResult<GameListTemplate> {
    build_list_template(
        &state,
        current_user,
        "⬇️ Tải nhiều nhất",
        "downloads",
        "downloads",
        q,
    )
    .await
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn list_featured(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(q): Query<ListQuery>,
) -> AppResult<GameListTemplate> {
    build_list_template(
        &state,
        current_user,
        "⭐ Game nổi bật",
        "featured",
        "trending",
        q,
    )
    .await
}

// ============= Category / Tag listing =============
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn list_by_category(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Path(cat_slug): Path<String>,
    Query(q): Query<ListQuery>,
) -> AppResult<GameListTemplate> {
    let category = CategoryRepo::find_by_slug(&state.db, &cat_slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Thể loại không tồn tại".into()))?;
    let sort = q.sort.unwrap_or_else(|| "latest".into());
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 24;
    let offset = (page - 1) * per_page;
    // games + count độc lập — join! song song.
    let (games_res, total_res) = tokio::join!(
        GameRepo::by_category(&state.db, &cat_slug, per_page, offset, &sort),
        GameRepo::count_by_category(&state.db, &cat_slug),
    );
    let games = games_res?;
    let total = total_res.unwrap_or(0);
    let unread = unread_for(&state, current_user.as_ref()).await;
    Ok(GameListTemplate {
        current_user,
        unread_notifications: unread,
        title: format!("📁 {}", category.name),
        games,
        total,
        page,
        per_page,
        sort,
        list_type: "category".into(),
        base_url: format!("/c/{cat_slug}"),
        site_url: state.config.base_url.clone(),
        category: Some(category),
        tag: None,
    })
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn list_by_tag(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Path(tag_slug): Path<String>,
    Query(q): Query<ListQuery>,
) -> AppResult<GameListTemplate> {
    let tag = TagRepo::find_by_slug(&state.db, &tag_slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Tag không tồn tại".into()))?;
    let sort = q.sort.unwrap_or_else(|| "latest".into());
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 24;
    let offset = (page - 1) * per_page;
    let (games_res, total_res) = tokio::join!(
        GameRepo::by_tag(&state.db, &tag_slug, per_page, offset, &sort),
        GameRepo::count_by_tag(&state.db, &tag_slug),
    );
    let games = games_res?;
    let total = total_res.unwrap_or(0);
    let unread = unread_for(&state, current_user.as_ref()).await;
    Ok(GameListTemplate {
        current_user,
        unread_notifications: unread,
        title: format!("#{}", tag.name),
        games,
        total,
        page,
        per_page,
        sort,
        list_type: "tag".into(),
        base_url: format!("/t/{tag_slug}"),
        site_url: state.config.base_url.clone(),
        category: None,
        tag: Some(tag),
    })
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn list_categories(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<CategoriesPageTemplate> {
    let cats = CategoryRepo::list_with_counts(&state.db).await?;
    let unread = unread_for(&state, current_user.as_ref()).await;
    Ok(CategoriesPageTemplate {
        current_user,
        unread_notifications: unread,
        categories: cats,
    })
}

// ============= Search =============
#[derive(Deserialize, Default)]
pub struct SearchQuery {
    // Sửa: `q` là Option để không trả 400 khi user vào /search không có query string.
    // Trang /search trống sẽ hiển thị danh sách game mới nhất (như browse page).
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub page: Option<i64>,
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn search(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Query(q): Query<SearchQuery>,
) -> AppResult<SearchTemplate> {
    let sort = q.sort.clone().unwrap_or_else(|| "latest".into());
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 24;
    let offset = (page - 1) * per_page;
    // Clamp từ khóa 200 ký tự (bằng giới hạn title game) — chống gửi
    // pattern khổng lồ làm ILIKE quét chậm toàn bảng games.
    let q_q: String = q.q.chars().take(200).collect();
    let has_filter = !q_q.trim().is_empty() || q.category.is_some() || q.platform.is_some();
    // 3 truy vấn độc lập (games/total/categories) — join! song song,
    // trước đây search + count + categories cộng dồn 3 round-trip.
    let (games, total, categories) = tokio::join!(
        async {
            if has_filter {
                GameRepo::search(
                    &state.db,
                    q_q.trim(),
                    q.category.as_deref(),
                    q.platform.as_deref(),
                    &sort,
                    per_page,
                    offset,
                )
                .await
            } else {
                GameRepo::list_published(&state.db, per_page, offset, &sort).await
            }
        },
        async {
            // Đếm đúng số kết quả khớp bộ lọc (trước đây lấy tổng game
            // đã đăng → 'N kết quả' và phân trang sai khi có từ khóa)
            if has_filter {
                GameRepo::count_search(
                    &state.db,
                    q_q.trim(),
                    q.category.as_deref(),
                    q.platform.as_deref(),
                )
                .await
                .unwrap_or(0)
            } else {
                GameRepo::count_published(&state.db).await.unwrap_or(0)
            }
        },
        CategoryRepo::list_all(&state.db),
    );
    let games = games?;
    let categories = categories?;
    let unread = unread_for(&state, current_user.as_ref()).await;
    Ok(SearchTemplate {
        current_user,
        unread_notifications: unread,
        query: q_q,
        sort,
        platform: q.platform,
        category_slug: q.category,
        games,
        categories,
        total,
        page,
        per_page,
    })
}

// ============= Share tracking =============
#[derive(Deserialize)]
pub struct ShareForm {
    pub platform: String,
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn share_game(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
    Path(slug): Path<String>,
    Form(form): Form<ShareForm>,
) -> AppResult<Html<String>> {
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    let user_id = current_user.as_ref().map(|u| u.id);
    // Chuẩn hoá platform về enum DB qua SharePlatform::from_str —
    // trước đây handler tự giữ whitelist string riêng, trùng logic với
    // model (2 nguồn truth dễ lệch khi thêm platform mới). Giá trị lạ
    // → "copy" (from_str default) — share vẫn được ghi nhận analytics.
    let platform = crate::models::interaction::SharePlatform::from_str(&form.platform).as_str();
    // Share analytics chạy nền — response trả ngay để JS copy link
    // clipboard không chờ DB (cùng pattern view/download counters).
    {
        let db = state.db.clone();
        let game_id = game.id;
        let platform_str = platform.to_string();
        tokio::spawn(async move {
            let _ = InteractionRepo::record_share(&db, game_id, user_id, &platform_str).await;
            let _ = GameRepo::increment_share_count(&db, game_id).await;
        });
    }
    Ok(Html("<span></span>".into()))
}

// ============= Game của tôi =============
#[derive(Deserialize, Default)]
pub struct MyGamesQuery {
    pub page: Option<i64>,
}

/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn my_games(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Query(q): Query<MyGamesQuery>,
) -> AppResult<MyGamesTemplate> {
    let page = q.page.unwrap_or(1).max(1);
    let per_page: i64 = 30;
    let offset = (page - 1) * per_page;
    // games + total độc lập — join! song song.
    let (games_res, total_res) = tokio::join!(
        GameRepo::all_by_user(&state.db, user.id, per_page, offset),
        GameRepo::count_all_by_user(&state.db, user.id),
    );
    let games = games_res?;
    let total = total_res.unwrap_or(0);
    let unread = unread_count(&state, user.id).await;
    Ok(MyGamesTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        games,
        total,
        page,
        per_page,
    })
}

// ============= Xuất bản game nháp =============
/// # Errors
///
/// Trả về lỗi khi thao tác thất bại (DB, I/O, validation).
pub async fn publish_game(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(slug): Path<String>,
) -> AppResult<Html<String>> {
    let game = GameRepo::find_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound("Game không tồn tại".into()))?;
    if game.user_id != user.id && !user.role.is_staff() {
        return Err(AppError::Forbidden("Bạn không có quyền".into()));
    }
    GameRepo::publish(&state.db, game.id).await?;
    Ok(Html(
        "<div class='alert alert-success'>Đã xuất bản game.</div>".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tạo form hợp lệ tối thiểu để các test chỉ cần sửa field liên quan.
    fn valid_form() -> GameForm {
        GameForm {
            title: "Game thử nghiệm".into(),
            excerpt: "Mô tả ngắn".into(),
            content: "Nội dung đầy đủ".into(),
            status: "published".into(),
            version: "1.0".into(),
            developer: "Studio A".into(),
            publisher: "Publisher B".into(),
            release_date: Some("2026-01-01".into()),
            file_size: "100MB".into(),
            age_rating: "everyone".into(),
            languages: "vi, en".into(),
            trailer_url: "https://youtube.com/watch?v=abc".into(),
            cover_image: "https://cdn.example.com/cover.png".into(),
            category_id: None,
            tags: "action, rpg".into(),
            screenshots: "https://cdn.example.com/1.png".into(),
            android_link: Some("https://example.com/apk".into()),
            ios_link: None,
            windows_link: None,
            linux_link: None,
            macos_link: None,
        }
    }

    #[test]
    fn test_validate_game_form_ok() {
        assert!(validate_game_form(&valid_form()).is_ok());
    }

    #[test]
    fn test_validate_title_empty() {
        let mut f = valid_form();
        f.title = "   ".into();
        assert!(matches!(
            validate_game_form(&f),
            Err(AppError::BadRequest(m)) if m.contains("trống")
        ));
    }

    #[test]
    fn test_validate_title_too_long() {
        let mut f = valid_form();
        f.title = "x".repeat(201);
        assert!(matches!(
            validate_game_form(&f),
            Err(AppError::BadRequest(m)) if m.contains("200")
        ));
        // Đúng 200 ký tự thì qua
        f.title = "y".repeat(200);
        assert!(validate_game_form(&f).is_ok());
    }

    #[test]
    fn test_validate_cover_image_javascript_blocked() {
        let mut f = valid_form();
        f.cover_image = "javascript:alert(1)".into();
        assert!(validate_game_form(&f).is_err());
        f.cover_image = "data:image/png;base64,xxx".into();
        assert!(validate_game_form(&f).is_err());
    }

    #[test]
    fn test_validate_trailer_url_scheme() {
        let mut f = valid_form();
        f.trailer_url = "ftp://evil.com/x.mp4".into();
        assert!(validate_game_form(&f).is_err());
        f.trailer_url = "https://youtube.com/watch?v=ok".into();
        assert!(validate_game_form(&f).is_ok());
    }

    #[test]
    fn test_validate_screenshot_lines() {
        let mut f = valid_form();
        // Dòng 2 là scheme nguy hiểm → chặn kèm số thứ tự dòng
        f.screenshots = "https://ok.com/1.png\njavascript:evil\nhttps://ok.com/2.png".into();
        assert!(matches!(
            validate_game_form(&f),
            Err(AppError::BadRequest(m)) if m.contains("#2")
        ));
    }

    #[test]
    fn test_validate_too_many_tags() {
        let mut f = valid_form();
        f.tags = (0..21)
            .map(|i| format!("tag{i}"))
            .collect::<Vec<_>>()
            .join(",");
        assert!(matches!(
            validate_game_form(&f),
            Err(AppError::BadRequest(m)) if m.contains("20 tag")
        ));
        // Đúng 20 tag thì qua (đã dedupe)
        f.tags = (0..20)
            .map(|i| format!("tag{i}"))
            .collect::<Vec<_>>()
            .join(",");
        assert!(validate_game_form(&f).is_ok());
    }

    #[test]
    fn test_validate_tag_too_long() {
        let mut f = valid_form();
        f.tags = format!("{},{}", "a".repeat(50), "b".repeat(51));
        assert!(matches!(
            validate_game_form(&f),
            Err(AppError::BadRequest(m)) if m.contains("50 ký tự")
        ));
    }

    #[test]
    fn test_validate_download_link_schemes() {
        let mut f = valid_form();
        f.android_link = Some("javascript:alert(1)".into());
        assert!(matches!(
            validate_game_form(&f),
            Err(AppError::BadRequest(m)) if m.contains("Android")
        ));
        f.android_link = None;
        f.windows_link = Some("file:///C:/evil.exe".into());
        assert!(matches!(
            validate_game_form(&f),
            Err(AppError::BadRequest(m)) if m.contains("Windows")
        ));
    }

    #[test]
    fn test_validate_url_length_limits() {
        let mut f = valid_form();
        f.cover_image = format!("https://example.com/{}", "a".repeat(2100));
        assert!(matches!(
            validate_game_form(&f),
            Err(AppError::BadRequest(m)) if m.contains("2048")
        ));
    }

    #[test]
    fn test_validate_25_tags_after_dedupe_ok() {
        // 25 tag nhưng sau dedupe chỉ còn 20 → phải pass
        let mut f = valid_form();
        f.tags = (0..20)
            .map(|i| format!("tag{i}, TAG{i}"))
            .collect::<Vec<_>>()
            .join(",");
        assert!(validate_game_form(&f).is_ok());
    }
}

#[cfg(test)]
mod tests_json_ld {
    use super::*;
    use crate::models::game::{AgeRating, Game, GameStatus};
    use crate::models::user::{User, UserRole};

    /// Dựng Game đủ field để test JSON-LD — không cần DB.
    fn sample_game() -> Game {
        Game {
            id: uuid::Uuid::new_v4(),
            user_id: uuid::Uuid::new_v4(),
            title: "Game Thử Nghiệm".into(),
            slug: "game-thu-nghiem".into(),
            excerpt: Some("Mô tả ngắn".into()),
            content: Some("Nội dung".into()),
            status: GameStatus::Published,
            version: Some("1.0".into()),
            developer: Some("Studio".into()),
            publisher: None,
            release_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 15),
            file_size: Some("100MB".into()),
            age_rating: AgeRating::Teen,
            languages: vec!["vi".into()],
            trailer_url: None,
            cover_image: Some("https://cdn.example.com/c.png".into()),
            category_id: None,
            view_count: 10,
            download_count: 5,
            like_count: 3,
            comment_count: 2,
            share_count: 1,
            rating_avg: bigdecimal::BigDecimal::from(45) / 10, // 4.5
            rating_count: 4,
            is_featured: false,
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn sample_author() -> User {
        User {
            id: uuid::Uuid::new_v4(),
            email: "a@b.c".into(),
            username: "tester".into(),
            display_name: "Tester".into(),
            avatar_url: None,
            bio: None,
            google_sub: "sub".into(),
            role: UserRole::User,
            is_banned: false,
            last_seen_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            signup_ip: None,
            signup_ua: None,
            last_login_ip: None,
            last_login_ua: None,
            last_login_at: None,
        }
    }

    #[test]
    fn test_json_ld_core_fields() {
        let g = sample_game();
        let author = sample_author();
        let html = build_game_json_ld("https://example.com", &g, &author, &[], &[], None);
        // Bọc trong thẻ script đúng loại
        assert!(html.contains(r#"<script type="application/ld+json">"#));
        // Parse được JSON hợp lệ (nội dung giữa 2 thẻ script)
        let start = html.find('>').map(|i| i + 1).unwrap();
        let end = html.rfind("</script>").unwrap();
        let json: serde_json::Value =
            serde_json::from_str(html[start..end].trim()).expect("JSON-LD phải là JSON hợp lệ");
        assert_eq!(json["@type"], "VideoGame");
        assert_eq!(json["name"], "Game Thử Nghiệm");
        assert_eq!(json["url"], "https://example.com/games/game-thu-nghiem");
        assert_eq!(json["author"]["name"], "Tester");
    }

    #[test]
    fn test_json_ld_rating_only_when_has_ratings() {
        let g = sample_game();
        let author = sample_author();
        let html = build_game_json_ld("https://example.com", &g, &author, &[], &[], None);
        assert!(html.contains("aggregateRating"));
        assert!(html.contains("4.5"));

        // Game chưa ai đánh giá → KHÔNG có aggregateRating (Google rich
        // result error khi ratingCount = 0)
        let mut g0 = sample_game();
        g0.rating_count = 0;
        let html0 = build_game_json_ld("https://example.com", &g0, &author, &[], &[], None);
        assert!(!html0.contains("aggregateRating"));
    }

    #[test]
    fn test_json_ld_genre_and_keywords() {
        let g = sample_game();
        let author = sample_author();
        let cat = crate::models::category::Category {
            id: uuid::Uuid::new_v4(),
            name: "Hành động".into(),
            slug: "hanh-dong".into(),
            description: None,
            icon: None,
            created_at: chrono::Utc::now(),
        };
        let tags = vec!["coop".to_string(), "pixel".to_string()];
        let html = build_game_json_ld("https://example.com", &g, &author, &[], &tags, Some(&cat));
        assert!(html.contains(r#""genre": "Hành động""#));
        assert!(html.contains("coop"));
        assert!(html.contains("pixel"));
    }

    #[test]
    fn test_breadcrumb_with_category() {
        let g = sample_game();
        let cat = crate::models::category::Category {
            id: uuid::Uuid::new_v4(),
            name: "Hành động".into(),
            slug: "hanh-dong".into(),
            description: None,
            icon: None,
            created_at: chrono::Utc::now(),
        };
        let html = build_breadcrumb_json_ld("https://example.com", &g, Some(&cat));
        assert!(html.contains("BreadcrumbList"));
        assert!(html.contains("Trang chủ"));
        assert!(html.contains("Hành động"));
        assert!(html.contains("/c/hanh-dong"));
        assert!(html.contains("Game Thử Nghiệm"));
        // 3 cấp: Trang chủ › Thể loại › Game
        assert_eq!(html.matches("ListItem").count(), 3);
        // JSON hợp lệ
        let start = html.find('>').map(|i| i + 1).unwrap();
        let end = html.rfind("</script>").unwrap();
        let json: serde_json::Value =
            serde_json::from_str(html[start..end].trim()).expect("breadcrumb JSON hợp lệ");
        assert_eq!(json["@type"], "BreadcrumbList");
    }

    #[test]
    fn test_breadcrumb_without_category() {
        let g = sample_game();
        let html = build_breadcrumb_json_ld("https://example.com", &g, None);
        // 2 cấp: Trang chủ › Game — không có URL category
        assert_eq!(html.matches("ListItem").count(), 2);
        assert!(!html.contains("/c/"));
    }
}
