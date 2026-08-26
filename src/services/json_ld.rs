//! JSON-LD schema.org builder cho trang game và homepage.
//!
//! Tách ra từ `handlers/games.rs` để giảm kích thước file (1649 lines →
//! bớt ~100 lines json_ld builder) và để `services` layer sở hữu logic
//! dựng structured data — handler chỉ gọi và inject vào template.
//!
//! Quan trọng: tất cả output JSON-LD phải qua `utils::json_ld_safe()`
//! trước khi wrap vào `<script type="application/ld+json">` để chống
//! stored XSS qua `</script>` breakout (serde_json mặc định không
//! escape `<` `>` `&`).

use crate::models::category::Category;
use crate::models::game::{Game, GameLink};
use crate::models::user::User;
use crate::utils::json_ld_safe;

/// Dựng JSON-LD schema.org/BreadcrumbList: Trang chủ › [Thể loại] › Tên game.
///
/// Đồng bộ markup với `<nav class="breadcrumb">` trong template show.html.
/// Output đã qua `json_ld_safe` escape `</script>` breakout.
#[must_use]
pub fn build_breadcrumb_json_ld(
    base_url: &str,
    game: &Game,
    category: Option<&Category>,
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
        json_ld_safe(&serde_json::to_string_pretty(&ld).unwrap_or_default())
    )
}

/// Dựng JSON-LD schema.org/VideoGame cho trang chi tiết game.
///
/// Trả về tag `<script type="application/ld+json">...</script>` hoàn chỉnh
/// đã được escape `</script>` breakout qua `json_ld_safe`.
#[must_use]
pub fn build_game_json_ld(
    base_url: &str,
    game: &Game,
    author: &User,
    links: &[GameLink],
    tags: &[String],
    category: Option<&Category>,
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
    let obj = if let Some(obj) = root.as_object_mut() {
        obj
    } else {
        // Defense-in-depth: root được build bằng json!({...}) phía trên nên
        // luôn là object. Nếu invariant bị破, trả string rỗng thay vì panic.
        tracing::error!("build_game_json_ld: root không phải JSON object");
        return String::new();
    };
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
        "<script type=\"application/ld+json\">\n{}\n</script>",
        json_ld_safe(&pretty)
    )
}

/// Dựng JSON-LD schema.org/WebSite cho homepage (kèm SearchAction cho
/// Google sitelinks searchbox rich result).
#[must_use]
pub fn build_homepage_json_ld(base_url: &str) -> String {
    let json_ld = serde_json::json!({
        "@context": "https://schema.org",
        "@type": "WebSite",
        "name": "Louis Space",
        "url": base_url,
        "description": "Nền tảng chia sẻ game độc lập & tin tức cộng đồng Việt Nam",
        "inLanguage": "vi-VN",
        "potentialAction": {
            "@type": "SearchAction",
            "target": {
                "@type": "EntryPoint",
                "urlTemplate": format!("{}/search?q={{search_term_string}}", base_url),
            },
            "query-input": "required name=search_term_string",
        }
    });
    format!(
        "<script type=\"application/ld+json\">\n{}\n</script>",
        json_ld_safe(&serde_json::to_string_pretty(&json_ld).unwrap_or_default())
    )
}
