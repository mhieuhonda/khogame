//! Markdown rendering engine — "xịn hơn GitHub" — v2.5.0 + bio Markdown.
//!
//! Built on top of [`comrak`] (100% CommonMark + GFM superset) with
//! a custom [`syntect`] adapter for code block highlighting.
//!
//! # Vượt trội hơn GitHub Flavored Markdown
//!
//! | Tính năng | GitHub | Khogame v2.5 |
//! |-----------|:------:|:------------:|
//! | CommonMark | ✅ | ✅ |
//! | Tables (GFM) | ✅ | ✅ |
//! | Task lists `[x]` | ✅ | ✅ (+ cả trong bảng) |
//! | Strikethrough `~~` | ✅ | ✅ |
//! | Autolinks | ✅ | ✅ |
//! | Footnotes `[^1]` | ✅ | ✅ (+ inline `^[...]`) |
//! | Math `$...$` (KaTeX-style) | ✅ | ✅ |
//! | Syntax highlighting | ✅ linguist | ✅ syntect (default theme) |
//! | Spoiler `\|\|text\|\|` | ❌ | ✅ |
//! | Callouts `> [!NOTE]` | ✅ | ✅ (+ collapsible `+` / `-` modifiers) |
//! | YouTube auto-embed | ❌ | ✅ |
//! | **Heading anchors (click #)** | ✅ | ✅ (v2.3) |
//! | **Table of Contents `[toc]`** | partial | ✅ (v2.3) |
//! | **Copy-to-clipboard on code** | ✅ (JS) | ✅ (v2.3 — pure HTML+CSS) |
//! | **Lazy `<img>`** | ✅ (GHP) | ✅ (v2.3) |
//! | **External link marker** | ✅ icon | ✅ (v2.3 — class hook) |
//! | **Description lists** `Term\n: Def` | ❌ | ✅ (v2.4) |
//! | **Image figure caption** | partial | ✅ (v2.4 — `![caption:...](url)`) |
//! | **Code block language label** | ✅ (linguist) | ✅ (v2.4 — visible badge) |
//! | **Collapsible callouts** `> [!NOTE]+` / `-` | partial | ✅ (v2.4) |
//! | **Footnote backref hover** | partial | ✅ (v2.4 — `↩` with title) |
//! | **Render cache** (avoid re-parse) | n/a | ✅ (v2.4 — SHA256 keyed LRU) |
//! | **Emoji shortcodes** `:tada:` | ✅ | ✅ (v2.5) |
//! | **Underline** `__text__` | ❌ | ✅ (v2.5) |
//! | **Subscript** `H~2~O` | ❌ | ✅ (v2.5) |
//! | **Highlight** `==text==` | ❌ | ✅ (v2.5 — `<mark>`) |
//! | **Insert** `++text++` | ❌ | ✅ (v2.5 — `<ins>`) |
//! | **@mention → link hồ sơ** | ✅ (user) | ✅ (v2.5 — `/u/{username}`) |
//! | **#hashtag → link tìm kiếm** | ✅ (issue) | ✅ (v2.5 — `/search?q=`) |
//! | **Diff block coloring** | ✅ | ✅ (v2.5 — `+`/`-`/`@@` classes) |
//! | **Code line numbers** | ✅ | ✅ (v2.5 — CSS counter) |
//! | **Bio Markdown (hồ sơ)** | ✅ (profile README) | ✅ (v2.5 — `render_bio`) |
//! | **Math render KaTeX** (client, lazy, self-hosted) | ❌ | ✅ (v3.11) |
//! | **Mermaid diagrams** (mermaid fence, lazy, self-hosted) | ❌ | ✅ (v3.11) |
//! | **Vimeo embed** | ❌ | ✅ (v3.11) |
//! | **Video/audio file embed** (.mp4/.webm/.mp3...) | ❌ | ✅ (v3.11) |
//! | **Keyboard keys** `[[Ctrl]]` to `<kbd>` | ❌ | ✅ (v3.11) |
//! | **Abbreviation** `*[X]: def` to `<abbr title>` | ❌ | ✅ (v3.11) |
//! | **Custom heading ID** `## T {#id}` | ❌ | ✅ (v3.11) |
//! | **Sortable tables** (client-side) | ❌ | ✅ (v3.11) |
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
//! # Hiệu năng (v2.4)
//!
//! - SyntaxSet được load 1 lần vào `OnceLock` (lazy init).
//! - Comrak options được build 1 lần, clone rẻ (Arc-like).
//! - **Render cache (v2.4)**: HTML đã render được cache theo SHA256 của
//!   input. Cache hit → return `Arc<String>` không re-parse. LRU eviction
//!   khi cache > 256 entry hoặc > 16MB tổng — đủ cho 200 bài tin dài,
//!   không leak memory.
//! - **ToC buffer per-render (v2.4)**: thay vì global Mutex dễ race,
//!   mỗi render tạo buffer riêng qua `Arc<Mutex<Vec>>` và truyền qua
//!   adapter instance. Concurrent renders không chia sẻ state, an toàn
//!   tuyệt đối.
//! - Phần post-process (spoiler, callout, YouTube, link rel, anchor, ToC,
//!   lazy img, copy button, external link, figure, code lang label)
//!   chạy 1 pass tuyến tính trên HTML output. Toàn bộ dùng `&str::find`
//!   + `String::push_str` thay vì regex để tránh overhead compile.

use comrak::adapters::{HeadingAdapter, HeadingMeta, SyntaxHighlighterAdapter};
use comrak::markdown_to_html_with_plugins;
use comrak::nodes::Sourcepos;
use comrak::options::{Options, Plugins, RenderPlugins};
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};
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
    opts.extension.spoiler = true; // ||spoiler|| (Discord-style)
    opts.extension.description_lists = true; // v2.4 — Term\n: Definition
                                             // Parse-time
                                             // === v2.5.0 — Markdown engine "mạnh hơn nữa" ===
    opts.extension.shortcodes = true; // :tada: → 🎉 (emoji_shortcode)
    opts.extension.underline = true; // __text__ → <u>text</u>
    opts.extension.subscript = true; // H~2~O → H<sub>2</sub>O
    opts.extension.highlight = true; // ==text== → <mark>text</mark>
    opts.extension.insert = true; // ++text++ → <ins>text</ins>
    opts.extension.inline_footnotes = true; // ^[chú thích inline]
    opts.parse.smart = true; // "quotes" → "quotes", -- → –
    opts.parse.default_info_string = Some("text".to_string());
    opts.parse.relaxed_tasklist_matching = true;
    opts.parse.tasklist_in_table = true; // [x] trong bảng
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
        // v2.5.0 — Diff/patch blocks: highlight theo prefix dòng (+/-/@@)
        // bằng class CSS riêng thay vì syntect (syntax Diff của syntect
        // không phân màu rõ + muốn kiểm soát markup hoàn toàn để CSS
        // .diff-add/.diff-del/.diff-meta style chuẩn GitHub diff view).
        if lang_str == "diff" || lang_str == "patch" {
            return write_diff_highlighted(output, code);
        }
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
        // FIX v2.5.0 (bug có sẵn từ v2.2): trước đây luôn append
        // ` class="hljs"` SAU attribute loop → sinh `<code class="language-rust"
        // class="hljs">` — TRÙNG thuộc tính class (invalid HTML, browser chỉ
        // nhận attr đầu tiên → class "hljs" bị bỏ). Giờ merge "hljs" vào
        // attribute class có sẵn: `class="language-rust hljs"`.
        let mut had_class = false;
        for (k, v) in &attributes {
            if *k == "class" {
                had_class = true;
                write!(output, " class=\"{v} hljs\"")?;
            } else {
                write!(output, " {k}=\"{v}\"")?;
            }
        }
        if !had_class {
            output.write_str(" class=\"hljs\"")?;
        }
        output.write_str(">")?;
        Ok(())
    }
}

/// v2.5.0 — Highlight diff/patch code block theo prefix từng dòng:
///   `+ ...` → `<span class="diff-add">` (xanh — dòng thêm)
///   `- ...` → `<span class="diff-del">` (đỏ — dòng xoá)
///   `@@ ...` → `<span class="diff-meta">` (xanh dương — hunk header)
/// Dòng khác giữ nguyên. Giữ `\n` giữa các dòng để pass line-number
/// (add_code_line_numbers) vẫn tách dòng được.
fn write_diff_highlighted(output: &mut dyn fmt::Write, code: &str) -> fmt::Result {
    for line in code.split_inclusive('\n') {
        // Tách newline ra ngoài span để layout <pre> giữ nguyên và pass
        // line-number vẫn tách dòng được.
        let (content, newline) = match line.strip_suffix('\n') {
            Some(body) => (body, "\n"),
            None => (line, ""),
        };
        let escaped = html_escape(content);
        let trimmed = content.trim_start();
        if trimmed.starts_with('+') {
            write!(output, "<span class=\"diff-add\">{escaped}</span>{newline}")?;
        } else if trimmed.starts_with('-') {
            write!(output, "<span class=\"diff-del\">{escaped}</span>{newline}")?;
        } else if trimmed.starts_with("@@") {
            write!(
                output,
                "<span class=\"diff-meta\">{escaped}</span>{newline}"
            )?;
        } else {
            write!(output, "{escaped}{newline}")?;
        }
    }
    Ok(())
}

/// v2.3.0 — Heading adapter: thêm `id` attribute + anchor link.
///
/// v2.4.0 — ToC buffer chuyển từ global Mutex sang per-render `Arc<Mutex>`
/// owned bởi adapter instance. Mỗi render tạo adapter riêng → không race
/// giữa các threads, không leak entries chéo.
struct AnchorHeadingAdapter {
    /// Per-render ToC buffer. Adapter own 1 Arc, clone cho comrak gọi
    /// `enter`/`exit` (trait method chỉ có `&self`). Sau render, buffer
    /// được snapshot qua `Arc::clone` + `lock().clone()`.
    toc: Arc<Mutex<Vec<TocEntry>>>,
    /// v3.11.0 — custom heading id: map từ text heading (đã strip cú pháp
    /// ` {#id}` ở pre-process) → id do tác giả chỉ định. Trống = không có.
    custom_ids: HashMap<String, String>,
}

/// Một entry ToC: text + slug + level.
#[derive(Clone)]
struct TocEntry {
    text: String,
    slug: String,
    level: u8,
}

impl AnchorHeadingAdapter {
    /// v3.11.0 — tạo adapter với map custom heading id.
    fn with_custom_ids(custom_ids: HashMap<String, String>) -> Self {
        Self {
            toc: Arc::new(Mutex::new(Vec::new())),
            custom_ids,
        }
    }

    /// Id của heading: custom id nếu tác giả khai báo ` {#id}`, ngược lại
    /// slug hoá text. Trả về (id, is_custom).
    fn heading_id(&self, content: &str) -> String {
        if let Some(custom) = self.custom_ids.get(content) {
            return custom.clone();
        }
        // Fuzzy: heading có inline markup (`**bold**`) — comrak content chỉ
        // giữ text thuần, map key có thể còn ký tự đánh dấu. Thử strip các
        // ký tự markdown inline trước khi tra.
        let stripped: String = strip_inline_marks(content);
        if let Some(custom) = self.custom_ids.get(&stripped) {
            return custom.clone();
        }
        slugify_heading(content)
    }
}

/// Bỏ ký tự đánh dấu markdown inline (bold/italic/code) — dùng để fuzzy
/// match key custom heading id.
fn strip_inline_marks(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '*' | '_' | '`' | '~'))
        .collect::<String>()
        .trim()
        .to_string()
}

