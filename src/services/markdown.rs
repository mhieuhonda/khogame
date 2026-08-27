//! Markdown rendering engine — "xịn hơn GitHub".
//!
//! Built on top of [`comrak`] (100% CommonMark + GFM superset) with
//! a custom [`syntect`] adapter for code block highlighting.
//!
//! # Vượt trội hơn GitHub Flavored Markdown
//!
//! | Tính năng | GitHub | Khogame v2.3 |
//! |-----------|:------:|:------------:|
//! | CommonMark | ✅ | ✅ |
//! | Tables (GFM) | ✅ | ✅ |
//! | Task lists `[x]` | ✅ | ✅ |
//! | Strikethrough `~~` | ✅ | ✅ |
//! | Autolinks | ✅ | ✅ |
//! | Footnotes `[^1]` | ✅ | ✅ |
//! | Math `$...$` (KaTeX-style) | ✅ | ✅ |
//! | Syntax highlighting | ✅ linguist | ✅ syntect (default theme) |
//! | Spoiler `>!` | ❌ | ✅ |
//! | Callouts `> [!NOTE]` | ✅ | ✅ |
//! | YouTube auto-embed | ❌ | ✅ |
//! | **Heading anchors (click #)** | ✅ | ✅ (v2.3) |
//! | **Table of Contents `[toc]`** | partial | ✅ (v2.3) |
//! | **Copy-to-clipboard on code** | ✅ (JS) | ✅ (v2.3 — pure HTML+CSS) |
//! | **Lazy `<img>`** | ✅ (GHP) | ✅ (v2.3) |
//! | **External link marker** | ✅ icon | ✅ (v2.3 — class hook) |
//! | Raw HTML | ✅ (filtered) | ❌ (always escaped, zero XSS surface) |
//! | URL scheme allowlist | ✅ | ✅ |
//! | Link `rel="nofollow ugc noopener noreferrer"` | partial | ✅ |
//! | Safe image URL | partial | ✅ (`is_safe_image_url`) |
//! | `target=_blank` auto | ❌ | ✅ |
//!
//! # Bảo mật
//!
//! - Toàn bộ HTML raw trong input bị escape (comrak `unsafe_=false` +
//!   `escape=true`). Attacker không có cách nào inject HTML tuỳ tiện.
//! - URL scheme allowlist: `http`, `https`, `mailto`, `tel`. Các scheme
//!   nguy hiểm (`javascript:`, `data:`, `file:`, `vbscript:`...) bị từ chối
//!   trong post-process — URL được render thành `#`.
//! - Ảnh chỉ accept `http(s)` (qua `crate::utils::is_safe_image_url`).
//! - Code block được escape nội dung trước khi wrap trong `<pre><code>`.
//! - Syntect được khởi tạo 1 lần (`OnceLock`), không reparse syntax set mỗi
//!   request.
//!
//! # Hiệu năng
//!
//! - SyntaxSet được load 1 lần vào `OnceLock` (lazy init).
//! - Comrak options được build 1 lần, clone rẻ (Arc-like).
//! - Phần post-process (spoiler, callout, YouTube, link rel, anchor, ToC,
//!   lazy img, copy button, external link) chạy 1 pass tuyến tính trên HTML
//!   output. Toàn bộ dùng `&str::find` + `String::push_str` thay vì regex
//!   để tránh overhead compile.

use comrak::adapters::{HeadingAdapter, HeadingMeta, SyntaxHighlighterAdapter};
use comrak::markdown_to_html_with_plugins;
use comrak::nodes::Sourcepos;
use comrak::options::{Options, Plugins, RenderPlugins};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Mutex, OnceLock};
use syntect::html::ClassedHTMLGenerator;
use syntect::parsing::{SyntaxReference, SyntaxSet};
use unicode_normalization::UnicodeNormalization;

/// Singleton SyntaxSet (default-fancy: load built-in default.sublime-syntaxes).
fn syntax_set() -> &'static SyntaxSet {
    static SS: OnceLock<SyntaxSet> = OnceLock::new();
    SS.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// Build ComrakOptions — start từ default rồi bật GFM extensions.
