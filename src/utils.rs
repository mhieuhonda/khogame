use slug::slugify;

/// Sinh slug từ tiêu đề. Việc đảm bảo tính duy nhất được thực hiện ở
/// `handlers::games::create_game` (kiểm tra DB với hậu tố -2, -3...).
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
    // Markdown an toàn: escape HTML trước, sau đó áp dụng định dạng tối thiểu
    // (**bold**, *italic*, `code`, [text](https://link)) trên từng đoạn văn.
    input
        .split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .map(|p| format!("<p>{}</p>", render_markdown_line(p)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Áp dụng inline markdown lên chuỗi ĐÃ escape. Chỉ nhận link http/https.
fn render_markdown_line(input: &str) -> String {
    let s = html_escape(input);
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // Link [text](url)
        if chars[i] == '[' {
            if let Some((text, url, next)) = parse_md_link(&chars, i) {
                out.push_str(&format!(
                    r#"<a href="{}" target="_blank" rel="noopener noopener">{}</a>"#,
                    url, text
                ));
                i = next;
                continue;
            }
        }
        // Bold **text**
        if chars[i] == '*' && i + 1 < chars.len() && chars[i + 1] == '*' {
            if let Some(close) = find_seq(&chars, i + 2, &['*', '*']) {
                let inner: String = chars[i + 2..close].iter().collect();
                out.push_str("<strong>");
                out.push_str(&render_markdown_line(&inner));
                out.push_str("</strong>");
                i = close + 2;
                continue;
            }
        }
        // Italic *text*
        if chars[i] == '*' {
            if let Some(close) = find_seq(&chars, i + 1, &['*']) {
                let inner: String = chars[i + 1..close].iter().collect();
                out.push_str("<em>");
                out.push_str(&render_markdown_line(&inner));
                out.push_str("</em>");
                i = close + 1;
                continue;
            }
        }
        // Code `text`
        if chars[i] == '`' {
            if let Some(close) = find_seq(&chars, i + 1, &['`']) {
                let inner: String = chars[i + 1..close].iter().collect();
                out.push_str("<code>");
                out.push_str(&inner);
                out.push_str("</code>");
                i = close + 1;
                continue;
            }
        }
        // Xuống dòng trong đoạn
        if chars[i] == '\n' {
            out.push_str("<br>");
            i += 1;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Tìm vị trí bắt đầu của `seq` trong chars từ `from`
fn find_seq(chars: &[char], from: usize, seq: &[char]) -> Option<usize> {
    let mut i = from;
    while i + seq.len() <= chars.len() {
        if chars[i..i + seq.len()] == *seq {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Parse `[text](https://url)` bắt đầu tại `start`. Trả về (text, url, vị trí kế).
/// URL chỉ cho phép http/https; text/url phải không rỗng.
fn parse_md_link(chars: &[char], start: usize) -> Option<(String, String, usize)> {
    let mut j = start + 1;
    let mut depth = 1usize;
    while j < chars.len() {
        match chars[j] {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        j += 1;
    }
    if j >= chars.len() || chars[j] != ']' {
        return None;
    }
    let text: String = chars[start + 1..j].iter().collect();
    // tiếp theo phải là '('
    if j + 1 >= chars.len() || chars[j + 1] != '(' {
        return None;
    }
    // tìm ')' đóng — không cho lồng nhau trong URL
    let open_url = j + 1;
    let close_url = find_seq(chars, open_url + 1, &[')'])?;
    let raw_url: String = chars[open_url + 1..close_url].iter().collect();
    let trimmed = raw_url.trim();
    let lower = trimmed.to_ascii_lowercase();
    if text.trim().is_empty()
        || trimmed.is_empty()
        || !(lower.starts_with("http://") || lower.starts_with("https://"))
    {
        return None;
    }
    Some((
        text,
        trimmed.replace('"', "%22").replace(' ', "%20"),
        close_url + 1,
    ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_escapes_html() {
        let html = safe_markdown_to_html("<script>alert(1)</script>");
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn test_markdown_bold_italic_code() {
        let html = safe_markdown_to_html("**to** và *nghiêng* và `code`");
        assert!(html.contains("<strong>to</strong>"));
        assert!(html.contains("<em>nghiêng</em>"));
        assert!(html.contains("<code>code</code>"));
    }

    #[test]
    fn test_markdown_link_only_http() {
        let ok = safe_markdown_to_html("[web](https://example.com)");
        assert!(ok.contains(r#"<a href="https://example.com""#));
        // javascript: bị từ chối → không tạo thẻ <a>, chỉ còn text thường
        let bad = safe_markdown_to_html("[x](javascript:alert(1))");
        assert!(!bad.contains("<a href=\"javascript"));
        assert!(bad.contains("alert(1)")); // nội dung vẫn hiển thị dạng text
    }

    #[test]
    fn test_time_ago_now() {
        let now = chrono::Utc::now();
        assert_eq!(time_ago(now), "vừa xong");
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number_i64(999), "999");
        assert_eq!(format_number_i64(1200), "1.2K");
        assert_eq!(format_number_i64(3_400_000), "3.4M");
    }

    #[test]
    fn test_extract_youtube_id() {
        // Các định dạng URL phổ biến
        assert_eq!(
            extract_youtube_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ"),
            Some("dQw4w9WgXcQ".into())
        );
        assert_eq!(
            extract_youtube_id("https://youtu.be/abc123"),
            Some("abc123".into())
        );
        assert_eq!(
            extract_youtube_id("https://www.youtube.com/embed/dQw4w9WgXcQ"),
            Some("dQw4w9WgXcQ".into())
        );
        assert_eq!(
            extract_youtube_id("https://www.youtube.com/shorts/XYZ789_-"),
            Some("XYZ789_-".into())
        );
        // URL có query param khác đi kèm
        assert_eq!(
            extract_youtube_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=30s&feature=share"),
            Some("dQw4w9WgXcQ".into())
        );
        // URL rỗng / không hợp lệ
        assert_eq!(extract_youtube_id(""), None);
        assert_eq!(extract_youtube_id("https://example.com"), None);
        assert_eq!(
            extract_youtube_id("https://youtube.com/watch?v="),
            None // ID rỗng sau v=
        );
    }

    #[test]
    fn test_initials() {
        assert_eq!(initials("Nguyễn Văn A"), "NA");
        assert_eq!(initials("hello"), "HE");
        assert_eq!(initials(""), "?");
    }

    #[test]
    fn test_parse_date() {
        assert_eq!(
            parse_date("2026-08-24"),
            Some(chrono::NaiveDate::from_ymd_opt(2026, 8, 24).unwrap())
        );
        assert_eq!(parse_date("không phải ngày"), None);
    }

    #[test]
    fn test_sanitize_redirect() {
        // Path nội bộ tuyệt đối → giữ nguyên
        assert_eq!(sanitize_redirect("/games/foo"), "/games/foo");
        assert_eq!(sanitize_redirect("/"), "/");
        // URL tuyệt đối có scheme → từ chối (chống redirect mở)
        assert_eq!(sanitize_redirect("https://evil.com"), "/");
        assert_eq!(sanitize_redirect("//evil.com"), "/");
        // Path tương đối → từ chối
        assert_eq!(sanitize_redirect("foo"), "/");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hello…");
        // Cắt theo char (không theo byte) — ký tự tiếng Việt giữ nguyên
        assert_eq!(truncate("hà nội", 3), "hà …");
        assert_eq!(truncate("hà nội", 6), "hà nội");
    }

    #[test]
    fn test_make_unique_slug() {
        assert_eq!(make_unique_slug("Hello World", 0), "hello-world");
        assert_eq!(make_unique_slug("Hello World", 1), "hello-world-2");
        assert_eq!(make_unique_slug("Hello World", 5), "hello-world-6");
    }

    #[test]
    fn test_html_escape() {
        let s = html_escape("<script>alert('xss')</script>");
        assert!(s.contains("&lt;script&gt;"));
        assert!(s.contains("&#x27;"));
        // & phải escape trước để tránh tạo thực thể giả
        assert_eq!(html_escape("a & b"), "a &amp; b");
    }

    #[test]
    fn test_format_number_edges() {
        // Số 0
        assert_eq!(format_number_i64(0), "0");
        // Số âm
        assert_eq!(format_number_i64(-999), "-999");
        assert_eq!(format_number_i64(-1200), "-1.2K");
        // Ranh giới 999/1000
        assert_eq!(format_number_i64(999), "999");
        assert_eq!(format_number_i64(1000), "1.0K");
        // Lớn
        assert_eq!(format_number_i64(2_500_000_000), "2.5B");
    }
}