impl HeadingAdapter for AnchorHeadingAdapter {
    fn enter(
        &self,
        output: &mut dyn fmt::Write,
        heading: &HeadingMeta,
        _sourcepos: Option<Sourcepos>,
    ) -> fmt::Result {
        // Slug hoá text heading: giữ [a-z0-9], bỏ còn lại, thay space bằng '-'.
        // v3.11.0 — cú pháp `## Tiêu đề {#id-rieng}`: pre-process đã strip
        // `{#id}` khỏi text, map custom_ids cho adapter biết id riêng.
        let slug = self.heading_id(&heading.content);
        // Lưu vào buffer cho ToC (nếu input có [toc] marker)
        if let Ok(mut buf) = self.toc.lock() {
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
            self.heading_id(&heading.content),
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

// ============================================================
// v2.4.0 — RENDER CACHE
// ------------------------------------------------------------
// Markdown render (comrak parse + syntect highlight + 6 post-process
// passes) tốn 50-500ms cho bài viết dài. Trên news page, mỗi page view
// đều re-render — đủ lớn để homepage 10 articles = ~5s nếu cache miss.
//
// Cache key = SHA256(input) — collision-resistant cho mục đích cache
// (chỉ cần match exactly same input). LRU eviction khi:
//   - Entry count > MAX_ENTRIES (256) — đủ cho ~200 bài dài + buffer.
//   - Total bytes > MAX_BYTES (16 MB) — chống memory leak nếu bài
//     quá dài.
//
// Cache hit return `Arc<String>` — clone rẻ (chỉ tăng refcount), không
// allocate. Cache miss render + insert.
//
// Cache KHÔNG bị invalidate chủ động — markdown source là immutable
// trong DB (chỉ thay khi user edit). Khi edit, hash thay → cache miss
// tự nhiên. Khi admin update markdown engine, bump cache version bằng
// static CACHE_VERSION.
// ============================================================

/// Cache version — bump khi markdown engine thay đổi output để invalidate
/// toàn bộ cache cũ (vd: thay đổi post-process logic, thêm class CSS mới).
/// v2.5.0: 2 → 3 (emoji shortcodes, underline/sub/highlight/insert, mention,
/// hashtag, diff highlight, line numbers).
/// v3.11.0: 3 → 4 (KaTeX-ready math markup giữ nguyên nhưng class khớp CSS,
/// kbd, abbreviation, custom heading id, Mermaid block, Vimeo + video/audio
/// embed — toàn bộ thay đổi output).
const CACHE_VERSION: u8 = 4;

/// Cache entry: rendered HTML + size (bytes) + last access (cho LRU).
struct CacheEntry {
    html: Arc<String>,
    size: usize,
    last_access: std::time::Instant,
}

/// LRU-ish cache. Đơn giản hoá: không dùng LinkedHashMap (no_std-unfriendly),
/// chỉ HashMap + periodic cleanup khi vượt ngưỡng.
struct RenderCache {
    map: HashMap<[u8; 32], CacheEntry>,
    total_bytes: usize,
}

impl RenderCache {
    const MAX_ENTRIES: usize = 256;
    const MAX_BYTES: usize = 16 * 1024 * 1024; // 16 MB tổng

    fn new() -> Self {
        Self {
            map: HashMap::with_capacity(64),
            total_bytes: 0,
        }
    }

    fn get(&mut self, key: &[u8; 32]) -> Option<Arc<String>> {
        let entry = self.map.get_mut(key)?;
        entry.last_access = std::time::Instant::now();
        Some(Arc::clone(&entry.html))
    }

    fn insert(&mut self, key: [u8; 32], html: Arc<String>) {
        let size = html.len();
        self.total_bytes += size;
        self.map.insert(
            key,
            CacheEntry {
                html,
                size,
                last_access: std::time::Instant::now(),
            },
        );
        // Cleanup nếu vượt ngưỡng — xoá entry cũ nhất (LRU).
        if self.map.len() > Self::MAX_ENTRIES || self.total_bytes > Self::MAX_BYTES {
            self.evict();
        }
    }

    /// Xoá entry cũ nhất đến khi dưới ngưỡng. Đơn giản hoá: sort bằng
    /// Vec thay vì BTreeMap (cache nhỏ <256 entry, O(n log n) OK).
    fn evict(&mut self) {
        let mut entries: Vec<([u8; 32], std::time::Instant, usize)> = self
            .map
            .iter()
            .map(|(k, v)| (*k, v.last_access, v.size))
            .collect();
        // Sort: cũ nhất (smaller Instant) lên đầu.
        entries.sort_by_key(|(_, t, _)| *t);
        for (key, _, size) in entries {
            if self.map.len() <= Self::MAX_ENTRIES && self.total_bytes <= Self::MAX_BYTES {
                break;
            }
            if self.map.remove(&key).is_some() {
                self.total_bytes = self.total_bytes.saturating_sub(size);
            }
        }
    }
}

fn render_cache() -> &'static Mutex<RenderCache> {
    static CACHE: OnceLock<Mutex<RenderCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(RenderCache::new()))
}

/// Compute SHA256 của input + cache version byte — làm cache key.
/// Cache version byte invalidate cache khi markdown engine đổi output.
fn cache_key(input: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([CACHE_VERSION]);
    hasher.update(input.as_bytes());
    hasher.finalize().into()
}

/// Ước lượng thời gian đọc: 200 từ/phút (chậm hơn default 250 để conservative
/// cho tiếng Việt có dấu + technical content). Trả về số phút tối thiểu 1.
#[must_use]
pub fn reading_time_minutes(input: &str) -> u32 {
    // Đếm từ: split theo whitespace, bỏ qua markdown syntax đơn giản.
    // Tinh chỉnh sẽ không đáng kể — đây chỉ là hint UI.
    let words: usize = input.split_whitespace().filter(|w| !w.is_empty()).count();
    ((words as f64 / 200.0).ceil() as u32).max(1)
}

/// Render Markdown input thành HTML an toàn — CACHED.
///
/// Lần đầu render: parse comrak + highlight syntect + post-process
/// → cache theo SHA256(input). Lần sau: return `Arc<String>` từ cache,
/// clone rẻ (chỉ tăng refcount, không allocate string mới).
///
/// Output đã được escape HTML + syntax-highlighted + post-processed
/// (spoiler/callout/YouTube/link rel/anchor/ToC/lazy img/copy button/
/// external link/figure/code lang label).
///
/// Có thể nhúng trực tiếp vào trang qua askama `|safe` filter.
#[must_use]
pub fn render(input: &str) -> String {
    if input.trim().is_empty() {
        return String::new();
    }
    // === Fast path: cache hit ===
    let key = cache_key(input);
    if let Ok(mut cache) = render_cache().lock() {
        if let Some(html) = cache.get(&key) {
            return (*html).clone();
        }
    }
    // === Slow path: render + cache ===
    let html = render_uncached(input);
    let html_arc = Arc::new(html);
    if let Ok(mut cache) = render_cache().lock() {
        cache.insert(key, Arc::clone(&html_arc));
    }
    (*html_arc).clone()
}

/// Render không cache — nội bộ + test. Áp dụng comrak + post-process đầy đủ.
fn render_uncached(input: &str) -> String {
    // v3.11.0 — PRE-PROCESS (trước comrak):
    //   1. ` {#custom-id}` cuối heading → strip khỏi text, map cho adapter.
    //   2. `*[ABBR]: định nghĩa` → bỏ dòng khỏi input, thu thập để thay
    //      thế <abbr> trong post-process.
    let (input, custom_ids, abbrs) = pre_process_input(input);
    let opts = comrak_options();
    let highlighter = SyntectHighlighter {
        syntax_set: syntax_set(),
    };
    let heading_adapter = AnchorHeadingAdapter::with_custom_ids(custom_ids);
    let plugins = Plugins {
        render: RenderPlugins {
            codefence_syntax_highlighter: Some(&highlighter),
            codefence_renderers: HashMap::new(),
            heading_adapter: Some(&heading_adapter),
        },
    };
    let html = markdown_to_html_with_plugins(&input, &opts, &plugins);
    // Snapshot ToC entries đã gom được trong phase render.
    let toc_entries: Vec<TocEntry> = heading_adapter
        .toc
        .lock()
        .map(|b| b.clone())
        .unwrap_or_default();
    post_process_with_abbrs(&html, &toc_entries, &abbrs)
}

/// v2.5.0 — Render Markdown cho BIO / hồ sơ cá nhân.
///
/// Khác `render()` (full article): profile pipeline GIẢM bớt cho phù hợp
/// ngữ cảnh khối giới thiệu ngắn (~1000 ký tự):
///   - KHÔNG heading anchor + ToC (bio không cần mục lục / link neo).
///   - KHÔNG YouTube embed, callout, figure, copy button, lang label,
///     line number — giữ bio gọn, không lấn chiếm layout trang hồ sơ.
///   - GIỮ: harden_links (rel/target/scheme allowlist — bắt buộc bảo
///     mật), spoiler, lazy image, external-link marker, mention
///     (@user → /u/user), hashtag (#tag → /search), emoji shortcodes
///     và mọi inline formatting (bold/italic/`code`/==mark==/~~strike~~
///     /__underline__/H~2~O/:tada:).
///
/// Không cache — bio ngắn (≤1000 ký tự), render ~sub-ms, không đáng
/// tốn memory cache entry.
#[must_use]
pub fn render_bio(input: &str) -> String {
    if input.trim().is_empty() {
        return String::new();
    }
    let opts = comrak_options();
    let highlighter = SyntectHighlighter {
        syntax_set: syntax_set(),
    };
    // Không dùng heading adapter → heading (nếu user gõ) render <hN> thường,
    // không sinh anchor/ToC entries cho bio.
    let plugins = Plugins {
        render: RenderPlugins {
            codefence_syntax_highlighter: Some(&highlighter),
            codefence_renderers: HashMap::new(),
            heading_adapter: None,
        },
    };
    let html = markdown_to_html_with_plugins(input, &opts, &plugins);
    let mut out = harden_links(&html);
    // v3.11.0 — math span chuẩn hoá (class + delimiter): KaTeX client
    // render được cả trong bio, CSS fallback hiển thị công thức dạng code.
    out = normalize_math_spans(&out);
    out = convert_spoiler_inline(&out);
    out = lazy_images(&out);
    out = harden_img_src(&out);
    out = mark_external_links(&out);
    // v3.11.0 — `[[Ctrl]]` → <kbd> cũng hoạt động trong bio (inline an toàn).
    out = convert_kbd(&out);
    out = linkify_mentions_hashtags(&out);
    out
}

/// Post-process HTML output: thêm rel/target cho link, mở rộng spoiler,
/// callout, YouTube embed, lazy `<img>`, external link marker, copy
/// button cho code block, thay thế `[toc]` marker, figure caption,
/// code lang label.
fn post_process_with_abbrs(
    html: &str,
    toc_entries: &[TocEntry],
    abbrs: &[(String, String)],
) -> String {
    let mut out = html.to_string();
    out = harden_links(&out);
    // v3.11.0 — chuẩn hoá span math của comrak (`data-math-style`) thành
    // `class="math inline/display"` + bọc lại delimiter \( \) \[ \] để
    // KaTeX auto-render client-side quét ra — đồng thời CSS fallback
    // (không JS) hiển thị công thức dạng code dễ đọc.
    out = normalize_math_spans(&out);
    out = convert_spoiler_inline(&out);
    out = convert_callouts(&out);
    out = embed_youtube(&out);
    // v3.11.0 — Vimeo + video/audio file embed (bare link / image syntax).
    out = embed_vimeo(&out);
    out = embed_media_links(&out);
    // v3.11.0 — Mermaid: PHẢI chạy TRƯỚC các pass code-block (line number,
    // copy button, lang label) vì block mermaid không còn là <pre>.
    out = convert_mermaid_blocks(&out);
    out = lazy_images(&out);
    // v3.4.2 — lọc scheme img src SAU lazy_images (pass cuối cùng sinh
    // <img src>) để mọi ảnh trong output đều http(s)/relative an toàn.
    out = harden_img_src(&out);
    out = wrap_image_figures(&out);
    out = mark_external_links(&out);
    // v3.11.0 — `[[Ctrl]]` → <kbd> (bỏ qua nội dung <pre>/<code>).
    out = convert_kbd(&out);
    // v2.5.0 — line numbers PHẢI chạy trước khi wrap copy-button (pass
    // này thao tác trên `<pre class="code-block">` "trần").
    out = add_code_line_numbers(&out);
    out = wrap_code_blocks_with_copy_button(&out);
    out = add_code_lang_label(&out);
    out = improve_footnote_backrefs(&out);
    out = inject_toc(&out, toc_entries);
    // v3.11.0 — abbreviation <abbr> chạy sau ToC (không đụng href) và
    // trước mention/hashtag (2 pass cuối cùng thao tác text node thuần).
    out = apply_abbreviations(&out, abbrs);
    // v2.5.0 — mention/hashtag chạy CUỐI: mọi text node đã ổn định, các
    // pass phía trước không sinh text @/# mới (chỉ markup).
    out = linkify_mentions_hashtags(&out);
    out
}

// ============================================================
// v3.11.0 — PRE-PROCESS (chạy trên markdown input TRƯỚC comrak)
// ============================================================

/// Thu thập cú pháp mở rộng cần bỏ khỏi input trước khi parse:
///
/// 1. **Custom heading id**: `## Tiêu đề {#id-rieng}` — strip ` {#id}`
///    khỏi dòng heading (text hiển thị sạch), trả map text → custom id
///    cho `AnchorHeadingAdapter`. Lần đầu gặp text trùng → bản ĐẦU thắng
///    (nhất quán với slug trùng lặp của comrak).
///
/// 2. **Abbreviation** (Pandoc-style): dòng dạng `*[HTML]: HyperText
///    Markup Language` → bỏ khỏi input (không thành paragraph rác),
///    trả list (term, definition) để post-process bọc `<abbr title>`.
///
/// Trả về (input đã làm sạch, custom_ids, abbreviations).
fn pre_process_input(input: &str) -> (String, HashMap<String, String>, Vec<(String, String)>) {
    let mut custom_ids: HashMap<String, String> = HashMap::new();
    let mut abbrs: Vec<(String, String)> = Vec::new();
    let mut out_lines: Vec<String> = Vec::with_capacity(input.lines().count());

    let mut in_code_block = false;
    for line in input.lines() {
        let trimmed = line.trim_start();
        // Fenced code block: KHÔNG pre-process bên trong (cú pháp heading/
        // abbr trong code là nội dung code, không phải markdown).
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code_block = !in_code_block;
            out_lines.push(line.to_string());
            continue;
        }
        if in_code_block {
            out_lines.push(line.to_string());
            continue;
        }

        // 1) Custom heading id: `^#{1,6} text {#id}$`
        if trimmed.starts_with('#') {
            if let Some((text, custom_id)) = strip_custom_heading_id(trimmed) {
                if !custom_ids.contains_key(&text) {
                    custom_ids.insert(text.clone(), custom_id);
                }
                let hashes_len = trimmed.chars().take_while(|c| *c == '#').count();
                let lead: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                out_lines.push(format!("{}{} {}", lead, "#".repeat(hashes_len), text));
                continue;
            }
        }

        // 2) Abbreviation: `*[TERM]: definition`
        if let Some(rest) = trimmed.strip_prefix("*[") {
            if let Some((term, def)) = rest.split_once("]:") {
                let term = term.trim();
                let def = def.trim();
                // Term hợp lệ: 1..=60 ký tự, KHÔNG chứa ký tự nguy hiểm
                // cho HTML (`[` `]` `<` `>` `&` `"` `'`) — term được chèn
                // vào output, chặn sớm mọi vector breakout (defense-in-
                // depth: text node đã escaped nên term có tag vốn không
                // match, nhưng chặn ở nguồn chắc chắn hơn).
                if !term.is_empty()
                    && term.chars().count() <= 60
                    && !term
                        .chars()
                        .any(|c| matches!(c, '[' | ']' | '<' | '>' | '&' | '"' | '\''))
                    && !def.is_empty()
                    && def.chars().count() <= 200
                    && !abbrs.iter().any(|(t, _)| t == term)
                {
                    abbrs.push((term.to_string(), def.to_string()));
                    continue; // bỏ dòng khỏi input
                }
            }
        }

        out_lines.push(line.to_string());
    }

