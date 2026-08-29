// ============================================================
// v2.9.2 — NGÀY KINH DOANH THEO GIỜ VIỆT NAM (UTC+7, không DST)
// ------------------------------------------------------------
// Trước đây codebase dùng LẪN LỘN 3 chuẩn "hôm nay":
//   1. `CURRENT_DATE` trong SQL (phụ thuộc timezone của Postgres server —
//      compose set TZ=Asia/Ho_Chi_Minh nhưng chỉ có hiệu lực nếu volume DB
//      được initdb SAU khi set TZ; volume cũ có thể vẫn UTC),
//   2. `date_trunc('day', NOW() AT TIME ZONE 'UTC')` (chuẩn UTC),
//   3. `Utc::now().date_naive()` trong Rust (chuẩn UTC).
// Hệ quả: trong khung 17:00–24:00 UTC (= 00:00–07:00 giờ VN) hai bên lệch
// 1 ngày — streak điểm danh "giữ" nhầm khi đã đứt, XP cap ngày reset sai
// giờ, "X tin hôm nay" của chat đếm từ 07:00 sáng VN.
// Chuẩn hoá: MỌI mốc "hôm nay" dùng giờ VN tường minh qua named zone
// 'Asia/Ho_Chi_Minh' trong SQL và offset +7h cố định trong Rust — KHÔNG
// còn phụ thuộc cấu hình timezone của server Postgres.
// ============================================================

/// Ngày "hôm nay" (DATE) theo giờ VN, tính trong SQL — dùng nhét vào query
/// runtime (không phải input user, an toàn cho `format!`).
pub const SQL_TODAY_VN: &str = "(NOW() AT TIME ZONE 'Asia/Ho_Chi_Minh')::date";

/// Mốc `timestamptz` "đầu giờ sáng nay (00:00 VN)" trong SQL — dùng so với
/// cột `created_at`/`earned_at` kiểu timestamptz.
pub const SQL_TODAY_START_VN: &str =
    "date_trunc('day', NOW() AT TIME ZONE 'Asia/Ho_Chi_Minh') AT TIME ZONE 'Asia/Ho_Chi_Minh'";

/// Ngày "hôm nay" theo giờ VN trong Rust (UTC+7 cố định — Việt Nam không
/// áp dụng daylight saving nên cộng offset vào timestamp UTC là chính xác).
#[must_use]
pub fn today_vn() -> chrono::NaiveDate {
    (chrono::Utc::now() + chrono::Duration::hours(7)).date_naive()
}

/// So sánh 2 slice byte constant-time (chống timing attack).
///
/// v2.9.2: chuyển thành public utility (trước đây là private trong
/// handlers/ai_agent.rs) để OAuth callback (`handlers/auth.rs`) so sánh
/// state cookie bằng constant-time như AI token — nhất quán toàn codebase.
#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

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
    // v2.2.0 — Delegate sang markdown engine mới (comrak + syntect).
    // Engine cũ (inline-only, không support headings/lists/tables/code blocks)
    // bị thay thế hoàn toàn. Hàm này giữ lại làm backward-compat shim —
    // các template vẫn gọi `|html` filter trong askama.
    crate::services::markdown::render(input)
}

// (v2.2.0) Markdown rendering được chuyển sang `services::markdown::render`
// (comrak + syntect). Code inline cũ ở đây đã được xoá. Hàm shim
// `safe_markdown_to_html` ở trên gọi sang module mới.

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

