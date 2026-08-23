use slug::slugify;

pub fn make_unique_slug(title: &str, existing_count: i64) -> String {
    let base = slugify(title);
    if existing_count == 0 {
        base
    } else {
        format!("{}-{}", base, existing_count + 1)
    }
}

pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{}…", truncated)
    }
}

pub fn time_ago(dt: chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let dur = now.signed_duration_since(dt);
    let secs = dur.num_seconds();
    if secs < 60 {
        "vừa xong".to_string()
    } else if secs < 3600 {
        format!("{} phút trước", secs / 60)
    } else if secs < 86400 {
        format!("{} giờ trước", secs / 3600)
    } else if secs < 2592000 {
        format!("{} ngày trước", secs / 86400)
    } else if secs < 31536000 {
        format!("{} tháng trước", secs / 2592000)
    } else {
        format!("{} năm trước", secs / 31536000)
    }
}

pub fn format_number(n: i32) -> String {
    format_number_i64(n as i64)
}

pub fn format_number_i64(n: i64) -> String {
    let n_abs = n.unsigned_abs();
    if n_abs < 1000 {
        n.to_string()
    } else if n_abs < 1_000_000 {
        format!("{:.1}K", n as f64 / 1000.0)
    } else if n_abs < 1_000_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    }
}

pub fn safe_markdown_to_html(input: &str) -> String {
    // Very basic safe markdown: escape HTML, then re-apply minimal formatting
    let escaped = html_escape(input);
    // Apply paragraphs
    escaped
        .split("\n\n")
        .map(|p| {
            if p.trim().is_empty() {
                String::new()
            } else {
                format!("<p>{}</p>", p.replace('\n', "<br>"))
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

pub fn extract_youtube_id(url: &str) -> Option<String> {
    if url.is_empty() {
        return None;
    }
    // Various YouTube URL formats
    let patterns = [
        "youtube.com/watch?v=",
        "youtu.be/",
        "youtube.com/embed/",
        "youtube.com/shorts/",
    ];
    for pat in patterns {
        if let Some(idx) = url.find(pat) {
            let rest = &url[idx + pat.len()..];
            let id: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                .collect();
            if !id.is_empty() {
                return Some(id);
            }
        }
    }
    None
}

pub fn initials(name: &str) -> String {
    let parts: Vec<&str> = name.split_whitespace().collect();
    if parts.is_empty() {
        return "?".to_string();
    }
    if parts.len() == 1 {
        return parts[0].chars().take(2).collect::<String>().to_uppercase();
    }
    let first = parts[0].chars().next().unwrap_or('?');
    let last = parts[parts.len() - 1].chars().next().unwrap_or('?');
    format!("{}{}", first, last).to_uppercase()
}

pub fn parse_date(s: &str) -> Option<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

/// Strip a query string and trailing slashes for safe redirects
pub fn sanitize_redirect(s: &str) -> String {
    if s.starts_with('/') && !s.starts_with("//") {
        s.to_string()
    } else {
        "/".to_string()
    }
}