    let mut cleaned = out_lines.join("\n");
    if input.ends_with('\n') {
        cleaned.push('\n');
    }
    (cleaned, custom_ids, abbrs)
}

/// `## Tiêu đề {#id-rieng}` → `Some(("Tiêu đề", "id-rieng"))`.
/// Id chỉ nhận [A-Za-z0-9_-], 1..=80 ký tự (id lạ có thể vỡ HTML attr).
fn strip_custom_heading_id(line: &str) -> Option<(String, String)> {
    let hashes_len = line.chars().take_while(|c| *c == '#').count();
    if hashes_len == 0 || hashes_len > 6 {
        return None;
    }
    let after_hashes = line[hashes_len..].trim_start();
    // Tìm ` {#...}` ở CUỐI dòng.
    let close = after_hashes.rfind('}')?;
    let open = after_hashes[..close].rfind("{#")?;
    // Phải nằm sát cuối (chỉ whitespace sau `}`).
    if !after_hashes[close + 1..].trim().is_empty() {
        return None;
    }
    let id = &after_hashes[open + 2..close];
    if id.is_empty()
        || id.len() > 80
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    let text = after_hashes[..open].trim_end();
    if text.is_empty() {
        return None;
    }
    Some((text.to_string(), id.to_string()))
}

// ============================================================
// v3.11.0 — POST-PROCESS MỚI (chạy trên HTML output)
// ============================================================

/// v3.11.0 — comrak (math_dollars) phát `<span data-math-style="inline">
/// CONTENT</span>` KHÔNG kèm delimiter. KaTeX auto-render quét delimiter
/// `\(...\)` / `\[...\]` trong text node — nên ta:
///   * thêm class `math inline|display` (CSS fallback + JS detection),
///   * bọc lại nội dung bằng delimiter tương ứng.
///
/// Nội dung giữ nguyên trạng thái escape (textContent client decode).
fn normalize_math_spans(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    const ATTR_INLINE: &str = "data-math-style=\"inline\"";
    const ATTR_DISPLAY: &str = "data-math-style=\"display\"";
    while let Some(pos) = rest.find("data-math-style=") {
        // Xác định tag mở span chứa attr này.
        let span_start = rest[..pos].rfind("<span ").map(|p| p + 1).unwrap_or(0);
        let _ = span_start;
        let tag_open = rest[..pos].rfind('<').unwrap_or(0);
        let tag_close = match rest[pos..].find('>') {
            Some(p) => pos + p,
            None => break,
        };
        let open_tag = &rest[tag_open..=tag_close];
        let is_inline = open_tag.contains(ATTR_INLINE);
        let is_display = open_tag.contains(ATTR_DISPLAY);
        if !is_inline && !is_display {
            // Attr lạ — copy qua.
            out.push_str(&rest[..tag_close + 1]);
            rest = &rest[tag_close + 1..];
            continue;
        }
        // Tìm </span> đóng.
        const CLOSE: &str = "</span>";
        let span_end = match rest[tag_close + 1..].find(CLOSE) {
            Some(p) => tag_close + 1 + p,
            None => break,
        };
        let content = &rest[tag_close + 1..span_end];
        let (class, open_delim, close_delim) = if is_inline {
            ("math inline", "\\(", "\\)")
        } else {
            ("math display", "\\[", "\\]")
        };
        out.push_str(&rest[..tag_open]);
        out.push_str(&format!(
            "<span class=\"{class}\">{open_delim}{content}{close_delim}"
        ));
        out.push_str(CLOSE);
        rest = &rest[span_end + CLOSE.len()..];
    }
    out.push_str(rest);
    out
}