/// Escape JSON cho nhúng an toàn vào trong `<script type="application/ld+json">`.
///
/// `serde_json` mặc định KHÔNG escape `<`, `>`, `&` (xem bảng `ESCAPE` trong
/// `serde_json/src/ser.rs`). Khi JSON chứa chuỗi do user kiểm soát (game.title,
/// news.title, author.display_name...) và được render raw qua `{{ json_ld|safe }}`
/// trong Askama, attacker có thể dùng `</script><script>alert(1)</script>` để
/// đóng script element sớm và chèn JS tuỳ tiện — stored XSS trong session của
/// mọi visitor, kể cả admin.
///
/// Fix: thay `<` bằng `\u003c` (JSON backslash escape hợp lệ trong chuỗi JSON).
/// `>` và `&` cũng escape luôn để phòng CSP `unsafe-inline` không bật.
#[must_use]
pub fn json_ld_safe(json_str: &str) -> String {
    json_str
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
        // Chống HTML comment `<!--` / `-->` break-out (IE legacy nhưng browser
        // vẫn parse: `<!--` trong <script> bắt đầu comment, `-->` kết thúc).
        .replace("<!--", "\\u003c!--")
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
    //
    // Cũng chặn `/\` ở đầu: nhiều browser (Chrome/Edge/IE) coi `\` như
    // path separator tương đương `/`, nên `/\evil.com` được interpret
    // thành `//evil.com` → protocol-relative URL → redirect sang domain
    // khác (open redirect bypass). Spec WHATWG URL Parser chính thức
    // normalise `\` thành `/` trong special-scheme URL.
    if s.starts_with('/')
        && !s.starts_with("//")
        && !s.starts_with("/\\")
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
///
/// Cũng từ chối URL có byte điều khiển (CR/LF/TAB/NUL) để chống header
/// injection khi URL được phát vào Location/X-Redirect header.
#[must_use]
pub fn is_safe_url(url: &str) -> bool {
    if url.is_empty() {
        return true;
    }
    // Chống header injection: URL có CR/LF có thể bẻ gãy response header
    // nếu server dùng URL làm giá trị Location/X-Redirect.
    if url.bytes().any(|b| b.is_ascii_control()) {
        return false;
    }
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// Validate một URL là ảnh hợp lệ: chấp nhận (1) http(s):// URL remote
/// HOẶC (2) `/uploads/...` URL do server tự sinh khi user upload ảnh.
/// Dùng cho avatar_url, cover_image, screenshots — các field ảnh cho
/// phép user upload hoặc điền URL remote.
///
/// Trả về `true` nếu URL rỗng (không bắt buộc). Trả về `false` cho mọi
/// scheme khác (javascript:, data:, file:, vbscript:).
///
/// Chặn control bytes (CR/LF/TAB/NUL) — chống header injection khi
/// URL được ghép vào Location header.
#[must_use]
pub fn is_safe_image_url(url: &str) -> bool {
    if url.is_empty() {
        return true;
    }
    if url.bytes().any(|b| b.is_ascii_control()) {
        return false;
    }
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || crate::services::storage::is_upload_url(url)
}

/// v2.9.1 FIX — chuẩn hoá chuỗi Unicode về dạng NFC (precomposed).
///
/// Google OAuth ĐÔI KHI trả `name` ở dạng NFD (decomposed): "Hiếu" thành
/// `H` + `i` + `e` + U+0302 + `u` thay vì precomposed `Hiếu`. Hiển thị
/// NFD trên web gây 2 vấn đề thật:
/// 1. Dấu combining (U+0302, U+031B horn, U+0323 dot-below...) rơi ngoài
///    `unicode-range` của @font-face subset Inter vietnamese → browser
///    fallback sang font hệ thống CHO RIÊNG dấu → dấu lệch nét, lệch vị
///    trí so với chữ (bug "tên hiển thị bị lỗi, lệch" trên desktop).
/// 2. NFD chiếm nhiều bytes hơn NFC khi search/so sánh chuỗi ("Hiếu" NFD
///    ≠ "Hiếu" NFC khi dùng = trong SQL/JS).
///
/// NFC là dạng chuẩn của text tiếng Việt (Unicode Normalization Form C).
/// Áp cho mọi điểm vào của display_name: Google OAuth, edit profile, AI agent.
#[must_use]
pub fn normalize_nfc(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfc().collect()
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
        assert!(ok.contains(r#"href="https://example.com""#));
        // javascript: bị từ chối → comrak strip URL, harden_links thay bằng #.
        // Link text "x" vẫn hiển thị nhưng URL nguy hiểm không xuất hiện trong href.
        let bad = safe_markdown_to_html("[x](javascript:alert(1))");
        assert!(!bad.contains("javascript:alert"));
        assert!(bad.contains("href=\"#\"") || bad.contains("href=\"\""));
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

    /// REGRESSION v2.9.1 — NFC normalize: tên Google NFD ("Hiếu" decomposed)
    /// phải về precomposed để font subset vietnamese render đúng dấu,
    /// không bị lệch glyphs trên desktop.
    #[test]
    fn test_normalize_nfc_vietnamese() {
        // 'ê' decomposed = 'e' + U+0302 (combining circumflex)
        assert_eq!(normalize_nfc("Hie\u{0302}u"), "Hiêu"); // H-i-e-◌̂-u → Hiêu
                                                           // 'ế' decomposed = 'e' + U+0302 (circumflex) + U+0301 (acute) —
                                                           // ĐÚNG THỨ TỰ dấu này mới là NFD của "Hiếu" (Google trả dạng này).
        assert_eq!(normalize_nfc("Hie\u{0302}\u{0301}u"), "Hiếu");
        assert_eq!(normalize_nfc("Hiếu").chars().count(), 4);
        // Chuỗi đã NFC → giữ nguyên
        assert_eq!(normalize_nfc("Hiếu"), "Hiếu");
        // ASCII không đổi
        assert_eq!(normalize_nfc("Louis Space"), "Louis Space");
        // "ưng" với horn decomposed: ư = u + U+031B
        assert_eq!(normalize_nfc("Hu\u{031B}ng"), "Hưng");
        // "ỡ" full decomposed: ỡ = O + U+031B (horn) + U+0303 (tilde) —
        // tổ hợp 2 dấu phải compose thành U+1EE0 ("Ỡ").
        assert_eq!(normalize_nfc("O\u{031B}\u{0303}ng"), "Ỡng");
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
        // Backslash bypass: `/\evil.com` browser sẽ coi `\` như `/`
        // → `//evil.com` protocol-relative URL → open redirect. Chặn.
        assert_eq!(sanitize_redirect("/\\evil.com"), "/");
        assert_eq!(sanitize_redirect("/\\"), "/");
        // Backslash KHÔNG ở đầu path thì an toàn (giữ nguyên — browser
        // interpret `/foo\bar` thành `/foo/bar` cùng origin).
        assert_eq!(sanitize_redirect("/games/foo\\bar"), "/games/foo\\bar");
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
                                              // CR/LF trong URL → không an toàn (chống header injection)
        assert!(!is_safe_url("https://evil.com/\r\nSet-Cookie: bad=1"));
        assert!(!is_safe_url("https://evil.com/\n"));
        assert!(!is_safe_url("https://evil.com/\tfoo"));
        assert!(!is_safe_url("https://evil.com/\0"));
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

    #[test]
    fn test_json_ld_safe_escapes_script_breakout() {
        // Payload `</script><script>alert(1)</script>` phải bị vô hiệu hoá
        // — mọi `<` thay bằng `\u003c`, mọi `>` thay bằng `\u003e`, mọi `&`
        // thay bằng `\u0026`. Browser JSON parser decode `\u003c` → `<`
        // trong GIÁ TRỊ chuỗi (không break script element).
        let payload = "</script><script>alert(document.cookie)</script>";
        let safe = json_ld_safe(payload);
        assert!(
            !safe.contains("</script>"),
            "safe JSON-LD không được chứa `</script>` literal"
        );
        // Quay lại chuỗi gốc khi JSON parse (decode \u003c → <)
        let decoded = serde_json::from_str::<String>(&format!("\"{safe}\""))
            .expect("json_ld_safe phải trả về JSON string hợp lệ");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn test_json_ld_safe_preserves_normal_json() {
        // JSON-LD bình thường không bị escape oan
        let s = r#"{"@type":"WebSite","name":"Louis Space"}"#;
        assert_eq!(json_ld_safe(s), s);
    }

    #[test]
    fn test_json_ld_safe_html_comment_breakout() {
        // `<!--` trong <script> bắt đầu HTML comment → có thể break-out
        // qua `-->` ở nhiều trình duyệt cũ. Đảm bảo bị escape.
        let payload = "<!--</script>-->";
        let safe = json_ld_safe(payload);
        assert!(!safe.contains("<!--"));
        assert!(!safe.contains("</script>"));
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

    /// Đoạn nhiều dòng (single \n) → soft break (GFM spec), đoạn đôi dòng → 2 thẻ <p>.
    /// v2.2.0: comrak mặc định hardbreaks=false nên single \n là space (không <br>).
    #[test]
    fn test_markdown_paragraph_split() {
        let html = safe_markdown_to_html("đoạn một\n\nđoạn hai");
        assert!(html.matches("<p>").count() >= 2);
        let soft = safe_markdown_to_html("dòng 1\ndòng 2");
        // GFM: single \n → trong cùng <p>, không tạo <br>
        assert!(soft.contains("dòng 1") && soft.contains("dòng 2"));
    }

    /// URL chứa ký tự cần escape trong link markdown — comrak tự URL-encode.
    #[test]
    fn test_markdown_link_url_escaping() {
        let html = safe_markdown_to_html("[x](https://ex.com/a%20b)");
        // Comrak tự xử lý URL — đảm bảo href không chứa raw space
        assert!(!html.contains("href=\"https://ex.com/a b"));
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
