#[must_use]
pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{truncated}…")
    }
}

#[must_use]
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
    } else if secs < 2_592_000 {
        format!("{} ngày trước", secs / 86400)
    } else if secs < 31_536_000 {
        format!("{} tháng trước", secs / 2_592_000)
    } else {
        format!("{} năm trước", secs / 31_536_000)
    }
}

#[must_use]
pub fn format_number(n: i32) -> String {
    format_number_i64(i64::from(n))
}

#[must_use]
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

#[must_use]
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
                    r#"<a href="{url}" target="_blank" rel="noopener noopener">{text}</a>"#
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

/// Escape ký tự đặc biệt XML 1.0 cho nội dung text + giá trị attribute.
///
/// & phải escape trước để tránh tạo thực thể giả (giống `html_escape`).
/// Dùng cho sitemap.xml / RSS / `OpenSearch` XML dựng bằng format!.
#[must_use]
pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[must_use]
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

#[must_use]
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

#[must_use]
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
    format!("{first}{last}").to_uppercase()
}

#[must_use]
pub fn parse_date(s: &str) -> Option<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

/// Strip a query string and trailing slashes for safe redirects
#[must_use]
pub fn sanitize_redirect(s: &str) -> String {
    // Path nội bộ tuyệt đối, KHÔNG có control char (chống header
    // injection qua Location: \r\nSet-Cookie:...), và không bắt đầu
    // bằng `//` (chống protocol-relative redirect mở ra domain khác).
    if s.starts_with('/')
        && !s.starts_with("//")
        && !s.bytes().any(|b| b.is_ascii_control())
    {
        s.to_string()
    } else {
        "/".to_string()
    }
}