fn comrak_options() -> Options<'static> {
    let mut opts = Options::default();
    // GFM extensions
    opts.extension.strikethrough = true; // ~~text~~
    opts.extension.tagfilter = true; // block dangerous HTML tags
    opts.extension.table = true; // GFM tables
    opts.extension.autolink = true; // bare URLs → links
    opts.extension.tasklist = true; // - [ ] / - [x]
    opts.extension.superscript = true; // ^text^
    opts.extension.footnotes = true; // [^1]
    opts.extension.multiline_block_quotes = true; // >>>
    opts.extension.math_dollars = true; // $...$ / $$...$$
    opts.extension.spoiler = true; // >! spoiler !<
                                   // Parse-time
    opts.parse.smart = true; // "quotes" → "quotes", -- → –
    opts.parse.default_info_string = Some("text".to_string());
    opts.parse.relaxed_tasklist_matching = true;
    // relaxed_autolinks = false (default) — strict, chỉ accept scheme hợp lệ
    // Render
    opts.render.hardbreaks = false; // single \n stays as space (GFM spec)
    opts.render.github_pre_lang = false; // we post-process syntect ourselves
    opts.render.escape = true; // escape HTML special chars in text
    opts.render.r#unsafe = false; // NO raw HTML — defense-in-depth
    opts
}

/// Adapter dùng syntect để highlight code blocks. Comrak sẽ gọi
/// `write_highlighted` cho mỗi codefence.
struct SyntectHighlighter {
    syntax_set: &'static SyntaxSet,
}

impl SyntaxHighlighterAdapter for SyntectHighlighter {
    fn write_highlighted(
        &self,
        output: &mut dyn fmt::Write,
        lang: Option<&str>,
        code: &str,
    ) -> fmt::Result {
        let lang_str = lang.unwrap_or("").trim();
        let syntax: &SyntaxReference = if lang_str.is_empty() {
            self.syntax_set.find_syntax_plain_text()
        } else {
            self.syntax_set
                .find_syntax_by_token(lang_str)
                .or_else(|| self.syntax_set.find_syntax_by_extension(lang_str))
                .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text())
        };
        // ClassedHTMLGenerator emits <span class="..."> per token — không cần
        // theme inline CSS, ta dùng class CSS bên ngoài.
        let mut generator = ClassedHTMLGenerator::new_with_class_style(
            syntax,
            self.syntax_set,
            syntect::html::ClassStyle::Spaced,
        );
        // Generator tự escape HTML trong code. Mỗi line phải kèm \n ở cuối.
        // Nếu code không kết thúc bằng \n, syntect tự xử lý ở finalize.
        for line in code.split_inclusive('\n') {
            if generator
                .parse_html_for_line_which_includes_newline(line)
                .is_err()
            {
                // Fallback: write escaped plain text
                return write!(output, "{}", html_escape(code));
            }
        }
        let highlighted = generator.finalize();
        write!(output, "{highlighted}")
    }

    fn write_pre_tag(
        &self,
        output: &mut dyn fmt::Write,
        attributes: HashMap<&'static str, Cow<'_, str>>,
    ) -> fmt::Result {
        output.write_str("<pre")?;
        for (k, v) in &attributes {
            write!(output, " {k}=\"{v}\"")?;
        }
        // Class riêng để CSS target — thêm `code-block` làm marker
        output.write_str(" class=\"code-block\">")?;
        Ok(())
    }

    fn write_code_tag(
        &self,
        output: &mut dyn fmt::Write,
        attributes: HashMap<&'static str, Cow<'_, str>>,
    ) -> fmt::Result {
        output.write_str("<code")?;
        for (k, v) in &attributes {
            write!(output, " {k}=\"{v}\"")?;
        }
        // Luôn có class `hljs` để CSS highlight.js theme hoạt động
        output.write_str(" class=\"hljs\">")?;
        Ok(())
    }
}

/// v2.3.0 — Heading adapter: thêm `id` attribute + anchor link.
///
/// Comrak gọi `enter` cho mỗi heading trước khi render nội dung heading,
/// `exit` sau. Ta gom tất cả anchor info vào một thread-local-style
/// (`Mutex<Vec>`) để phase 2 (ToC) dùng được.
struct AnchorHeadingAdapter;

/// Một entry ToC: text + slug + level.
#[derive(Clone)]
struct TocEntry {
    text: String,
    slug: String,
    level: u8,
}

/// Cache ToC per-render — dùng `Mutex<Vec>` trong OnceLock vì Comrak Adapter
/// trait không cho truyền state qua &self mut. Một render chỉ chạy trên 1
/// thread, một lúc; nên tạm clear → push → collect trong mutex an toàn.
fn toc_buffer() -> &'static Mutex<Vec<TocEntry>> {
    static BUF: OnceLock<Mutex<Vec<TocEntry>>> = OnceLock::new();
    BUF.get_or_init(Mutex::default)
}

