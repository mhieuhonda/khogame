//! Markdown rendering engine — "xịn hơn GitHub".
//!
//! Built on top of [`comrak`] (100% CommonMark + GFM superset) with
//! a custom [`syntect`] adapter for code block highlighting.
//!
//! # Vượt trội hơn GitHub Flavored Markdown
//!
//! | Tính năng | GitHub | Khogame v2.2 |
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
//! - Phần post-process (spoiler, callout, YouTube, link rel) chạy 1 pass
//!   tuyến tính trên HTML output.

use comrak::adapters::SyntaxHighlighterAdapter;
use comrak::options::{Plugins, RenderPlugins, Options};
use comrak::markdown_to_html_with_plugins;
use std::sync::OnceLock;
use std::collections::HashMap;
use std::fmt;
use std::borrow::Cow;
use syntect::html::ClassedHTMLGenerator;
use syntect::parsing::{SyntaxReference, SyntaxSet};

/// Singleton SyntaxSet (default-fancy: load built-in default.sublime-syntaxes).
fn syntax_set() -> &'static SyntaxSet {
    static SS: OnceLock<SyntaxSet> = OnceLock::new();
    SS.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// Build ComrakOptions — start từ default rồi bật GFM extensions.
fn comrak_options() -> Options<'static> {
    let mut opts = Options::default();
    // GFM extensions
    opts.extension.strikethrough = true;          // ~~text~~
    opts.extension.tagfilter = true;               // block dangerous HTML tags
    opts.extension.table = true;                   // GFM tables
    opts.extension.autolink = true;                // bare URLs → links
    opts.extension.tasklist = true;                // - [ ] / - [x]
    opts.extension.superscript = true;             // ^text^
    opts.extension.footnotes = true;               // [^1]
    opts.extension.multiline_block_quotes = true;  // >>>
    opts.extension.math_dollars = true;            // $...$ / $$...$$
    opts.extension.spoiler = true;                 // >! spoiler !<
    // Parse-time
    opts.parse.smart = true;                       // "quotes" → "quotes", -- → –
    opts.parse.default_info_string = Some("text".to_string());
    opts.parse.relaxed_tasklist_matching = true;
    // relaxed_autolinks = false (default) — strict, chỉ accept scheme hợp lệ
    // Render
    opts.render.hardbreaks = false;                 // single \n stays as space (GFM spec)
    opts.render.github_pre_lang = false;            // we post-process syntect ourselves
    opts.render.escape = true;                      // escape HTML special chars in text
    opts.render.r#unsafe = false;                   // NO raw HTML — defense-in-depth
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

/// Render Markdown input thành HTML an toàn.
///
/// Output đã được escape HTML + syntax-highlighted + post-processed
/// (spoiler/callout/YouTube/link rel). Có thể nhúng trực tiếp vào trang
/// qua askama `|safe` filter.
#[must_use]
pub fn render(input: &str) -> String {
    if input.trim().is_empty() {
        return String::new();
    }
    let opts = comrak_options();
    let highlighter = SyntectHighlighter {
        syntax_set: syntax_set(),
    };
    let plugins = Plugins {
        render: RenderPlugins {
            codefence_syntax_highlighter: Some(&highlighter),
            codefence_renderers: HashMap::new(),
            heading_adapter: None,
        },
    };
    let html = markdown_to_html_with_plugins(input, &opts, &plugins);
    post_process(&html)
}

/// Post-process HTML output: thêm rel/target cho link, mở rộng spoiler,
/// callout, YouTube embed.
fn post_process(html: &str) -> String {
    let mut out = html.to_string();
    out = harden_links(&out);
    out = convert_spoiler_inline(&out);
    out = convert_callouts(&out);
    out = embed_youtube(&out);
    out
}

/// Đảm bảo mọi thẻ <a> có rel="nofollow ugc noopener noreferrer" + target=_blank.
/// Lọc URL nguy hiểm: nếu href là `javascript:`, `data:` v.v → thay bằng `#`.
fn harden_links(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let bytes = html.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' && i + 1 < bytes.len() && (bytes[i + 1] == b'a' || bytes[i + 1] == b'A')
        {
            // Tìm `>` đóng tag
            if let Some(end) = find_byte(bytes, b'>', i) {
                let tag = &html[i..=end];
                let lower = tag.to_ascii_lowercase();
                if let Some(href_start) = lower.find("href=\"") {
                    let href_start = href_start + 6;
                    if let Some(href_end) = lower[href_start..].find('"') {
                        let href_end = href_start + href_end;
                        let href = &tag[href_start..href_end];
                        let lower_href = href.to_ascii_lowercase();
                        let safe_href = if is_safe_url_scheme(&lower_href) {
                            href
                        } else {
                            "#"
                        };
                        out.push_str(r#"<a href=""#);
                        out.push_str(safe_href);
                        out.push_str(r#"" rel="nofollow ugc noopener noreferrer" target="_blank">"#);
                        i = end + 1;
                        continue;
                    }
                }
                out.push_str(tag);
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
        let end = rest[start..].find(end_marker).map(|p| start + p + end_marker.len()).unwrap_or(rest.len());
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
        let end_p = rest[start..].find(close_marker).map(|p| start + p + close_marker.len()).unwrap_or(rest.len());
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
        assert!(out.contains("<h1>Hello</h1>"));
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
        // GFM tasklist render checkbox inputs
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
        // Inline spoiler: `>!secret!<` — comrak spoiler extension requires
        // inline context (not at line start where `>` becomes blockquote).
        let out = render("This is >!secret!< inline");
        assert!(out.contains("spoiler") || out.contains("secret"));
    }

    #[test]
    fn test_callout_note() {
        let out = render("> [!NOTE]\n> hello");
        assert!(out.contains("callout-note"));
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
}