/// Escape ký tự wildcard của ILIKE/LIKE (% _ \) để tìm kiếm THEO CHUẨI
/// ký tự. ILIKE không escape tự động: tìm "100%" tạo pattern "%100%%"
/// match sai mọi tiêu đề có "100". Sau khi escape cần thêm ESCAPE '\\'
/// vào câu ILIKE (mặc định PostgreSQL hiểu ESCAPE '\\').
#[must_use]
pub fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c == '%' || c == '_' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Validate một URL là http(s) — chống <javascript:/data:/file>: scheme
/// nguy hiểm khi dùng URL làm href hoặc src trong HTML. Trả về true nếu
/// URL rỗng (không bắt buộc) hoặc là http(s)://.
#[must_use]
pub fn is_safe_url(url: &str) -> bool {
    if url.is_empty() {
        return true;
    }
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
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
        // Control char (CR/LF) → từ chối (chống header injection)
        assert_eq!(sanitize_redirect("/games\r\nSet-Cookie: bad=1"), "/");
        assert_eq!(sanitize_redirect("/\tfoo"), "/");
        // Path có null byte → từ chối
        assert_eq!(sanitize_redirect("/games\0"), "/");
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
    fn test_initials_unicode() {
        // Tiếng Việt có dấu: lấy ký tự ĐẦU của chữ — char boundary đúng
        assert_eq!(initials("Trần Anh Dũng"), "TD");
        assert_eq!(initials("Đức"), "ĐỨ"); // 1 từ → 2 ký tự đầu uppercase (Đư→ĐỨ)
                                           // Ký tự 1 byte + whitespace thừa
        assert_eq!(initials("  a   b  "), "AB");
        // Emoji như 1 ký tự
        assert_eq!(initials("🎮 người"), "🎮N");
        // Chỉ whitespace → fallback "?"
        assert_eq!(initials("   "), "?");
        // Ký tự hoa đã giữ nguyên
        assert_eq!(initials("AB CD"), "AC");
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

    #[test]
    fn test_is_safe_url() {
        // URL rỗng → an toàn (không bắt buộc)
        assert!(is_safe_url(""));
        // http(s) → an toàn
        assert!(is_safe_url("http://example.com/img.png"));
        assert!(is_safe_url("https://example.com/img.png"));
        assert!(is_safe_url("HTTPS://EXAMPLE.COM/X")); // case-insensitive
                                                       // Scheme nguy hiểm → không an toàn
        assert!(!is_safe_url("javascript:alert(1)"));
        assert!(!is_safe_url("data:text/html,<script>alert(1)</script>"));
        assert!(!is_safe_url("file:///etc/passwd"));
        assert!(!is_safe_url("vbscript:msgbox"));
        assert!(!is_safe_url("//evil.com/x")); // protocol-relative
        assert!(!is_safe_url("javascript:")); // rỗng sau scheme
    }

    #[test]
    fn test_time_ago_future() {
        // Thời điểm trong tương lai (đồng hồ lệch) → vẫn trả "vừa xong"
        // thay vì giá trị âm kỳ quặc.
        let future = chrono::Utc::now() + chrono::Duration::seconds(60);
        assert_eq!(time_ago(future), "vừa xong");
    }

    #[test]
    fn test_time_ago_past() {
        let now = chrono::Utc::now();
        assert_eq!(time_ago(now), "vừa xong");
        // 2 phút trước
        let two_mins = now - chrono::Duration::minutes(2);
        let s = time_ago(two_mins);
        assert!(s.contains("phút trước"), "got: {s}");
        // 3 giờ trước
        let three_hours = now - chrono::Duration::hours(3);
        let s = time_ago(three_hours);
        assert!(s.contains("giờ trước"), "got: {s}");
        // 5 ngày trước
        let five_days = now - chrono::Duration::days(5);
        let s = time_ago(five_days);
        assert!(s.contains("ngày trước"), "got: {s}");
        // 2 tháng trước (61 ngày — vượt ngưỡng 30 ngày)
        let two_months = now - chrono::Duration::days(61);
        let s = time_ago(two_months);
        assert!(s.contains("tháng trước"), "got: {s}");
        // 14 tháng trước (426 ngày — vượt ngưỡng 365 ngày → năm)
        let fourteen_months = now - chrono::Duration::days(426);
        let s = time_ago(fourteen_months);
        assert!(s.contains("năm trước"), "got: {s}");
        // Ranh giới chính xác 30 ngày → "1 tháng trước" (không phải 30 ngày)
        let thirty_days = now - chrono::Duration::days(30);
        assert_eq!(time_ago(thirty_days), "1 tháng trước");
        // 365 ngày → "1 năm trước"
        let one_year = now - chrono::Duration::days(365);
        assert_eq!(time_ago(one_year), "1 năm trước");
    }

    #[test]
    fn test_escape_like() {
        // Wildcard bị escape → khớp literal
        assert_eq!(escape_like("100%"), "100\\%");
        assert_eq!(escape_like("a_b"), "a\\_b");
        // Backslash cũng phải escape (tránh thành escape char trái ý)
        assert_eq!(escape_like("a\\b"), "a\\\\b");
        // Chuỗi thường không đổi
        assert_eq!(escape_like("game hay"), "game hay");
        // Tiếng Việt giữ nguyên
        assert_eq!(escape_like("Hà Nội"), "Hà Nội");
    }

    #[test]
    fn test_html_escape_quotes() {
        // Đảm bảo escape cả dấu nháy đơn và kép để chống XSS qua attribute
        let s = html_escape(r#"<a href="x" title='y'>"#);
        assert!(s.contains("&lt;a href=&quot;x&quot;"));
        assert!(s.contains("title=&#x27;y&#x27;"));
    }

    #[test]
    fn test_html_escape_vietnamese_preserved() {
        // Escape không đụng ký tự unicode — title game tiếng Việt render đúng
        let s = html_escape("Trần Văn Ưng — game «hay» 🎮");
        assert_eq!(s, "Trần Văn Ưng — game «hay» 🎮");
        // Ký tự gần giống HTML nhưng không phải tag vẫn giữ nguyên
        let s = html_escape("a < b và c > d");
        assert_eq!(s, "a &lt; b và c &gt; d");
    }

    #[test]
    fn test_xml_escape() {
        // & trước các ký tự khác — tránh tạo thực thể giả
        assert_eq!(xml_escape("a & b"), "a &amp; b");
        assert_eq!(xml_escape("<tag>"), "&lt;tag&gt;");
        assert_eq!(xml_escape(r#"a "b" 'c'"#), "a &quot;b&quot; &apos;c&apos;");
        // Chuỗi thường không đổi
        assert_eq!(xml_escape("game-hay-2026"), "game-hay-2026");
        assert_eq!(xml_escape("hà-nội"), "hà-nội");
        // & đã escape không bị escape kép
        assert_eq!(xml_escape("a&amp;b"), "a&amp;amp;b");
    }
}

#[cfg(test)]
mod tests_markdown_edge {
    use super::*;

    /// Bold lồng trong italic và ngược lại — không crash, giữ tag cân.
    #[test]
    fn test_markdown_nested_formatting() {
        let html = safe_markdown_to_html("**bold *italic* bold**");
        assert!(html.contains("<strong>"));
        assert!(html.contains("<em>italic</em>"));
        // Tag phải cân bằng (đóng đủ số lần mở)
        let opens = html.matches("<strong>").count();
        let closes = html.matches("</strong>").count();
        assert_eq!(opens, closes);
    }

    /// Code span chứa dấu sao — ** không tạo bold bên trong `...`
    #[test]
    fn test_markdown_code_with_asterisks() {
        let html = safe_markdown_to_html("`a ** b`");
        assert!(html.contains("<code>a ** b</code>"));
    }

    /// Đoạn nhiều dòng (single \n) → <br>, đoạn đôi dòng → 2 thẻ <p>
    #[test]
    fn test_markdown_paragraph_split() {
        let html = safe_markdown_to_html("đoạn một\n\nđoạn hai");
        assert_eq!(html.matches("<p>").count(), 2);
        let br = safe_markdown_to_html("dòng 1\ndòng 2");
        assert!(br.contains("dòng 1<br>dòng 2"));
    }

    /// URL chứa ký tự cần escape (khoảng trắng, nháy) trong link markdown
    #[test]
    fn test_markdown_link_url_escaping() {
        let html = safe_markdown_to_html("[x](https://ex.com/a b\"c)");
        // Space được encode %20, quote encode %22 → href an toàn
        assert!(!html.contains("href=\"https://ex.com/a b"));
        assert!(html.contains("%20") || html.contains("%22"));
    }

    /// Chuỗi rỗng / chỉ whitespace → không tạo thẻ p rỗng
    #[test]
    fn test_markdown_empty_input() {
        assert_eq!(safe_markdown_to_html(""), "");
        assert_eq!(safe_markdown_to_html("   \n\n  \n "), "");
    }

    /// Markdown không tự đóng các thẻ hở trong input — input đã escape trước
    #[test]
    fn test_markdown_escapes_before_format() {
        // <b> user-gõ không trở thành thẻ thật trong output
        let html = safe_markdown_to_html("<b>not real</b>");
        assert!(!html.contains("<b>"));
        assert!(html.contains("&lt;b&gt;"));
    }
}