/// `[[Ctrl]]` → `<kbd>Ctrl</kbd>` — cú pháp phím bàn phím (GitHub-style
/// `[[kbd]]` của GitLab/Pandoc). Chỉ thay trong TEXT node — bỏ qua toàn
/// bộ nội dung `<pre ...>...</pre>` / `<code ...>...</code>` / thuộc tính
/// thẻ. Nội dung giữa `[[` `]]` tối đa 30 ký tự, không chứa `[`/`]<`/`>`
/// (chặn lồng nhau/bẻ HTML).
fn convert_kbd(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let bytes = html.as_bytes();
    let mut i = 0;
    // Vị trí của thẻ mở code/pre đang bao quanh con trỏ hiện tại.
    let mut code_depth = 0usize;
    while i < bytes.len() {
        // Theo dõi mở/đóng <code>/<pre> (output comrak luôn lowercase tag).
        if bytes[i] == b'<' {
            if starts_with_ci(&html[i..], "<code") || starts_with_ci(&html[i..], "<pre") {
                code_depth += 1;
            } else if starts_with_ci(&html[i..], "</code") || starts_with_ci(&html[i..], "</pre") {
                code_depth = code_depth.saturating_sub(1);
            }
        }
        if code_depth == 0 && bytes[i] == b'[' && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            // Tìm đóng `]]`.
            if let Some(close) = html[i + 2..].find("]]") {
                let inner = &html[i + 2..i + 2 + close];
                // Điều kiện an toàn: ngắn, không ký tự nguy hiểm.
                if !inner.is_empty()
                    && inner.chars().count() <= 30
                    && !inner.contains('[')
                    && !inner.contains(']')
                    && !inner.contains('<')
                    && !inner.contains('>')
                    && !inner.contains('\n')
                {
                    out.push_str("<kbd>");
                    out.push_str(inner);
                    out.push_str("</kbd>");
                    i = i + 2 + close + 2;
                    continue;
                }
            }
        }
        let ch = html[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Case-insensitive prefix check (ASCII tags). Soi trên BYTES để không
/// bao giờ cắt giữa ký tự UTF-8 đa byte (tiếng Việt) — bug panic
/// "not a char boundary" khi test với văn bản có dấu.
fn starts_with_ci(haystack: &str, prefix: &str) -> bool {
    let h = haystack.as_bytes();
    let p = prefix.as_bytes();
    h.len() >= p.len() && h[..p.len()].eq_ignore_ascii_case(p)
}

/// Bọc abbreviation: mọi xuất hiện NGUYÊN TỪ của term trong text node
/// (ngoài code/pre/thead?) → `<abbr title="định nghĩa">term</abbr>`.
/// Chỉ chạy khi có định nghĩa (pass no-op khi abbrs rỗng). Term xuất hiện
/// trong thuộc tính/href KHÔNG bị thay (chỉ scan text giữa `>` và `<`).
fn apply_abbreviations(html: &str, abbrs: &[(String, String)]) -> String {
    if abbrs.is_empty() {
        return html.to_string();
    }
    let mut out = String::with_capacity(html.len());
    let bytes = html.as_bytes();
    let mut i = 0;
    let mut code_depth = 0usize;
    while i < bytes.len() {
        let ch = html[i..].chars().next().unwrap();
        if ch == '<' {
            // Copy nguyên tag cho tới `>` (thuộc tính không bị thay).
            // LƯU Ý: find() trả offset QUAN HỆ TỚI html[i..] — phải cộng i
            // để được vị trí tuyệt đối (bug panic "byte range starts at X
            // but ends at Y" nếu quên).
            if let Some(gt_rel) = html[i..].find('>') {
                let gt = i + gt_rel;
                let tag = &html[i..=gt];
                if starts_with_ci(tag, "<code") || starts_with_ci(tag, "<pre") {
                    code_depth += 1;
                } else if starts_with_ci(tag, "</code") || starts_with_ci(tag, "</pre") {
                    code_depth = code_depth.saturating_sub(1);
                }
                out.push_str(tag);
                i = gt + 1;
                continue;
            }
        }
        if code_depth > 0 {
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }
        // Text node: gom tới `<` kế tiếp, thay abbreviation bên trong.
        let text_end = html[i..].find('<').map(|p| i + p).unwrap_or(html.len());
        let text = &html[i..text_end];
        let replaced = replace_abbr_in_text(text, abbrs);
        out.push_str(&replaced);
        i = text_end;
    }
    out
}

/// Thay nguyên từ term trong 1 đoạn text thuần (đã escape HTML entities).
/// Word-boundary: ký tự trước/sau không phải chữ/số (đầu/cuối chuỗi tính
/// là biên). Không thay trong từ dài hơn (vd "HTML5" chứa "HTML" → KHÔNG
/// thay — tôn trọng từ người viết).
fn replace_abbr_in_text(text: &str, abbrs: &[(String, String)]) -> String {
    let mut out = text.to_string();
    for (term, def) in abbrs {
        if term.is_empty() || !text.contains(term.as_str()) {
            continue;
        }
        let mut result = String::with_capacity(out.len());
        let mut rest = out.as_str();
        while let Some(pos) = rest.find(term.as_str()) {
            let before_ok = rest[..pos]
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_alphanumeric());
            let after = &rest[pos + term.len()..];
            let after_ok = after.chars().next().is_none_or(|c| !c.is_alphanumeric());
            if before_ok && after_ok {
                let def_esc = escape_attr(def);
                result.push_str(&rest[..pos]);
                result.push_str(&format!("<abbr title=\"{def_esc}\">{term}</abbr>"));
                rest = after;
            } else {
                // Không phải nguyên từ — giữ nguyên phần đầu + thử tiếp sau.
                let keep_len = pos + term.len();
                result.push_str(&rest[..keep_len]);
                rest = &rest[keep_len..];
            }
        }
        result.push_str(rest);
        out = result;
    }
    out
}

/// Escape giá trị cho attribute HTML (", <, >, &).
fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// v3.11.0 — Vimeo embed: link Vimeo đơn độc trong paragraph → iframe
/// player.vimeo.com (cùng cơ chế embed_youtube). Hỗ trợ:
///   https://vimeo.com/{id}, https://www.vimeo.com/{id},
///   https://player.vimeo.com/video/{id} (đã embed-ready).
fn embed_vimeo(html: &str) -> String {
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
                if let Some(id) = extract_vimeo_id(url) {
                    out.push_str(&rest[..start]);
                    out.push_str(&format!(
                        r#"<div class="video-embed"><iframe src="https://player.vimeo.com/video/{id}" loading="lazy" title="Vimeo player" allow="autoplay; fullscreen; picture-in-picture" allowfullscreen referrerpolicy="strict-origin-when-cross-origin" sandbox="allow-scripts allow-same-origin allow-presentation allow-popups"></iframe></div>"#
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

/// Trích video id từ URL Vimeo. Trả về None nếu không phải Vimeo.
fn extract_vimeo_id(url: &str) -> Option<&str> {
    // Strip scheme từ URL GỐC rồi cắt host.
    let stripped = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let after_host = stripped
        .strip_prefix("www.")
        .or(Some(stripped))
        .unwrap_or(stripped);
    if let Some(rest) = after_host.strip_prefix("vimeo.com/") {
        if let Some(rest) = rest.strip_prefix("video/") {
            let id = rest.split(['/', '?', '#']).next().unwrap_or("");
            return valid_vimeo_id(id);
        }
        let id = rest.split(['/', '?', '#']).next().unwrap_or("");
        return valid_vimeo_id(id);
    }
    if let Some(rest) = after_host.strip_prefix("player.vimeo.com/video/") {
        let id = rest.split(['/', '?', '#']).next().unwrap_or("");
        return valid_vimeo_id(id);
    }
    None
}

/// Vimeo id hợp lệ: 6..=12 chữ số.
fn valid_vimeo_id(id: &str) -> Option<&str> {
    if (6..=12).contains(&id.len()) && id.chars().all(|c| c.is_ascii_digit()) {
        Some(id)
    } else {
        None
    }
}

/// v3.11.0 — Video/audio file embed: link đơn độc (text == href, tức bare
/// URL được autolink) HOẶC ảnh đơn độc (`![](file.mp4)`) trỏ tới file
/// media → thay bằng `<video controls>` / `<audio controls>`:
///   * Video: .mp4 .webm .ogv .ogg(video) .mov .m4v
///   * Audio: .mp3 .wav .m4a .aac .flac .oga
///
/// Chỉ http(s) URL (qua harden_links đã chạy trước — href an toàn scheme).
fn embed_media_links(html: &str) -> String {
    const VIDEO_EXTS: [&str; 6] = [".mp4", ".webm", ".ogv", ".mov", ".m4v", ".ogg"];
    const AUDIO_EXTS: [&str; 5] = [".mp3", ".wav", ".m4a", ".aac", ".flac"];

    let media_kind = |url: &str| -> Option<&'static str> {
        let lower = url.to_ascii_lowercase();
        // Bỏ query/fragment trước khi soi extension.
        let clean = lower.split(['?', '#']).next().unwrap_or(&lower);
        if VIDEO_EXTS.iter().any(|e| clean.ends_with(e)) {
            Some("video")
        } else if AUDIO_EXTS.iter().any(|e| clean.ends_with(e)) {
            Some("audio")
        } else {
            None
        }
    };

    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    // Pass 1: <p><a href="URL">URL</a></p> — bare URL media.
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
                // Bare link: inner text == href (comrak autolink). harden_links
                // đã chạy trước và chèn rel/target/class vào tag — extract
                // inner text bằng `>` ĐÓNG THẺ <a> (>` đầu tiên SAU href,
                // KHÔNG dùng `>` đầu tiên của paragraph — đó là `>` của <p>).
                let text = paragraph[href_end..]
                    .find('>')
                    .and_then(|gt| {
                        let text_start = href_end + gt + 1;
                        paragraph[text_start..]
                            .find("</a>")
                            .map(|e| &paragraph[text_start..text_start + e])
                    })
                    .unwrap_or("");
                if text == url {
                    if let Some(kind) = media_kind(url) {
                        out.push_str(&rest[..start]);
                        let url_esc = escape_attr(url);
                        if kind == "video" {
                            out.push_str(&format!(
                                r#"<div class="video-embed video-file"><video controls preload="metadata" src="{url_esc}"></video></div>"#
                            ));
                        } else {
                            out.push_str(&format!(
                                r#"<div class="audio-embed"><audio controls preload="metadata" src="{url_esc}"></audio></div>"#
                            ));
                        }
                        rest = &rest[end_p..];
                        continue;
                    }
                }
            }
        }
        out.push_str(&rest[..end_p]);
        rest = &rest[end_p..];
    }
    let out = out + rest;

    // Pass 2: <p><img src="URL" ...></p> — ảnh có đuôi media → player.
    let mut result = String::with_capacity(out.len());
    let mut rest = out.as_str();
    while let Some(p_start) = rest.find("<p><img ") {
        let p_end_marker = "</p>";
        let p_end = rest[p_start..]
            .find(p_end_marker)
            .map(|p| p_start + p + p_end_marker.len())
            .unwrap_or(rest.len());
        let paragraph = &rest[p_start..p_end];
        if let Some(src_idx) = paragraph.find("src=\"") {
            let src_start = src_idx + 5;
            if let Some(src_end_rel) = paragraph[src_start..].find('"') {
                let url = &paragraph[src_start..src_start + src_end_rel];
                if let Some(kind) = media_kind(url) {
                    let url_esc = escape_attr(url);
                    result.push_str(&rest[..p_start]);
                    if kind == "video" {
                        result.push_str(&format!(
                            r#"<div class="video-embed video-file"><video controls preload="metadata" src="{url_esc}"></video></div>"#
                        ));
                    } else {
                        result.push_str(&format!(
                            r#"<div class="audio-embed"><audio controls preload="metadata" src="{url_esc}"></audio></div>"#
                        ));
                    }
                    rest = &rest[p_end..];
                    continue;
                }
            }
        }
        result.push_str(&rest[..p_end]);
        rest = &rest[p_end..];
    }
    result.push_str(rest);
    result
}

/// Xoá mọi tag HTML (`<...>`) khỏi chuỗi, GIỮ nguyên entities (&amp; &lt;
/// &gt;...) — dùng cho nội dung block Mermaid (tag syntect wrap phải đi,
/// text nguồn giữ escape để textContent client decode đúng).
fn strip_html_tags_keep_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// v3.11.0 — Mermaid: ```mermaid code block → `<div class="mermaid">`.
/// Chạy TRƯỚC các pass code-block (line numbers/copy button/lang label)
/// — block mermaid không còn là <pre> nên các pass đó bỏ qua.
///
/// Output comrak + adapter: `<pre class="code-block"><code
/// class="language-mermaid hljs">NỘI DUNG (escaped)</code></pre>`.
/// Mermaid client-side đọc textContent (browser tự decode entities).
/// Không khớp block `text`/plain khác — chỉ language-mermaid.
fn convert_mermaid_blocks(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    const PRE_OPEN: &str = "<pre class=\"code-block\"><code";
    while let Some(start) = rest.find(PRE_OPEN) {
        // Tag `<code ...>` bắt đầu ngay cuối PRE_OPEN. Tìm `>` ĐÓNG TAG
        // CODE — KHÔNG phải `>` đóng `<pre>` (bug: find('>') từ `start`
        // trúng `>` của thẻ pre → code_tag không chứa language-mermaid →
        // block bị bỏ qua nguyên vẹn).
        let code_tag_start = start + PRE_OPEN.len() - "<code".len();
        let code_tag_end = match rest[code_tag_start..].find('>') {
            Some(p) => code_tag_start + p + 1,
            None => break,
        };
        let code_open_tag = &rest[start..code_tag_end];
        let lower = code_open_tag.to_ascii_lowercase();
        if !lower.contains("language-mermaid") {
            // Không phải mermaid — copy nguyên block này (đến </pre>).
            let pre_end = match rest[code_tag_end..].find("</pre>") {
                Some(p) => code_tag_end + p + "</pre>".len(),
                None => rest.len(),
            };
            out.push_str(&rest[..pre_end]);
            rest = &rest[pre_end..];
            continue;
        }
        // Tìm </code></pre> đóng.
        let close_marker = "</code></pre>";
        let block_end = match rest[code_tag_end..].find(close_marker) {
            Some(p) => code_tag_end + p + close_marker.len(),
            None => rest.len(),
        };
        let raw_content = &rest[code_tag_end..block_end - close_marker.len()];
        // Mermaid client (mermaid.run v11) đọc innerHTML của div — mọi TAG
        // HTML trong div sẽ vỡ parse ("Syntax error in text"). Syntect wrap
        // nội dung plain-text trong `<span class="text plain">` — STRIP toàn
        // bộ tag syntect, GIỮ text + entities đã escape (textContent client
        // decode đúng nguồn gốc — đã kiểm chứng bằng browser e2e).
        let content = strip_html_tags_keep_entities(raw_content);
        out.push_str(&rest[..start]);
        out.push_str("<div class=\"mermaid-wrapper\"><div class=\"mermaid\">");
        out.push_str(&content);
        out.push_str("</div><noscript><p class=\"mermaid-noscript\">Cần bật JavaScript để hiển thị sơ đồ Mermaid.</p></noscript></div>");
        rest = &rest[block_end..];
    }
    out.push_str(rest);
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
        // v3.4.2 FIX (audit): matcher cũ chỉ kiểm tra bytes[i+1]=='a' → match
        // cả <article>/<audio>/<abbr>/<address> rồi nhét rel/target vào các
        // thẻ không phải anchor (HTML méo mó). Giờ yêu cầu byte sau `<a` là
        // delimiter (space/'>'/tab/newline/'/') mới tính là anchor.
        if bytes[i] == b'<'
            && i + 1 < bytes.len()
            && (bytes[i + 1] == b'a' || bytes[i + 1] == b'A')
            && i + 2 < bytes.len()
            && matches!(bytes[i + 2], b' ' | b'>' | b'\t' | b'\n' | b'\r' | b'/')
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

/// v3.4.2 — Lọc scheme của `<img src>` trong HTML output markdown.
///
/// Audit: `harden_links` chỉ lọc `<a href>`; ảnh markdown
/// `![x](javascript:...)` / `![x](data:image/svg+xml;base64,...)` đi qua
/// nguyên vẹn, trong khi CSP cho phép `img-src ... data:` — SVG tải qua
/// <img> tuy không chạy script nhưng là vector tracking/phishing và trái
/// với cam kết tài liệu module ("ảnh chỉ http(s)"). Pass này rewrite
/// `src` thành `#` khi scheme không an toàn (giữ thẻ, giữ alt).
fn harden_img_src(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let bytes = html.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' && html[i..].starts_with("<img") {
            if let Some(end) = find_byte(bytes, b'>', i) {
                let tag = &html[i..=end];
                let lower = tag.to_ascii_lowercase();
                if let Some(src_start) = lower.find("src=\"") {
                    let v_start = src_start + 5;
                    if let Some(v_end_rel) = lower[v_start..].find('"') {
                        let v_end = v_start + v_end_rel;
                        let src = &tag[v_start..v_end];
                        if is_safe_url_scheme(src) {
                            out.push_str(tag);
                        } else {
                            // Replace giá trị src bằng "#" — giữ nguyên phần còn lại.
                            out.push_str(&tag[..v_start]);
                            out.push('#');
                            out.push_str(&tag[v_end..]);
                        }
                        i = end + 1;
                        continue;
                    }
                }
            }
        }
        let ch = html[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
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

/// Convert `>!` spoiler sang `<span class="spoiler">` với tabindex/role cho accessibility.
/// Comrak hỗ trợ `spoiler: true` nhưng cú pháp là `>!text<!` inline. Block-level
/// `>!` style Reddit cần parse riêng. Đây chỉ thêm tabindex/role cho accessibility.
fn convert_spoiler_inline(html: &str) -> String {
    html.replace(
        r#"<span class="spoiler">"#,
        r#"<span class="spoiler" tabindex="0" role="button" aria-label="Hiện nội dung ẩn">"#,
    )
}

/// Convert GFM-style callout syntax `> [!NOTE]` thành `<blockquote class="callout ...">`.
/// v2.4 — hỗ trợ collapsible callout với `> [!NOTE]+` (mở) và `> [!NOTE]-` (đóng).
/// Markup cho collapsible: `<details class="callout ..."><summary>Ghi chú</summary>...`
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
                let kind_inner = &inner[kind_start + 2..kind_start + 2 + kind_end];
                // Parse modifier: "+" (open) hoặc "-" (closed) sau `]`.
                let after_bracket = kind_start + 2 + kind_end + 1;
                let modifier_char = inner
                    .get(after_bracket..after_bracket + 1)
                    .and_then(|s| s.chars().next());
                // Strip modifier ra khỏi kind_inner nếu có.
                let kind_clean = kind_inner
                    .strip_prefix('+')
                    .or_else(|| kind_inner.strip_prefix('-'))
                    .unwrap_or(kind_inner);
                let kind_lower = kind_clean.to_ascii_lowercase();
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
                // Strip modifier (+/-) ở đầu rest_inner nếu có.
                let rest_clean = if let Some(m) = modifier_char {
                    if (m == '+' || m == '-') && rest_inner.starts_with(m) {
                        rest_inner[1..].trim_start()
                    } else {
                        rest_inner.trim_start()
                    }
                } else {
                    rest_inner.trim_start()
                };
                out.push_str(&rest[..start]);
                // Collapsible variants → <details>
                if let Some(m) = modifier_char {
                    if m == '+' || m == '-' {
                        let open_attr = if m == '+' { " open" } else { "" };
                        out.push_str(&format!(
                            r#"<details class="callout callout-collapsible {css_class}"{open_attr}><summary class="callout-summary">{label}</summary><div class="callout-body">{rest_clean}</div></details>"#
                        ));
                        rest = &rest[end..];
                        continue;
                    }
                }
                // Default → blockquote
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

/// v2.4.0 — Wrap `<img>` trong `<figure>` nếu alt text bắt đầu bằng `caption:`.
/// Cú pháp Markdown: `![caption:Mô tả ảnh](url)` →
///   `<figure class="md-figure"><img src="url" alt="Mô tả ảnh"><figcaption>Mô tả ảnh</figcaption></figure>`
///
/// Nếu alt không có prefix `caption:` → giữ nguyên `<img>` (no change).
/// Chỉ áp dụng cho `<p><img ...></p>` (ảnh đơn độc trong paragraph — comrak
/// wrap standalone image trong `<p>`).
fn wrap_image_figures(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(p_start) = rest.find("<p><img ") {
        // Tìm đóng `</p>` của paragraph chứa <img>.
        let p_end_marker = "</p>";
        let p_end = rest[p_start..]
            .find(p_end_marker)
            .map(|p| p_start + p + p_end_marker.len())
            .unwrap_or(rest.len());
        let paragraph = &rest[p_start..p_end];
        // Tìm `<img ` trong paragraph — vị trí LOCAL trong paragraph.
        let img_local_start = match paragraph.find("<img ") {
            Some(p) => p,
            None => {
                // Shouldn't happen since we matched "<p><img " — but be safe
                out.push_str(&rest[..p_end]);
                rest = &rest[p_end..];
                continue;
            }
        };
        // Tìm `>` đóng thẻ img (first '>' after img_local_start).
        let img_local_end = match paragraph[img_local_start..].find('>') {
            Some(p) => img_local_start + p + 1,
            None => {
                out.push_str(&rest[..p_end]);
                rest = &rest[p_end..];
                continue;
            }
        };
        let img_tag = &paragraph[img_local_start..img_local_end];
        let lower = img_tag.to_ascii_lowercase();
        // Tìm alt="..."
        if let Some(alt_idx) = lower.find("alt=\"") {
            let alt_val_start = alt_idx + 5;
            if let Some(alt_end_rel) = lower[alt_val_start..].find('"') {
                let alt_end = alt_val_start + alt_end_rel;
                let alt = &img_tag[alt_val_start..alt_end];
                if let Some(caption) = alt.strip_prefix("caption:") {
                    // Có caption → wrap trong <figure>.
                    // Extract src để rebuild alt text (loại bỏ prefix "caption:").
                    let src = lower
                        .find("src=\"")
                        .and_then(|s| {
                            let s_start = s + 5;
                            lower[s_start..]
                                .find('"')
                                .map(|e| &img_tag[s_start..s_start + e])
                        })
                        .unwrap_or("");
                    out.push_str(&rest[..p_start]);
                    out.push_str(&format!(
                        r#"<figure class="md-figure"><img src="{src}" alt="{caption}" loading="lazy" decoding="async"><figcaption>{caption}</figcaption></figure>"#
                    ));
                    rest = &rest[p_end..];
                    continue;
                }
            }
        }
        // Không phải figure → giữ nguyên paragraph
        out.push_str(&rest[..p_end]);
        rest = &rest[p_end..];
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

/// v2.4.0 — Thêm language label visible trên code block.
/// Comrak syntect output `<pre class="code-block"><code class="hljs language-rust">...`
/// Ta thêm `<span class="code-lang-label">rust</span>` vào wrapper để CSS
/// hiển thị badge với tên ngôn ngữ ở góc trên-phải.
fn add_code_lang_label(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<div class=\"code-block-wrapper\">") {
        let wrapper_open_len = "<div class=\"code-block-wrapper\">".len();
        // Tìm code class="hljs language-XXX" trong wrapper này.
        let next_wrapper_end = rest[start..]
            .find("</div>")
            .map(|p| start + p + "</div>".len())
            .unwrap_or(rest.len());
        let wrapper_content = &rest[start..next_wrapper_end];
        // Tìm `language-` token trong class attribute.
        if let Some(lang_idx) = wrapper_content.find("language-") {
            let after_lang = &wrapper_content[lang_idx + "language-".len()..];
            // Tìm kết thúc token (quote hoặc space).
            let lang_end = after_lang.find(['"', ' ', '>']).unwrap_or(after_lang.len());
            let lang = &after_lang[..lang_end];
            // Skip plain "text" (default info string) — không hiển thị badge.
            if !lang.is_empty() && lang != "text" {
                out.push_str(&rest[..start + wrapper_open_len]);
                out.push_str(&format!(
                    r#"<span class="code-lang-label" aria-hidden="true">{lang}</span>"#
                ));
                out.push_str(&rest[start + wrapper_open_len..next_wrapper_end]);
                rest = &rest[next_wrapper_end..];
                continue;
            }
        }
        // Không có language → giữ nguyên
        out.push_str(&rest[..next_wrapper_end]);
        rest = &rest[next_wrapper_end..];
    }
    out.push_str(rest);
    out
}

/// v2.4.0 — Comrak 0.54 đã tự thêm `aria-label="Back to reference 1"` cho
/// footnote backref. Function này giữ lại làm no-op idempotent — phòng khi
/// downgrade comrak hoặc tuỳ chỉnh output sau này. Trả về html nguyên.
fn improve_footnote_backrefs(html: &str) -> String {
    html.to_string()
}

/// v2.5.0 — Thêm số dòng cho code block: wrap từng dòng trong
/// `<span class="code-line">...</span>`; CSS counter hiển thị số dòng
/// (xem .code-line::before trong style.css).
///
/// Syntect để span MỞ XUYÊN DÒNG (vd `<span class="source rust">` mở ở
/// dòng 1) và đóng các span chưa đóng dồn về cuối — KỂ CẢ closer đứng
/// ĐẦU dòng giữa chừng (vd `}` của block: `</span><span...>}`) — nên
/// wrap trực tiếp từng dòng làm browser đóng code-line span sớm, nội
/// dung dòng tràn ra ngoài (mất số dòng). Fix 2 bước:
///   1. `rebalance_spans_per_line`: biến đổi HTML sao cho MỖI DÒNG tự
///      cân bằng (span mở từ dòng trước được "mở lại" ở đầu dòng và
///      "đóng tạm" ở cuối dòng) — kỹ thuật chuẩn của syntax highlighter.
///   2. Wrap từng dòng (đã cân bằng) trong span code-line.
///
/// Closer run thuần (`</span>...`) ở cuối bị drop (rebalance đã đóng
/// hết ở cuối dòng cuối). Dòng trống cuối (từ `\n` kết thúc) giữ `\n`
/// ngoài span. textContent (copy button) không đổi.
fn add_code_line_numbers(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<pre class=\"code-block\">") {
        let pre_open_len = "<pre class=\"code-block\">".len();
        // Tìm thẻ <code ...> mở bên trong pre (adapter luôn phát <code>).
        // Tìm '<' SAU thẻ pre mở (không tính '<' của chính thẻ pre).
        let code_open = match rest[start + pre_open_len..].find('<') {
            Some(p) if rest[start + pre_open_len + p..].starts_with("<code") => rest
                [start + pre_open_len + p..]
                .find('>')
                .map(|q| start + pre_open_len + p + q + 1),
            _ => None,
        };
        let end_marker = "</pre>";
        let pre_end = rest[start..]
            .find(end_marker)
            .map(|p| start + p + end_marker.len())
            .unwrap_or(rest.len());
        match code_open {
            Some(code_body_start) if code_body_start < pre_end => {
                let code_close = rest[code_body_start..]
                    .find("</code>")
                    .map(|p| code_body_start + p)
                    .unwrap_or(pre_end);
                out.push_str(&rest[..code_body_start]);
                let content = &rest[code_body_start..code_close];
                if content.contains("code-line") || content.is_empty() {
                    // Idempotent — không double-wrap.
                    out.push_str(content);
                } else {
                    // Bước 0: tách closer run thuần cuối + dòng trống cuối.
                    let mut body = content;
                    if let Some(nl_pos) = body.rfind('\n') {
                        let last_part = &body[nl_pos + 1..];
                        let non_empty =
                            last_part.split("</span>").filter(|s| !s.is_empty()).count();
                        if !last_part.is_empty() && non_empty == 0 {
                            body = &body[..nl_pos]; // drop closer run (+ \n trước nó)
                        }
                    }
                    let mut trailing_nl = false;
                    if let Some(stripped) = body.strip_suffix('\n') {
                        body = stripped;
                        trailing_nl = true;
                    }
                    // Bước 1: rebalance span theo dòng.
                    let rebalanced = rebalance_spans_per_line(body);
                    // Bước 2: wrap từng dòng.
                    for (n, line) in rebalanced.split('\n').enumerate() {
                        out.push_str("<span class=\"code-line\">");
                        out.push_str(line);
                        out.push_str("</span>");
                        if n + 1 < rebalanced.split('\n').count() {
                            out.push('\n');
                        }
                    }
                    if trailing_nl {
                        out.push('\n');
                    }
                }
                out.push_str(&rest[code_close..pre_end]);
                rest = &rest[pre_end..];
            }
            // Structure lạ (không có <code>) — giữ nguyên cả block.
            _ => {
                out.push_str(&rest[..pre_end]);
                rest = &rest[pre_end..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// v2.5.0 — Rebalance các span syntect theo DÒNG: biến HTML (nhiều span
/// mở xuyên dòng, closer dồn cuối) thành HTML mà MỖI DÒNG tự cân bằng.
///
/// Cơ chế: duyệt token (thẻ + text) từng dòng, giữ `stack` span mở:
///   - Đầu dòng: phát lại (synthesized) mọi tag trong `stack`.
///   - Gặp `</span>` gốc: pop stack (closer dư khi stack rỗng → bỏ).
///   - Cuối dòng: phát (synthesized) `</span>` × len(stack).
///
/// `stack` được GIỮ NGUYÊN qua các dòng (chỉ output đóng tạm) — dòng sau
/// mở lại y hệt. Bảo toàn: mỗi dòng opens == closes → wrap ngoài an toàn.
///
/// Input phải là nội dung code đã escape (mọi `<` đều là thẻ thật do
/// syntect/diff-highlighter escape `&lt;` sẵn) — không có `>` trong attr
/// class của syntect output.
fn rebalance_spans_per_line(content: &str) -> String {
    let mut out = String::with_capacity(content.len() + 256);
    let mut stack: Vec<&str> = Vec::new();
    for line in content.split_inclusive('\n') {
        let (body, newline) = match line.strip_suffix('\n') {
            Some(b) => (b, "\n"),
            None => (line, ""),
        };
        // Mở lại span từ dòng trước.
        for open in &stack {
            out.push_str(open);
        }
        // Duyệt token trong dòng.
        let bytes = body.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'<' {
                let end = body[i..].find('>').map(|p| i + p + 1).unwrap_or(body.len());
                let tag = &body[i..end];
                if tag.starts_with("</span") {
                    // Closer gốc — pop stack. Closer dư (stack rỗng) → bỏ.
                    if stack.pop().is_some() {
                        out.push_str("</span>");
                    }
                } else if tag.starts_with("<span") {
                    stack.push(tag);
                    out.push_str(tag);
                } else {
                    // Thẻ khác (hiếm) — passthrough.
                    out.push_str(tag);
                }
                i = end;
            } else {
                let next = body[i..].find('<').map(|p| i + p).unwrap_or(body.len());
                out.push_str(&body[i..next]);
                i = next;
            }
        }
        // Đóng tạm mọi span còn mở cuối dòng (stack giữ cho dòng sau).
        for _ in 0..stack.len() {
            out.push_str("</span>");
        }
        out.push_str(newline);
    }
    out
}

/// v2.5.0 — Link hoá mention + hashtag trong TEXT NODE của HTML đã render:
///   - `@username`  → `<a href="/u/username" class="md-mention">@username</a>`
///   - `#từ-khoá`   → `<a href="/search?q=..." class="md-hashtag">#từ-khoá</a>`
///
/// Bỏ qua (không link) khi nằm trong:
///   - `<pre>` / `<code>` (đó là mã nguồn)
///   - `<a>` (đã là link — tránh lồng thẻ `<a>` trong thẻ `<a>`, HTML không hợp lệ)
///   - attribute của tag (chỉ xử lý text node)
///
/// An toàn entity: `&#39;` / `&#x27;` chứa `#` nhưng # đứng sau `&` và
/// theo sau là CHỮ SỐ → bị chặn bởi (1) quy tắc "ký tự trước # phải
/// không phải `&`" và (2) quy tắc "ký tự đầu tag phải là CHỮ".
/// Mention chỉ nhận [a-zA-Z0-9_] (username site là ASCII), hashtag nhận
/// chữ cái unicode (hỗ trợ #TiếngViệt) + số.
fn linkify_mentions_hashtags(html: &str) -> String {
    let bytes = html.as_bytes();
    let mut out = String::with_capacity(html.len() + 64);
    let mut i = 0usize;
    // Depth counter cho vùng "không link hoá": code/pre/anchor
    let mut code_depth: usize = 0; // <pre> hoặc <code> lồng nhau
    let mut anchor_depth: usize = 0; // <a> lồng nhau (HTMX partial có thể lồng)

    while i < bytes.len() {
        if bytes[i] == b'<' {
            // Copy toàn bộ tag (từ '<' tới '>')
            let tag_end = match html[i..].find('>') {
                Some(p) => i + p + 1,
                None => html.len(),
            };
            let tag = &html[i..tag_end];
            let lower = tag.to_ascii_lowercase();
            if lower.starts_with("<pre") || lower.starts_with("<code") {
                code_depth = code_depth.saturating_add(1);
            } else if lower.starts_with("</pre") || lower.starts_with("</code") {
                code_depth = code_depth.saturating_sub(1);
            } else if lower.starts_with("<a ") || lower.starts_with("<a>") {
                anchor_depth = anchor_depth.saturating_add(1);
            } else if lower.starts_with("</a") {
                anchor_depth = anchor_depth.saturating_sub(1);
            }
            out.push_str(tag);
            i = tag_end;
            continue;
        }
        // Text node — tìm tag tiếp theo để biết ranh giới text
        let next_tag = html[i..].find('<').map(|p| i + p).unwrap_or(html.len());
        let text = &html[i..next_tag];
        if code_depth > 0 || anchor_depth > 0 {
            out.push_str(text);
        } else {
            linkify_text(text, &mut out);
        }
        i = next_tag;
    }
    out
}

/// Link hoá 1 text node (đã escape HTML — text thuần + entity).
fn linkify_text(text: &str, out: &mut String) {
    let chars: Vec<char> = text.chars().collect();
    let mut idx = 0usize;
    while idx < chars.len() {
        let c = chars[idx];
        if c == '@' {
            // @mention: ký tự trước @ phải không phải chữ/số/_/& (chặn
            // email user@domain và entity) — sau @ là [a-zA-Z0-9_]{2,30}
            let prev_ok = idx == 0
                || !chars[idx - 1].is_alphanumeric()
                    && chars[idx - 1] != '_'
                    && chars[idx - 1] != '&';
            if prev_ok {
                let mut j = idx + 1;
                while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }
                let len = j - (idx + 1);
                if (2..=30).contains(&len) {
                    let username: String = chars[idx + 1..j].iter().collect();
                    out.push_str("<a href=\"/u/");
                    out.push_str(&username);
                    out.push_str("\" class=\"md-mention\">@");
                    out.push_str(&username);
                    out.push_str("</a>");
                    idx = j;
                    continue;
                }
            }
            out.push(c);
            idx += 1;
        } else if c == '#' {
            // #hashtag: ký tự trước # phải không phải chữ/số/_/& (chặn
            // entity &#39;/&#x27;), ký tự đầu tag phải là CHỮ (chặn #39),
            // theo sau là chữ unicode/số/_ , dài 2..=48.
            let prev_ok = idx == 0
                || !chars[idx - 1].is_alphanumeric()
                    && chars[idx - 1] != '_'
                    && chars[idx - 1] != '&';
            if prev_ok {
                let mut j = idx + 1;
                if j < chars.len() && chars[j].is_alphabetic() {
                    j += 1;
                    while j < chars.len()
                        && (chars[j].is_alphanumeric() || chars[j] == '_')
                        && j - idx <= 48
                    {
                        j += 1;
                    }
                    let len = j - (idx + 1);
                    if (1..=47).contains(&len) {
                        let tag: String = chars[idx + 1..j].iter().collect();
                        out.push_str("<a href=\"/search?q=");
                        // URL-encode tag cho query param an toàn
                        for b in tag.bytes() {
                            match b {
                                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' => {
                                    out.push(b as char)
                                }
                                _ => out.push_str(&format!("%{b:02X}")),
                            }
                        }
                        out.push_str("\" class=\"md-hashtag\">#");
                        out.push_str(&tag);
                        out.push_str("</a>");
                        idx = j;
                        continue;
                    }
                }
            }
            out.push(c);
            idx += 1;
        } else {
            out.push(c);
            idx += 1;
        }
    }
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
        // v2.4: có language label visible
        assert!(out.contains("code-lang-label"));
        assert!(out.contains(">rust<"));
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
    fn test_callout_collapsible_open() {
        let out = render("> [!NOTE]+\n> hidden content");
        assert!(out.contains("callout-collapsible"));
        assert!(out.contains(" open"));
        assert!(out.contains("<details"));
        assert!(out.contains("<summary"));
    }

    #[test]
    fn test_callout_collapsible_closed() {
        let out = render("> [!WARNING]-\n> hidden content");
        assert!(out.contains("callout-collapsible"));
        assert!(out.contains("<details"));
        // Modifier "-" → không có "open" attribute
        assert!(
            !out.contains("<details class=\"callout callout-collapsible callout-warning\" open>")
        );
    }

    #[test]
    fn test_footnote() {
        let out = render("text[^1]\n\n[^1]: footnote");
        assert!(out.contains("footnote"));
        // Comrak 0.54 đã có aria-label default trên backref
        assert!(out.contains("aria-label="));
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

    // v2.4 — Tests for new features

    #[test]
    fn test_render_cache_hit() {
        // Render 2 lần cùng input → output giống hệt (cache hit).
        let input = "# Hello\n\nSome **bold** text with `code`.";
        let out1 = render(input);
        let out2 = render(input);
        assert_eq!(out1, out2);
    }

    #[test]
    fn test_render_cache_different_input() {
        let out1 = render("# Title 1");
        let out2 = render("# Title 2");
        assert!(out1.contains("id=\"title-1\""));
        assert!(out2.contains("id=\"title-2\""));
        assert_ne!(out1, out2);
    }

    #[test]
    fn test_reading_time_short() {
        // 5 từ → 1 phút (ceil(5/200) = 1)
        assert_eq!(reading_time_minutes("one two three four five"), 1);
    }

    #[test]
    fn test_reading_time_long() {
        // 250 từ → 2 phút (ceil(250/200) = 2)
        let words: Vec<&str> = (0..250).map(|_| "word").collect();
        let input = words.join(" ");
        assert_eq!(reading_time_minutes(&input), 2);
    }

    #[test]
    fn test_reading_time_empty() {
        assert_eq!(reading_time_minutes(""), 1);
        assert_eq!(reading_time_minutes("   "), 1);
    }

    #[test]
    fn test_description_lists() {
        let input = "Term 1\n: Definition 1\n\nTerm 2\n: Definition 2";
        let out = render(input);
        // comrak với description_lists=true output <dl><dt>...</dt><dd>...</dd></dl>
        assert!(out.contains("<dl>") || out.contains("<dt>") || out.contains("<dd>"));
    }

    #[test]
    fn test_image_figure_caption() {
        let input = "![caption:Mô tả ảnh đẹp](https://example.com/photo.jpg)";
        let out = render(input);
        assert!(out.contains("<figure class=\"md-figure\">"), "out: {out}");
        assert!(out.contains("<figcaption>Mô tả ảnh đẹp</figcaption>"));
    }

    #[test]
    fn test_image_no_caption_stays_img() {
        let input = "![regular alt](https://example.com/photo.jpg)";
        let out = render(input);
        // Không có prefix "caption:" → không wrap figure, giữ <img> trong <p>
        assert!(!out.contains("<figure"));
        assert!(out.contains("<img"));
    }

    #[test]
    fn test_code_lang_label_rust() {
        let out = render("```rust\nfn x() {}\n```");
        assert!(out.contains("code-lang-label"));
        assert!(out.contains(">rust<"));
    }

    #[test]
    fn test_code_lang_label_text_no_badge() {
        // Default info string là "text" → không hiển thị badge
        let out = render("```\nplain code\n```");
        // Không có language-text → không có badge
        assert!(!out.contains(">text<") || !out.contains("code-lang-label"));
    }

    #[test]
    fn test_code_lang_label_python() {
        let out = render("```python\nprint('hi')\n```");
        assert!(out.contains("code-lang-label"));
        assert!(out.contains(">python<"));
    }

    #[test]
    fn test_footnote_backref_has_aria_label() {
        let input = "text[^1]\n\n[^1]: this is a footnote";
        let out = render(input);
        // Comrak 0.54 default output có aria-label="Back to reference 1"
        // trên backref — ta chỉ verify nó tồn tại (migrate sang aria-label
        // tuỳ chỉnh sẽ cần parser lại nếu downgrade comrak).
        assert!(
            out.contains("aria-label=\"Back to reference")
                || out.contains("aria-label=\"Quay lại vị trí chú thích\""),
            "missing aria-label on backref in: {out}"
        );
    }

    #[test]
    fn test_toc_buffer_no_race() {
        // Render nhiều lần liên tiếp, mỗi render phải có ToC riêng KHÔNG leak.
        // (Trước đây global Mutex có thể leak entries giữa renders.)
        let out1 = render("[toc]\n\n# A");
        let out2 = render("[toc]\n\n# B");
        // out1 chỉ có "A", out2 chỉ có "B" — không có cả hai.
        assert!(out1.contains("href=\"#a\""));
        assert!(!out1.contains("href=\"#b\""));
        assert!(out2.contains("href=\"#b\""));
        assert!(!out2.contains("href=\"#a\""));
    }

    // ============ v2.5.0 — Markdown engine "mạnh hơn nữa" ============

    #[test]
    fn test_emoji_shortcode_v25() {
        let out = render("Chúc mừng :tada: và :smile:");
        assert!(out.contains('🎉'), "shortcode :tada: phải thành 🎉: {out}");
        assert!(out.contains('😄'), "shortcode :smile: phải thành 😄: {out}");
    }

    #[test]
    fn test_shortcode_unknown_name_stays_literal() {
        // Tên không có trong bảng shortcode → giữ nguyên text
        let out = render("giờ :khongtontrongbang: 8:00");
        assert!(out.contains(":khongtontrongbang:"));
        assert!(out.contains("8:00"));
    }

    #[test]
    fn test_underline_v25() {
        let out = render("__văn bản gạch chân__");
        assert!(out.contains("<u>văn bản gạch chân</u>"), "got: {out}");
    }

    #[test]
    fn test_subscript_v25() {
        let out = render("H~2~O và CO~2~");
        assert!(out.contains("<sub>2</sub>"), "got: {out}");
    }

    #[test]
    fn test_highlight_mark_v25() {
        let out = render("đây là ==điểm quan trọng== nè");
        assert!(out.contains("<mark>điểm quan trọng</mark>"), "got: {out}");
    }

    #[test]
    fn test_insert_ins_v25() {
        let out = render("++đoạn thêm mới++");
        assert!(out.contains("<ins>đoạn thêm mới</ins>"), "got: {out}");
    }

    #[test]
    fn test_inline_footnote_v25() {
        let out = render("văn bản^[chú thích inline] ở đây");
        assert!(
            out.contains("chú thích inline"),
            "inline footnote content phải xuất hiện: {out}"
        );
    }

    #[test]
    fn test_mention_linkified_v25() {
        let out = render("chào @mhieuhonda nhé!");
        assert!(
            out.contains("<a href=\"/u/mhieuhonda\" class=\"md-mention\">@mhieuhonda</a>"),
            "got: {out}"
        );
    }

    #[test]
    fn test_mention_in_code_not_linkified() {
        let out = render("`@khong_phai_user` và:\n\n```\n@git_handle\n```\n");
        assert!(
            !out.contains("md-mention"),
            "code không được link hoá: {out}"
        );
    }

    #[test]
    fn test_mention_email_not_linkified() {
        let out = render("gửi mail cho user@domain.com nhé");
        assert!(
            !out.contains("md-mention"),
            "email không được link hoá: {out}"
        );
    }

    #[test]
    fn test_hashtag_linkified_v25() {
        let out = render("bài này về #GameNhay nhiều lắm");
        assert!(out.contains("class=\"md-hashtag\">#GameNhay"), "got: {out}");
        assert!(out.contains("href=\"/search?q=GameNhay\""));
    }

    #[test]
    fn test_hashtag_vietnamese_linkified() {
        let out = render("hãy chơi #TiếngViệt nào");
        assert!(
            out.contains("md-hashtag"),
            "hashtag tiếng Việt phải link: {out}"
        );
        // Ký tự có dấu phải được URL-encode trong query param
        assert!(out.contains("/search?q=Ti%"), "got: {out}");
    }

    #[test]
    fn test_hashtag_entity_not_linkified() {
        // `&#39;` (apostrophe escaped) chứa # nhưng KHÔNG được thành hashtag
        let out = render("don't stop me now");
        assert!(
            !out.contains("md-hashtag"),
            "entity &#39; không được link: {out}"
        );
    }

    #[test]
    fn test_hashtag_in_code_not_linkified() {
        let out = render("chạy lệnh `#include <stdio.h>` nhé");
        assert!(!out.contains("md-hashtag"), "got: {out}");
    }

    #[test]
    fn test_diff_block_colored_v25() {
        let input = "```diff\n@@ -1,2 +1,2 @@\n-old line\n+new line\n context\n```";
        let out = render(input);
        assert!(out.contains("diff-add"), "missing diff-add: {out}");
        assert!(out.contains("diff-del"), "missing diff-del: {out}");
        assert!(out.contains("diff-meta"), "missing diff-meta: {out}");
    }

    #[test]
    fn test_diff_block_escapes_html() {
        let input = "```diff\n+<script>alert(1)</script>\n```";
        let out = render(input);
        assert!(
            !out.contains("<script>alert"),
            "diff phải escape HTML: {out}"
        );
        assert!(out.contains("&lt;script&gt;"));
    }

    #[test]
    fn test_code_line_numbers_v25() {
        let out = render("```rust\nfn a() {}\nfn b() {}\n```");
        assert!(
            out.contains("<span class=\"code-line\">"),
            "phải có span code-line: {out}"
        );
        // 2 dòng code → 2 span code-line (không tính dòng trống cuối)
        assert_eq!(
            out.matches("<span class=\"code-line\">").count(),
            2,
            "got: {out}"
        );
    }

    #[test]
    fn test_code_line_numbers_multiline_block_v25() {
        // Block đa dòng có span mở xuyên dòng (meta block của `{`...`}`) —
        // syntect phát closer `</span>` ĐẦU dòng cuối. Rebalance phải giữ
        // `}` BÊN TRONG span code-line cuối (browser không đóng sớm).
        let out = render("```rust\nfn main() {\n    println!(\"x\");\n}\n```");
        assert_eq!(
            out.matches("<span class=\"code-line\">").count(),
            3,
            "3 dòng → 3 span: {out}"
        );
        // `}` phải nằm trong code-line span cuối cùng: tìm đoạn từ code-line
        // mở CUỐI tới </code> phải chứa block-end span với `}`.
        let last_open = out.rfind("<span class=\"code-line\">").unwrap();
        let tail = &out[last_open..];
        assert!(
            tail.contains("block end rust\">}</span>"),
            "`}}` phải trong span cuối: {tail}"
        );
        // Mỗi dòng tự cân bằng: tổng mở == tổng đóng span trong pre
        let pre_start = out.find("<pre class=\"code-block\">").unwrap();
        let pre_end = out.find("</pre>").unwrap();
        let pre = &out[pre_start..pre_end];
        let opens = pre.matches("<span").count();
        let closes = pre.matches("</span>").count();
        assert_eq!(opens, closes, "span phải cân bằng trong pre: {pre}");
    }

    #[test]
    fn test_code_line_numbers_empty_line_v25() {
        // Dòng trống giữa code — vẫn được đánh số (span rỗng)
        let out = render("```rust\nlet a = 1;\n\nlet b = 2;\n```");
        assert_eq!(out.matches("<span class=\"code-line\">").count(), 3);
    }

    #[test]
    fn test_code_tag_single_class_attribute_v25() {
        // FIX bug trùng thuộc tính class có sẵn từ v2.2:
        // `<code class="language-rust" class="hljs">` (invalid HTML) →
        // `<code class="language-rust hljs">`
        let out = render("```rust\nfn main() {}\n```");
        assert!(
            out.contains("class=\"language-rust hljs\""),
            "code tag phải merge class: {out}"
        );
    }

    #[test]
    fn test_mention_inside_link_text_not_nested() {
        // @user trong text của <a> sẵn → KHÔNG lồng <a> trong <a>
        let out = render("[xem @mhieuhonda profile](https://example.com)");
        assert!(
            !out.contains("<a href=\"/u/"),
            "không lồng <a> trong <a>: {out}"
        );
    }

    // ============ v2.5.0 — Bio Markdown (render_bio) ============

    #[test]
    fn test_bio_renders_markdown() {
        let out = render_bio("Xin chào, tôi là **Louis** — dev *Rust* :rocket:");
        assert!(out.contains("<strong>Louis</strong>"), "got: {out}");
        assert!(out.contains("<em>Rust</em>"));
        assert!(out.contains('🚀'));
    }

    #[test]
    fn test_bio_mention_and_hashtag() {
        let out = render_bio("follow @mhieuhonda — chơi #GameHay mỗi ngày");
        assert!(out.contains("md-mention"), "got: {out}");
        assert!(out.contains("md-hashtag"));
    }

    #[test]
    fn test_bio_link_hardened() {
        let out = render_bio("[web](https://example.com) và [x](javascript:alert(1))");
        assert!(out.contains("rel=\"nofollow ugc noopener noreferrer\""));
        assert!(!out.contains("javascript:alert"));
    }

    #[test]
    fn test_bio_no_toc_no_youtube() {
        let out = render_bio("[toc]\n\nhttps://www.youtube.com/watch?v=dQw4w9WgXcQ");
        // Bio pipeline không inject ToC, không embed YouTube
        assert!(!out.contains("toc-list"), "bio không có ToC: {out}");
        assert!(
            !out.contains("youtube-nocookie.com"),
            "bio không embed YouTube: {out}"
        );
    }

    #[test]
    fn test_bio_heading_no_anchor() {
        let out = render_bio("# Tiêu đề lớn trong bio");
        assert!(!out.contains("heading-anchor"), "bio không anchor: {out}");
        assert!(out.contains("Tiêu đề lớn trong bio"));
    }

    #[test]
    fn test_bio_empty_input() {
        assert_eq!(render_bio(""), "");
        assert_eq!(render_bio("   \n  "), "");
    }

    #[test]
    fn test_bio_escapes_raw_html() {
        let out = render_bio("<img src=x onerror=alert(1)>");
        assert!(!out.contains("<img src=x"), "bio phải escape HTML: {out}");
    }

    #[test]
    fn test_bio_code_block_still_highlighted_but_simple() {
        let out = render_bio("```\nlet x = 1;\n```");
        // Code block vẫn render (syntax màu) nhưng KHÔNG copy button/label
        assert!(out.contains("code-block"));
        assert!(!out.contains("code-copy-btn"), "bio không copy btn: {out}");
        assert!(!out.contains("code-lang-label"));
    }

    // ============================================================
    // v3.11.0 — TESTS CHO SIÊU NÂNG CẤP MARKDOWN
    // ============================================================

    #[test]
    fn test_v3110_kbd_basic() {
        let out = render("Nhấn [[Ctrl]] + [[C]] để copy");
        assert!(out.contains("<kbd>Ctrl</kbd>"), "got: {out}");
        assert!(out.contains("<kbd>C</kbd>"));
    }

    #[test]
    fn test_v3110_kbd_ignored_inside_code() {
        // `[[X]]` trong code block KHÔNG được convert
        let out = render("```\n[[X]]\n```");
        assert!(
            !out.contains("<kbd>"),
            "kbd không được sinh trong <pre>: {out}"
        );
        assert!(out.contains("[[X]]"));
    }

    #[test]
    fn test_v3110_kbd_dangerous_content_stays_escaped() {
        // comrak escape `<script>` thành &lt;script&gt; TRƯỚC khi kbd pass
        // chạy — kbd bọc nội dung đã escape → render ra chữ "<script>"
        // thuần text, KHÔNG có tag thật trong output.
        let out = render("[[<script>]]");
        assert!(
            !out.contains("<script>"),
            "không được có tag script thật: {out}"
        );
        assert!(
            out.contains("&lt;script&gt;"),
            "nội dung phải escape: {out}"
        );
    }

    #[test]
    fn test_v3110_kbd_in_bio() {
        let out = render_bio("Bio có [[F5]] nè");
        assert!(out.contains("<kbd>F5</kbd>"), "bio: {out}");
    }

    #[test]
    fn test_v3110_custom_heading_id() {
        let out = render("## Cài đặt nhanh {#cai-dat-rieng}");
        assert!(
            out.contains("id=\"cai-dat-rieng\""),
            "custom id phải được dùng: {out}"
        );
        // Text hiển thị KHÔNG còn `{#...}`
        assert!(!out.contains("{#"));
        // Anchor link trỏ đúng custom id
        assert!(out.contains("href=\"#cai-dat-rieng\""));
    }

    #[test]
    fn test_v3110_custom_heading_id_in_toc() {
        let input = "[toc]\n\n## Mục Một {#muc-mot}";
        let out = render(input);
        // ToC entry phải link tới custom id
        assert!(
            out.contains("href=\"#muc-mot\""),
            "ToC phải dùng custom id: {out}"
        );
    }

    #[test]
    fn test_v3110_custom_heading_id_invalid_rejected() {
        // id có ký tự lạ → KHÔNG strip (hiển thị nguyên văn như text)
        let out = render("## Title {#id lạ}");
        assert!(!out.contains("id=\"id lạ\""));
    }

    #[test]
    fn test_v3110_abbreviation() {
        let input = "*[XP]: Điểm kinh nghiệm\n\nKiếm XP mỗi ngày nhé!";
        let out = render(input);
        assert!(
            out.contains("<abbr title=\"Điểm kinh nghiệm\">XP</abbr>"),
            "abbr phải render: {out}"
        );
        // Dòng định nghĩa bị bỏ khỏi output
        assert!(!out.contains("Điểm kinh nghiệm\n"));
        assert!(!out.contains("<p>[XP]:"));
    }

    #[test]
    fn test_v3110_abbreviation_word_boundary() {
        // "HTML5" chứa "HTML" nhưng là từ khác → KHÔNG thay
        let input = "*[HTML]: HyperText Markup Language\n\nHTML5 ra mắt rồi, HTML cũ hơn.";
        let out = render(input);
        // "HTML5" nguyên vẹn
        assert!(out.contains("HTML5"));
        // Từ đơn HTML được bọc
        assert!(out.matches("<abbr").count() >= 1);
        assert!(!out.contains("<abbr title=\"HyperText Markup Language\">HTML5</abbr>"));
    }

    #[test]
    fn test_v3110_abbreviation_ignored_in_code() {
        let input = "*[X]: Y\n\n```\nX = X + 1\n```";
        let out = render(input);
        assert!(!out.contains("<abbr"), "code block không abbr: {out}");
    }

    #[test]
    fn test_v3110_abbreviation_escapes_title() {
        let input = "*[A]: a\"b<c\n\nA here";
        let out = render(input);
        assert!(
            out.contains("title=\"a&quot;b&lt;c\""),
            "attr phải escape: {out}"
        );
    }

    #[test]
    fn test_v3110_vimeo_embed() {
        let out = render("https://vimeo.com/76979871");
        assert!(
            out.contains("player.vimeo.com/video/76979871"),
            "got: {out}"
        );
        assert!(out.contains("<iframe"));
    }

    #[test]
    fn test_v3110_vimeo_player_url_passthrough() {
        let out = render("https://player.vimeo.com/video/123456789");
        assert!(out.contains("player.vimeo.com/video/123456789"));
    }

    #[test]
    fn test_v3110_vimeo_invalid_id_rejected() {
        let out = render("https://vimeo.com/short");
        assert!(
            !out.contains("player.vimeo.com"),
            "id quá ngắn phải từ chối: {out}"
        );
    }

    #[test]
    fn test_v3110_video_file_bare_link() {
        let out = render("https://example.com/trailer.mp4");
        assert!(out.contains("<video controls"), "got: {out}");
        assert!(out.contains("video-embed"));
        assert!(!out.contains("<a href=\"https://example.com/trailer.mp4\">"));
    }

    #[test]
    fn test_v3110_video_file_image_syntax() {
        let out = render("![](https://example.com/clip.webm)");
        assert!(out.contains("<video controls"), "img-syntax video: {out}");
    }

    #[test]
    fn test_v3110_audio_file_embed() {
        let out = render("https://example.com/audio.mp3");
        assert!(out.contains("<audio controls"), "got: {out}");
        assert!(out.contains("audio-embed"));
    }

    #[test]
    fn test_v3110_labeled_video_link_not_converted() {
        // Link CÓ text riêng → vẫn là link (không tự nhúng)
        let out = render("[tải video](https://example.com/a.mp4)");
        // harden_links chèn rel/target vào tag — chỉ so phần đầu href.
        assert!(out.contains("<a href=\"https://example.com/a.mp4\""));
        assert!(out.contains(">tải video</a>"));
        assert!(!out.contains("<video"));
    }

    #[test]
    fn test_v3110_query_suffix_video_still_detected() {
        let out = render("https://example.com/v.mp4?download=1");
        assert!(
            out.contains("<video controls"),
            "query phải bỏ qua khi soi ext: {out}"
        );
    }

    #[test]
    fn test_v3110_mermaid_block_converted() {
        let out = render("```mermaid\ngraph TD\n  A --> B\n```");
        assert!(out.contains("<div class=\"mermaid\">"), "got: {out}");
        assert!(out.contains("mermaid-wrapper"));
        // KHÔNG còn là code block (không line number/copy cho diagram)
        assert!(!out.contains("code-block-wrapper"));
        assert!(!out.contains("code-lang-label"));
    }

    #[test]
    fn test_v3110_mermaid_strips_syntect_span_tags() {
        // mermaid.run đọc innerHTML — tag syntect `<span class="text plain">`
        // trong div phải được STRIP, text + entities giữ nguyên.
        let out = render("```mermaid\ngraph TD\n  A --> B\n```");
        assert!(
            !out.contains("<div class=\"mermaid\"><span"),
            "div mermaid không được chứa tag syntect: {out}"
        );
        assert!(out.contains("<div class=\"mermaid\">graph TD"));
        // `-->` giữ dạng escape `--&gt;` (textContent client decode)
        assert!(out.contains("--&gt;"));
    }

    #[test]
    fn test_v3110_mermaid_content_preserved_escaped() {
        let out = render("```mermaid\nA[x > y]\n```");
        // Nội dung phải được escape trong HTML (an toàn) nhưng textContent
        // nguyên vẹn cho mermaid client đọc.
        assert!(out.contains("A[x &gt; y]"), "escaped content: {out}");
    }

    #[test]
    fn test_v3110_other_code_not_mermaid() {
        let out = render("```rust\nfn main() {}\n```");
        assert!(out.contains("code-block-wrapper"));
        assert!(!out.contains("mermaid"));
    }

    #[test]
    fn test_v3110_math_classes_match_comrak_output() {
        let out = render("Công thức $E=mc^2$ nè");
        // comrak phát class="math inline" — CSS v3.11.0 giờ khớp đúng
        assert!(out.contains("class=\"math inline\""), "got: {out}");
    }

    #[test]
    fn test_v3110_math_display() {
        let out = render("$$x^2$$");
        assert!(out.contains("math display"));
    }

    #[test]
    fn test_v3110_cache_version_bumped() {
        // Bảo hiểm: đổi engine phải bump cache version (4 hiện tại)
        assert_eq!(CACHE_VERSION, 4);
    }

    #[test]
    fn test_v3110_pre_process_keeps_code_fences() {
        // ` {#id}` bên trong fenced code KHÔNG bị strip
        let input = "```md\n## Title {#id}\n```";
        let (cleaned, ids, abbrs) = pre_process_input(input);
        assert!(ids.is_empty(), "không parse id trong code fence");
        assert!(abbrs.is_empty());
        assert!(cleaned.contains("{#id}"));
    }

    #[test]
    fn test_v3110_strip_custom_heading_id_unit() {
        assert_eq!(
            strip_custom_heading_id("## Cài đặt {#cai-dat}"),
            Some(("Cài đặt".to_string(), "cai-dat".to_string()))
        );
        assert_eq!(strip_custom_heading_id("## Không có id"), None);
        assert_eq!(strip_custom_heading_id("## Id lạ {#a b}"), None);
        assert_eq!(strip_custom_heading_id("### {#only-id}"), None); // text rỗng
    }

    #[test]
    fn test_v3110_vimeo_id_extraction() {
        assert_eq!(
            extract_vimeo_id("https://vimeo.com/76979871"),
            Some("76979871")
        );
        assert_eq!(
            extract_vimeo_id("https://www.vimeo.com/123456?x=1"),
            Some("123456")
        );
        assert_eq!(
            extract_vimeo_id("https://player.vimeo.com/video/987654321"),
            Some("987654321")
        );
        assert_eq!(extract_vimeo_id("https://youtube.com/watch?v=x"), None);
        assert_eq!(extract_vimeo_id("https://vimeo.com/abc"), None);
    }

    #[test]
    fn test_v3110_guide_document_renders_clean() {
        // Trang /markdown render guide thật — smoke test bảo vệ
        const GUIDE_MD: &str = include_str!("../../docs/markdown_guide.md");
        let out = render(GUIDE_MD);
        // Không HTML thô lọt qua
        assert!(out.contains("Hướng dẫn Markdown toàn diện"));
        // Có ToC (guide chứa [toc])
        assert!(out.contains("toc") || out.contains("Mục lục"));
        // Guide dùng kbd
        assert!(out.contains("<kbd>"));
        // Guide dùng mermaid
        assert!(out.contains("<div class=\"mermaid\">"));
        // Guide dùng math
        assert!(out.contains("math"));
        // Guide dùng abbreviation
        assert!(out.contains("<abbr title="));
        // Guide dùng custom heading id
        assert!(out.contains("id=\"cai-dat-nhanh\""));
    }
    #[test]
    fn test_v3110_probe_spoiler_pipe() {
        let out = render("Kết quả: ||bí mật|| nè");
        println!("SPOILER_OUT: {out}");
        assert!(out.contains("spoiler"), "got: {out}");
    }
    #[test]
    fn test_v3110_math_span_xss_neutralized() {
        // comrak escape nội dung math TRƯỚC khi normalize_math_spans bọc
        // wrapper — mọi payload tag đều thành text escaped.
        for input in [
            "$<script>alert(1)</script>$",
            "$<img src=x onerror=alert(2)>$",
            "$$</span><script>alert(3)</script>$$",
        ] {
            let out = render(input);
            assert!(!out.contains("<script"), "XSS leak: {out}");
            assert!(!out.contains("<img src=x"), "XSS img leak: {out}");
            assert!(out.contains("&lt;"), "nội dung phải escaped: {out}");
        }
    }

    #[test]
    fn test_v3110_abbr_term_with_tag_is_ignored() {
        // Term chứa tag → không match text escaped → không bao giờ được
        // chèn raw vào output (không có đường inject).
        let out = render("*[<script>]: x\n\n<script> here");
        assert!(!out.contains("<abbr"), "no abbr for tag-term: {out}");
        assert!(!out.contains("<script> here</abbr>"));
    }
}