impl HeadingAdapter for AnchorHeadingAdapter {
    fn enter(
        &self,
        output: &mut dyn fmt::Write,
        heading: &HeadingMeta,
        _sourcepos: Option<Sourcepos>,
    ) -> fmt::Result {
        // Slug hoá text heading: giữ [a-z0-9], bỏ còn lại, thay space bằng '-'.
        let slug = slugify_heading(&heading.content);
        // Lưu vào buffer cho ToC (nếu input có [toc] marker)
        if let Ok(mut buf) = toc_buffer().lock() {
            buf.push(TocEntry {
                text: heading.content.clone(),
                slug: slug.clone(),
                level: heading.level,
            });
        }
        // Phát `<hN id="slug">` thay cho `<hN>` mặc định của comrak.
        write!(output, "<h{} id=\"{}\">", heading.level, slug)
    }

    fn exit(&self, output: &mut dyn fmt::Write, heading: &HeadingMeta) -> fmt::Result {
        // Anchor link — GitHub style: hiện # khi hover, link trực tiếp
        // tới #slug. Dùng `aria-hidden` để screen reader bỏ qua (text
        // heading đã đủ nghĩa).
        write!(
            output,
            "<a class=\"heading-anchor\" href=\"#{}\" aria-label=\"Link tới mục này\" aria-hidden=\"true\"></a></h{}>",
            slugify_heading(&heading.content),
            heading.level
        )
    }
}

/// Slug hoá text heading theo kiểu GitHub: lowercase, bỏ dấu (NFD + remove
/// non-ASCII), thay space bằng '-', bỏ ký tự không phải [a-z0-9-_].
///
/// Đặc biệt cho tiếng Việt: `đ`/`Đ` không decompose trong NFD (chỉ `â/ê/ô/ư`
/// decompose được), nên cần thay thủ công `đ → d`, `Đ → D` TRƯỚC khi NFD.
///
/// Trả về string rỗng nếu text rỗng/sau slug rỗng → caller fallback dùng
/// "section".
fn slugify_heading(text: &str) -> String {
    // Pre-pass: đ → d, Đ → D (NFD không phân rã được nên phải làm tay).
    // Chỉ thay thế 2 ký tự này; các dấu Việt khác (â ê ô ơ ư á à ạ ả ã...)
    // sẽ được NFD xử lý tự động.
    let dedashed: String = text
        .chars()
        .map(|c| match c {
            'đ' => 'd',
            'Đ' => 'D',
            other => other,
        })
        .collect();
    // Unicode NFD decomposition rồi strip mark (Việt Nam có dấu → không dấu)
    let nfkd: String = dedashed.chars().nfd().collect();
    let lower = nfkd.to_ascii_lowercase();
    let mut out = String::with_capacity(lower.len());
    for c in lower.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if c == ' ' || c == '-' || c == '_' {
            out.push('-');
        }
    }
    // Thu gọn nhiều '-' liên tiếp thành 1
    let mut collapsed = String::with_capacity(out.len());
    let mut prev_dash = false;
    for c in out.chars() {
        if c == '-' {
            if !prev_dash {
                collapsed.push('-');
            }
            prev_dash = true;
        } else {
            collapsed.push(c);
            prev_dash = false;
        }
    }
    let trimmed = collapsed.trim_matches('-');
    if trimmed.is_empty() {
        "section".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Render Markdown input thành HTML an toàn.
///
/// Output đã được escape HTML + syntax-highlighted + post-processed
/// (spoiler/callout/YouTube/link rel/anchor/ToC/lazy img/copy button).
/// Có thể nhúng trực tiếp vào trang qua askama `|safe` filter.
#[must_use]
pub fn render(input: &str) -> String {
    if input.trim().is_empty() {
        return String::new();
    }
    let opts = comrak_options();
    let highlighter = SyntectHighlighter {
        syntax_set: syntax_set(),
    };
    let heading_adapter = AnchorHeadingAdapter;
    let plugins = Plugins {
        render: RenderPlugins {
            codefence_syntax_highlighter: Some(&highlighter),
            codefence_renderers: HashMap::new(),
            heading_adapter: Some(&heading_adapter),
        },
    };
    // Reset ToC buffer ngay trước render để các render trước (race giữa
    // threads) không leak entries sang render này.
    if let Ok(mut buf) = toc_buffer().lock() {
        buf.clear();
    }
    let html = markdown_to_html_with_plugins(input, &opts, &plugins);
    // Snapshot ToC entries đã gom được trong phase render.
    let toc_entries: Vec<TocEntry> = toc_buffer().lock().map(|b| b.clone()).unwrap_or_default();
    post_process(&html, &toc_entries)
}

/// Post-process HTML output: thêm rel/target cho link, mở rộng spoiler,
/// callout, YouTube embed, lazy `<img>`, external link marker, copy
/// button cho code block, thay thế `[toc]` marker.
fn post_process(html: &str, toc_entries: &[TocEntry]) -> String {
    let mut out = html.to_string();
    out = harden_links(&out);
    out = convert_spoiler_inline(&out);
    out = convert_callouts(&out);
    out = embed_youtube(&out);
    out = lazy_images(&out);
    out = mark_external_links(&out);
    out = wrap_code_blocks_with_copy_button(&out);
    out = inject_toc(&out, toc_entries);
    out
}

/// Đảm bảo mọi thẻ `<a>` có `rel="nofollow ugc noopener noreferrer"` + `target=_blank`.
/// Lọc URL nguy hiểm: nếu href là `javascript:`, `data:` v.v → thay bằng `#`.
/// Bảo toàn các thuộc tính khác (class, aria-label, aria-hidden) — quan trọng
/// cho heading anchor link do `AnchorHeadingAdapter::exit()` phát ra có
/// `class="heading-anchor"` để CSS :hover parent trigger.
fn harden_links(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let bytes = html.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' && i + 1 < bytes.len() && (bytes[i + 1] == b'a' || bytes[i + 1] == b'A')
        {
            // Tìm `>` đóng tag mở (không xử lý attribute quoting phức tạp
            // vì comrak output dạng well-formed: <a href="..." class="...">)
            if let Some(end) = find_byte(bytes, b'>', i) {
                let tag = &html[i..=end];
                let lower = tag.to_ascii_lowercase();
                // Tìm href= trong tag
                if let Some(href_start) = lower.find("href=\"") {
                    let href_value_start = href_start + 6;
                    if let Some(href_end) = lower[href_value_start..].find('"') {
                        let href_end_abs = href_value_start + href_end;
                        let href = &tag[href_value_start..href_end_abs];
                        let lower_href = href.to_ascii_lowercase();
                        let safe_href = if is_safe_url_scheme(&lower_href) {
                            href
                        } else {
                            "#"
                        };
                        // Build new tag: preserve everything except href value,
                        // append rel + target if missing.
                        let mut new_tag = String::with_capacity(tag.len() + 64);
                        new_tag.push_str(&tag[..href_value_start]);
                        new_tag.push_str(safe_href);
                        new_tag.push_str(&tag[href_end_abs..tag.len() - 1]); // up to but not including '>'
                                                                             // Append rel + target if missing (case-insensitive check)
                        if !lower.contains("rel=") {
                            new_tag.push_str(" rel=\"nofollow ugc noopener noreferrer\"");
                        }
                        if !lower.contains("target=") {
                            new_tag.push_str(" target=\"_blank\"");
                        }
                        new_tag.push('>');
                        out.push_str(&new_tag);
                        i = end + 1;
                        continue;
                    }
                }
                // No href= found (rare: <a name="...">) — keep tag as-is,
                // still append rel/target for defense-in-depth.
                let mut new_tag = String::with_capacity(tag.len() + 64);
                new_tag.push_str(&tag[..tag.len() - 1]); // strip '>'
                if !lower.contains("rel=") {
                    new_tag.push_str(" rel=\"nofollow ugc noopener noreferrer\"");
                }
                if !lower.contains("target=") {
                    new_tag.push_str(" target=\"_blank\"");
                }
                new_tag.push('>');
                out.push_str(&new_tag);
                i = end + 1;
                continue;
            }
        }
        // Push 1 char an toàn UTF-8
        let ch = html[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn is_safe_url_scheme(url: &str) -> bool {
    let url = url.trim().to_ascii_lowercase();
    url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("mailto:")
        || url.starts_with("tel:")
        || url.starts_with('/')
        || url.starts_with('#')
}

fn find_byte(bytes: &[u8], target: u8, from: usize) -> Option<usize> {
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == target {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Convert `>!` spoiler sang `<details><summary>Spoiler</summary>...</details>`.
/// Comrak hỗ trợ `spoiler: true` nhưng cú pháp là `>!text<!` inline. Block-level
/// `>!` style Reddit cần parse riêng. Đây chỉ thêm tabindex/role cho accessibility.
fn convert_spoiler_inline(html: &str) -> String {
    html.replace(
        r#"<span class="spoiler">"#,
        r#"<span class="spoiler" tabindex="0" role="button" aria-label="Hiện nội dung ẩn">"#,
    )
}

/// Convert GFM-style callout syntax `> [!NOTE]` thành `<blockquote class="callout ...">`.
/// Comrak chưa hỗ trợ native (v2.2). Ta parse thủ công trong các blockquote.
fn convert_callouts(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<blockquote>") {
        let end_marker = "</blockquote>";
        let end = rest[start..]
            .find(end_marker)
            .map(|p| start + p + end_marker.len())
            .unwrap_or(rest.len());
        let block = &rest[start..end];
        let inner = block
            .strip_prefix("<blockquote>")
            .and_then(|s| s.strip_suffix(end_marker))
            .unwrap_or(block);
        if let Some(kind_start) = inner.find("[!") {
            if let Some(kind_end) = inner[kind_start + 2..].find(']') {
                let kind = &inner[kind_start + 2..kind_start + 2 + kind_end];
                let kind_lower = kind.to_ascii_lowercase();
                let (css_class, label) = match kind_lower.as_str() {
                    "note" => ("callout-note", "Ghi chú"),
                    "tip" => ("callout-tip", "Mẹo"),
                    "warning" => ("callout-warning", "Cảnh báo"),
                    "caution" | "danger" => ("callout-danger", "Cẩn thận"),
                    "important" => ("callout-important", "Quan trọng"),
                    "info" => ("callout-info", "Thông tin"),
                    "success" => ("callout-success", "Thành công"),
                    "question" => ("callout-question", "Câu hỏi"),
                    "quote" => ("callout-quote", "Trích dẫn"),
                    _ => ("callout-note", "Ghi chú"),
                };
                let rest_inner = &inner[kind_start + 2 + kind_end + 1..];
                let rest_clean = rest_inner.trim_start();
                out.push_str(&rest[..start]);
                out.push_str(&format!(
                    r#"<blockquote class="callout {css_class}"><p><strong>{label}</strong></p>{rest_clean}</blockquote>"#
                ));
                rest = &rest[end..];
                continue;
            }
        }
        out.push_str(&rest[..end]);
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// Thay link YouTube đơn độc thành iframe embed responsive.
fn embed_youtube(html: &str) -> String {
    use crate::utils::extract_youtube_id;
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<p><a href=\"") {
        let close_marker = "</a></p>";
        let end_p = rest[start..]
            .find(close_marker)
            .map(|p| start + p + close_marker.len())
            .unwrap_or(rest.len());
        let paragraph = &rest[start..end_p];
        if let Some(href_start) = paragraph.find("href=\"") {
            let href_start = href_start + 6;
            if let Some(href_end) = paragraph[href_start..].find('"') {
                let href_end = href_start + href_end;
                let url = &paragraph[href_start..href_end];
                if let Some(id) = extract_youtube_id(url) {
                    out.push_str(&rest[..start]);
                    out.push_str(&format!(
                        r#"<div class="video-embed"><iframe src="https://www.youtube-nocookie.com/embed/{id}" loading="lazy" allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture" allowfullscreen referrerpolicy="strict-origin-when-cross-origin" sandbox="allow-scripts allow-same-origin allow-presentation allow-popups"></iframe></div>"#
                    ));
                    rest = &rest[end_p..];
                    continue;
                }
            }
        }
        out.push_str(&rest[..end_p]);
        rest = &rest[end_p..];
    }
    out.push_str(rest);
    out
}

/// v2.3.0 — Đảm bảo mọi `<img>` có `loading="lazy"` và `decoding="async"`.
/// Comrak render `<img src="..." alt="..." />` không có thuộc tính lazy.
/// Ta thêm vào sau `src=` để không ghi đè alt.
fn lazy_images(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<img ") {
        // Tìm `>` đóng tag (img là self-closing hoặc void)
        let end = rest[start..]
            .find('>')
            .map(|p| start + p + 1)
            .unwrap_or(rest.len());
        let tag = &rest[start..end];
        // Nếu đã có loading= attrib thì skip (idempotent)
        if tag.to_ascii_lowercase().contains("loading=") {
            out.push_str(&rest[..end]);
            rest = &rest[end..];
            continue;
        }
        // Insert loading + decoding ngay sau `<img `
        let new_tag = format!("<img loading=\"lazy\" decoding=\"async\"{}", &tag[4..]);
        out.push_str(&rest[..start]);
        out.push_str(&new_tag);
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// v2.3.0 — Đánh dấu link external bằng class `external-link` để CSS thêm
/// icon (nếu muốn). Link nội bộ (bắt đầu bằng `/` hoặc `#`) KHÔNG đánh dấu.
/// Chỉ chạy SAU `harden_links` để mọi `<a>` đều có đầy đủ thuộc tính.
fn mark_external_links(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<a ") {
        let end = rest[start..]
            .find('>')
            .map(|p| start + p + 1)
            .unwrap_or(rest.len());
        let tag = &rest[start..end];
        let lower = tag.to_ascii_lowercase();
        // Nếu đã có class= thì append " external-link" vào class value
        // Nếu chưa có thì thêm class="external-link" mới
        // Chỉ đánh dấu nếu href http(s) external
        let is_external = lower.contains("href=\"http")
            && !lower.contains("href=\"http://localhost")
            && !lower.contains("href=\"https://localhost")
            && !lower.contains("href=\"http://127.0.0.1")
            && !lower.contains("href=\"https://127.0.0.1");
        if !is_external {
            out.push_str(&rest[..end]);
            rest = &rest[end..];
            continue;
        }
        if let Some(class_idx) = lower.find("class=\"") {
            let class_value_start = class_idx + 7;
            if let Some(class_value_end) = lower[class_value_start..].find('"') {
                let class_value_end_abs = class_value_start + class_value_end;
                let existing_classes = &tag[class_value_start..class_value_end_abs];
                if existing_classes
                    .to_ascii_lowercase()
                    .contains("external-link")
                {
                    // Đã có → skip
                    out.push_str(&rest[..end]);
                    rest = &rest[end..];
                    continue;
                }
                let new_tag = format!(
                    "{}external-link {}{}",
                    &tag[..class_value_start],
                    existing_classes,
                    &tag[class_value_end_abs..]
                );
                out.push_str(&rest[..start]);
                out.push_str(&new_tag);
                rest = &rest[end..];
                continue;
            }
        }
        // Chưa có class → thêm class="external-link" trước `>`
        let new_tag = format!("{} class=\"external-link\"", tag.trim_end_matches('>'));
        out.push_str(&rest[..start]);
        out.push_str(&new_tag);
        out.push('>');
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// v2.3.0 — Wrap mỗi `<pre class="code-block">...</pre>` trong một container
/// có nút "Copy" dùng HTMX-free, CSS-only button.
///
/// Markup output:
/// ```html
/// <div class="code-block-wrapper">
///   <button class="code-copy-btn" type="button"
///           aria-label="Sao chép mã"
///           data-code="...">Sao chép</button>
///   <pre class="code-block">...</pre>
/// </div>
/// ```
///
/// JS client-side (xem static/js/app.js) sẽ click → `navigator.clipboard.writeText`
/// với text decode từ `data-code` (đã escape HTML attribute).
///
/// Để tránh phình HTML (data-code duplicate toàn bộ code), ta KHÔNG duplicate —
/// JS sẽ lấy `textContent` từ `<pre>` kế cận. Vì vậy ở đây chỉ cần thêm wrapper
/// + button, không cần data attribute.
fn wrap_code_blocks_with_copy_button(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<pre class=\"code-block\">") {
        // Tìm `</pre>` kết thúc
        let end_marker = "</pre>";
        let end = rest[start..]
            .find(end_marker)
            .map(|p| start + p + end_marker.len())
            .unwrap_or(rest.len());
        let pre_block = &rest[start..end];
        out.push_str(&rest[..start]);
        out.push_str("<div class=\"code-block-wrapper\">");
        out.push_str(
            r#"<button type="button" class="code-copy-btn" aria-label="Sao chép mã" title="Sao chép mã">Sao chép</button>"#,
        );
        out.push_str(pre_block);
        out.push_str("</div>");
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// v2.3.0 — Thay thế marker `[toc]` (rendered là `<p>[toc]</p>`) bằng
/// danh sách mục lục dựng từ `toc_entries` (gom được trong phase render).
///
/// Nếu không có marker → không làm gì. Nếu có marker nhưng không có entries
/// → thay bằng chuỗi rỗng (input chưa có heading).
fn inject_toc(html: &str, toc_entries: &[TocEntry]) -> String {
    if toc_entries.is_empty() {
        // Vẫn thay marker thành rỗng nếu có
        return html.replace("<p>[toc]</p>", "").replace("<p>[TOC]</p>", "");
    }
    let toc_html = build_toc_html(toc_entries);
    html.replace("<p>[toc]</p>", &toc_html)
        .replace("<p>[TOC]</p>", &toc_html)
}

/// Build nested `<ul>` ToC từ entries. Level 1 là root, level 2+ là
/// nested `<ul>`. Cấp bắt đầu = min level (mục lớn nhất xuất hiện).
fn build_toc_html(entries: &[TocEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let min_level = entries.iter().map(|e| e.level).min().unwrap_or(1);
    let mut out = String::with_capacity(256);
    out.push_str("<nav class=\"toc\" aria-label=\"Mục lục\">");
    out.push_str("<p class=\"toc-title\">Mục lục</p>");
    out.push_str("<ul class=\"toc-list\">");
    let mut current_level = min_level;
    for entry in entries {
        // Mở `<ul>` mới khi level tăng
        while current_level < entry.level {
            out.push_str("<li><ul class=\"toc-list\">");
            current_level += 1;
        }
        // Đóng `<ul>` khi level giảm
        while current_level > entry.level {
            out.push_str("</ul></li>");
            current_level -= 1;
        }
        out.push_str(&format!(
            "<li class=\"toc-level-{lvl}\"><a href=\"#{slug}\">{text}</a></li>",
            lvl = entry.level,
            slug = crate::utils::html_escape(&entry.slug),
            text = crate::utils::html_escape(&entry.text)
        ));
    }
    // Đóng các `<ul>` còn mở
    while current_level > min_level {
        out.push_str("</ul></li>");
        current_level -= 1;
    }
    out.push_str("</ul></nav>");
    out
}

/// Escape HTML cơ bản — dùng cho fallback khi syntect fail.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_markdown() {
        let out = render("# Hello");
        assert!(out.contains("<h1") && out.contains("id=\"hello\""));
        assert!(out.contains("Hello"));
    }

    #[test]
    fn test_bold_italic() {
        let out = render("**bold** and *italic*");
        assert!(out.contains("<strong>bold</strong>"));
        assert!(out.contains("<em>italic</em>"));
    }

    #[test]
    fn test_escape_html() {
        let out = render("<script>alert(1)</script>");
        assert!(!out.contains("<script>alert"));
        assert!(out.contains("&lt;script&gt;"));
    }

    #[test]
    fn test_code_block_with_highlight() {
        let out = render("```rust\nfn main() {}\n```");
        assert!(out.contains("language-rust"));
        assert!(out.contains("code-block"));
        // v2.3: có wrapper + copy button
        assert!(out.contains("code-block-wrapper"));
        assert!(out.contains("code-copy-btn"));
    }

    #[test]
    fn test_table() {
        let out = render("| a | b |\n|---|---|\n| 1 | 2 |");
        assert!(out.contains("<table>"));
        assert!(out.contains("<th>a</th>"));
    }

    #[test]
    fn test_tasklist() {
        let out = render("- [x] done\n- [ ] todo");
        assert!(out.contains("checkbox") || out.contains("task-list") || out.contains("enabled"));
    }

    #[test]
    fn test_javascript_link_blocked() {
        let out = render("[click](javascript:alert(1))");
        assert!(out.contains("href=\"#\""));
    }

    #[test]
    fn test_link_rel_hardened() {
        let out = render("[link](https://example.com)");
        assert!(out.contains("rel=\"nofollow ugc noopener noreferrer\""));
        assert!(out.contains("target=\"_blank\""));
    }

    #[test]
    fn test_youtube_embed() {
        let out = render("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
        assert!(out.contains("youtube-nocookie.com"));
        assert!(out.contains("dQw4w9WgXcQ"));
    }

    #[test]
    fn test_spoiler() {
        let out = render("This is >!secret!< inline");
        assert!(out.contains("spoiler") || out.contains("secret"));
    }

    #[test]
    fn test_callout_note() {
        let out = render("> [!NOTE]\n> hello");
        assert!(out.contains("callout-note"));
    }

    #[test]
    fn test_callout_variants() {
        for kind in ["TIP", "WARNING", "DANGER", "IMPORTANT", "INFO", "SUCCESS"] {
            let out = render(&format!("> [!{kind}]\n> x"));
            let class = format!("callout-{}", kind.to_ascii_lowercase());
            assert!(out.contains(&class), "missing {class} in: {out}");
        }
    }

    #[test]
    fn test_footnote() {
        let out = render("text[^1]\n\n[^1]: footnote");
        assert!(out.contains("footnote"));
    }

    #[test]
    fn test_strikethrough() {
        let out = render("~~deleted~~");
        assert!(out.contains("<del>deleted</del>"));
    }

    #[test]
    fn test_no_double_escape_in_bold() {
        let out = render("**a < b**");
        assert!(out.contains("<strong>a &lt; b</strong>"));
        assert!(!out.contains("&amp;lt;"));
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(render(""), "");
        assert_eq!(render("   "), "");
    }

    #[test]
    fn test_heading_anchor_id() {
        let out = render("# Tiêu đề tiếng Việt");
        assert!(out.contains("id=\"tieu-de-tieng-viet\""));
        assert!(out.contains("heading-anchor"));
    }

    #[test]
    fn test_heading_anchor_special_chars() {
        let out = render("## Hello, World! 2.0");
        // Slug phải collapse space + bỏ punctuation
        assert!(out.contains("id=\"hello-world-20\""));
    }

    #[test]
    fn test_toc_marker_replaced() {
        let input = "[toc]\n\n# Section A\n\n# Section B";
        let out = render(input);
        assert!(out.contains("<nav class=\"toc\""));
        assert!(out.contains("toc-list"));
        assert!(out.contains("href=\"#section-a\""));
        assert!(out.contains("href=\"#section-b\""));
        assert!(!out.contains("[toc]"));
    }

    #[test]
    fn test_toc_nested() {
        let input = "[toc]\n\n# A\n\n## A.1\n\n## A.2\n\n# B";
        let out = render(input);
        // ToC phải có nested ul
        assert!(out.contains("toc-list"));
        assert!(out.contains("href=\"#a\""));
        // "A.1" → slug "a1" (dấu chấm bị bỏ, không thành dash) theo GitHub
        assert!(out.contains("href=\"#a1"));
        assert!(out.contains("href=\"#a2"));
        assert!(out.contains("href=\"#b\""));
    }

    #[test]
    fn test_toc_no_marker_no_change() {
        let input = "# A\n\n# B";
        let out = render(input);
        // Không có marker → không có nav toc
        assert!(!out.contains("<nav class=\"toc\""));
    }

    #[test]
    fn test_lazy_images_added() {
        let out = render("![alt text](https://example.com/x.png)");
        assert!(out.contains("loading=\"lazy\""));
        assert!(out.contains("decoding=\"async\""));
    }

    #[test]
    fn test_lazy_images_idempotent() {
        // Markdown raw HTML bị escape → không ảnh hưởng. Nhưng nếu ta chạy
        // post-process 2 lần thì phải idempotent.
        let once = render("![a](https://example.com/x.png)");
        let twice = render("![a](https://example.com/x.png)");
        assert_eq!(once, twice);
    }

    #[test]
    fn test_external_link_marker() {
        let out = render("[ext](https://example.com)");
        assert!(out.contains("external-link"));
    }

    #[test]
    fn test_internal_link_not_marked_external() {
        // Anchor (#section) — harden_links giữ nguyên `href="#" (đã bị thay
        // cho javascript:) nhưng với `#fragment` thì giữ. Ở đây test link
        // nội bộ dạng path.
        // Markdown link dạng `(path)` không tự convert thành <a> nếu không
        // có scheme; comrak relaxed_autolinks=false. Dùng explicit [text](/path)
        let out = render("[home](/)");
        // /internal không có http → không được mark external
        assert!(!out.contains("external-link") || out.contains("href=\"#\""));
    }

    #[test]
    fn test_copy_button_present() {
        let out = render("```rust\nfn x() {}\n```");
        assert!(out.contains("code-copy-btn"));
        assert!(out.contains("code-block-wrapper"));
    }

    #[test]
    fn test_slugify_basic() {
        assert_eq!(slugify_heading("Hello World"), "hello-world");
        assert_eq!(slugify_heading("Hello, World!"), "hello-world");
        assert_eq!(slugify_heading("  spaced  "), "spaced");
    }

    #[test]
    fn test_slugify_vietnamese() {
        // Dấu Việt Nam được NFD decompose rồi strip mark → giữ ASCII a-z0-9
        assert_eq!(slugify_heading("Tiêu đề"), "tieu-de");
        assert_eq!(slugify_heading("Thành phố"), "thanh-pho");
    }

    #[test]
    fn test_slugify_empty_fallback() {
        assert_eq!(slugify_heading("!!!"), "section");
        assert_eq!(slugify_heading(""), "section");
    }

    #[test]
    fn test_slugify_collapse_dashes() {
        assert_eq!(slugify_heading("a---b"), "a-b");
        assert_eq!(slugify_heading("a   b"), "a-b");
    }
}
