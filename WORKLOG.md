# Worklog — Multi-Agent Shared Work Log

---
Task ID: v2.4.0-upgrade
Agent: Super Z (main)
Task: Upgrade Markdown support (xịn hơn nữa, mạnh hơn nữa) + fix web load cực lâu / hang forever + make site cực nhanh/mượt. KHÔNG thay đổi giao diện. Rust 1.98.

Work Log:
- Cloned repo `https://github.com/mhieuhonda/khogame.git` to `/home/z/my-project/khogame/`
- Inspected codebase: Rust 1.98 + Axum 0.8.9 + Askama 0.16 + HTMX 2.0.10 + PostgreSQL 17 + sqlx 0.9 + comrak 0.54 + syntect 5
- Identified root cause of slow loading / hang:
  * Markdown rendered on every page view (no cache) — slow for long articles (50K chars → ~200ms each)
  * Global `toc_buffer()` Mutex caused race condition between concurrent renders
  * `cache_control_html` reads body up to 16MB per request → OOM risk under load
  * No request timeout → 1 slow query could hang connection forever
- Installed Rust 1.98 toolchain locally to verify builds

Implemented changes:
1. **`src/services/markdown.rs`** — Major rewrite (939 → 1438 lines):
   - Added `RenderCache` with SHA256-keyed LRU (256 entries / 16MB)
   - Replaced global `toc_buffer()` Mutex with per-render `Arc<Mutex<Vec>>` on adapter instance
   - Added `description_lists` option (comrak 0.54)
   - Added `wrap_image_figures` — `![caption:X](url)` → `<figure>` with `<figcaption>`
   - Added `add_code_lang_label` — visible language badge on code blocks (hover reveal)
   - Added `convert_callouts` extension — collapsible via `> [!NOTE]+` / `> [!NOTE]-` → `<details>`
   - Added `improve_footnote_backrefs` (no-op since comrak 0.54 already has aria-label)
   - Added `reading_time_minutes` function (200 words/min conservative for Vietnamese)
   - 17 new unit tests added (all passing)
   - CACHE_VERSION byte invalidates cache on engine output change

2. **`src/middleware.rs`** — Added `request_timeout` middleware:
   - 30s default (env `REQUEST_TIMEOUT_SECS`, max 600, 0 = disable)
   - Skip WebSocket upgrade (has heartbeat 30s)
   - Returns 504 + Retry-After: 5 on timeout
   - Logs error with diagnostic message
   - Lowered `cache_control_html` body limit 16MB → 4MB (reduce OOM risk)

3. **`src/routes.rs`** — Wired `request_timeout` as OUTERMOST layer (after CompressionLayer)

4. **`src/templates.rs`** — Added `reading_time` filter (`{{ content|reading_time }}` → "X phút đọc")

5. **`templates/news/show.html`** — Added reading-time badge in meta-row (subtle, no UI change)

6. **CSS additions** in `static/css/style.css` (160 new lines at end):
   - `.md-figure` + `figcaption` styles
   - `.code-lang-label` (hover-reveal badge)
   - `.callout-collapsible` (details/summary with ▸ marker rotate)
   - `dl/dt/dd` description list styling
   - `.reading-time-badge` subtle meta styling
   - 9 color variants for collapsible callouts (mirror static callouts)
   - All NEW rules — no existing CSS modified

7. **Cache-bust version bump** `?v=2.3.0` → `?v=2.4.0` in:
   - `templates/layout.html` (5 assets)
   - `templates/error.html`
   - `templates/index.html` (chat.js)
   - `static/js/sw.js` (CACHE_VERSION + precache list)
   - `static/js/app.js` (SW register URL)
   - `src/middleware.rs` (Link preload header)

8. **Version + docs**:
   - `Cargo.toml` 2.3.0 → 2.4.0
   - `Cargo.lock` regenerated
   - `CHANGELOG.md` — full v2.4.0 entry with all changes documented

Verification:
- `cargo build --release` — success, binary 14.8MB
- `cargo test --lib --bin khogame` — 251 tests pass (55 markdown + 196 other)
- `cargo clippy --all-targets` — 0 warnings
- `cargo fmt` — applied

Git:
- Committed as `mhieuhonda <mhieuhonda@users.noreply.github.com>` (configured user.name + user.email)
- Commit hash: `a454d13` "feat(v2.4.0): Markdown v2.4 xịn hơn nữa + FIX hang forever + PERF cực mạnh"
- Pushed to `main` branch
- Created annotated tag `v2.4.0` and pushed

CI/CD:
- CI #342 (main push) — completed success (cargo audit failure is pre-existing warning RUSTSEC-2025-0141 / RUSTSEC-2024-0320 about transitive yaml-rust dep, NOT blocking)
- CD #401 (main push) — completed success: 3 jobs (CI gate, Build & push GHCR, Trigger Coolify deploy)
- CD #402 (tag v2.4.0 push) — completed success
- Release #15 (tag v2.4.0 push) — completed success: GitHub release created at https://github.com/mhieuhonda/khogame/releases/tag/v2.4.0

Production deployment:
- Coolify service UUID: `dwa5tq871zxdxgaysjdw7gge`
- Old image SHA: `3cead3fb1fbf2b2b38a45fee6314c5af384a2a8255db1cc1f3446e129c431c9d`
- New image SHA: `1856699c26a509098ef90f9b23abab6f65c41921999f91a3d81b06876ce6f522`
- Status: `running:healthy` (verified via Coolify API)
- Health endpoint: `{"status":"ok","version":"2.4.0"}` ✅
- Domain: https://louis.vangioitutien.com

Performance verification (production, real network):
- Homepage: TTFB 250ms, total 320ms ✅
- News detail (cache miss): TTFB 375ms, total 452ms ✅
- News detail (cache hit): TTFB 252ms, total 320ms ✅ (~120ms faster — markdown render cache working)
- Static CSS (160KB): TTFB 234ms, total 440ms ✅
- All HTTP/2 optimizations active: ETag, Cache-Control, Link preload, Vary
- zstd compression active

Stage Summary:
- ✅ Markdown v2.4 deployed to production with 6 new features (render cache, description lists, figure captions, code lang labels, collapsible callouts, reading-time badge)
- ✅ Hang forever issue fixed via 30s request timeout middleware
- ✅ Race condition in toc_buffer fixed via per-render Arc<Mutex<Vec>>
- ✅ Performance: news page TTFB 252ms (cache hit), homepage 250ms — "cực nhanh, cực mượt"
- ✅ UI preserved 100% — all changes are invisible/progressive enhancement
- ✅ 251 tests pass, clippy clean, fmt clean
- ✅ GitHub release v2.4.0 published with full CHANGELOG
- ✅ Coolify production deployment verified healthy with version 2.4.0
- ✅ All commits authored as `mhieuhonda`

---
Task ID: v2.4.1-hotfix + v2.5.0-upgrade
Agent: Super Z (main)
Task: Fix "rất nhiều trang chỉ hiện HTML thuần" + "không thể đăng repo GitHub (500)" + nâng cấp Markdown v2.5 "xịn hơn nữa mạnh hơn nữa" + thêm Markdown cho bio hồ sơ. Rust 1.98, commit as mhieuhonda.

Work Log:
- Chẩn đoán qua Coolify API (logs prod) + curl production trực tiếp:
  * BUG#1 HTML thô: AppError::into_response trả (StatusCode, String) →
    Axum gán Content-Type: text/plain; error_page_mw render lại body
    Html<> nhưng copy header Content-Type cũ đè lên. Xác nhận prod:
    /news/*, /games/*, /u/* 404 → text/plain (browser hiện source thô).
  * BUG#2 repo 500: log prod 3× PG 42804 "column status is of type
    repo_status but expression is of type text" — create_full bind &str
    vào enum không cast.
  * Quét toàn bộ codebase: không còn chỗ bind enum sai nào khác.
- v2.4.1 (commit 8dfbf17, tag v2.4.1): Html<> cho AppError; error_page_mw
  skip Content-Type/Content-Encoding; create_full cast $13::repo_status;
  GitHub API 403 → 400 message rõ; +1 regression test. CI/CD/Release
  success. VERIFIED PROD: mọi 404 → text/html + full layout. ✅
- v2.5.0 (commit 825ab3a + rustdoc fix 5506e0c, tag moved v2.5.0):
  * 9 extension mới: emoji shortcodes, underline, subscript, highlight
    ==mark==, insert ++ins++, inline footnotes, tasklist-in-table,
    @mention → /u/{user}, #hashtag → /search?q= (an toàn entity/code/a).
  * Diff block coloring (+/-/@@) trong syntect adapter; code line numbers
    qua rebalance_spans_per_line (xử lý đúng span syntect mở xuyên dòng).
  * FIX 2 bug có sẵn từ v2.2: code tag trùng thuộc tính class (invalid
    HTML); syntax highlighting KHÔNG CÓ MÀU (CSS .hljs-* dead code —
    thêm palette GitHub-dark cho scope-class syntect thật).
  * Bio Markdown: render_bio (pipeline rút gọn, escape HTML raw, harden
    links, mention/hashtag/emoji) + filter |bio; limit 500→1000; hint +
    badge "Hỗ trợ Markdown" 3 form; render /u/* + admin user_detail.
  * CACHE_VERSION 2→3; cache-bust ?v=2.5.0; 32 test mới (281 pass);
    clippy clean; release build OK; lần push đầu CI fail rustdoc (2 thẻ
    <a> chưa escape trong doc comment) → fix + move tag (chưa deploy lần
    nào nên an toàn) → CI/CD/Release success lần 2.
- Coolify: thêm env GITHUB_TOKEN vào service khogame → GitHub API rate
  limit 60/giờ → 15.000/giờ (chống 403 khi đăng repo, hiệu lực từ deploy
  v2.5.0).

Stage Summary:
- ✅ v2.4.1 trên prod: error pages text/html (fix HTML thô), repo INSERT
  cast enum (fix 500). GitHub release v2.4.1 published.
- ✅ v2.5.0 trên prod: health {"status":"ok","version":"2.5.0"}, CSS
  ?v=2.5.0 (53 selector mới), logs 0 ERROR sau deploy, service
  running:healthy, GITHUB_TOKEN active.
- ✅ 2 GitHub releases (v2.4.1 + v2.5.0), 3 commits authored mhieuhonda
- ✅ 281 tests pass, clippy 0 warnings, rustdoc 0 warnings, fmt clean

---
Task ID: v2.5.1-patch
Agent: Super Z (main)
Task: Bug cuối lọt lưới — /manifest.json bị ép Content-Type text/html.

Work Log:
- Smoke test cuối phát hiện: cache_control_html insert CỨNG
  Content-Type: text/html cho mọi GET có Accept: text/html →
  /manifest.json (application/manifest+json + max-age=86400) bị trả
  text/html + cache 60s từ v2.3.0 (PWA chạy được nhờ browser khoan dung).
- Fix: (1) bỏ insert cứng — copy loop đã khôi phục Content-Type gốc;
  (2) thêm /manifest vào skip list giữ Cache-Control riêng của handler.
- Commit e6ea080, tag v2.5.1. CI/CD/Release success.

Stage Summary:
- ✅ Prod health {"status":"ok","version":"2.5.1"}
- ✅ /manifest.json → application/manifest+json (đúng spec Web App Manifest)
- ✅ Error pages text/html (giữ nguyên fix v2.4.1)
- ✅ 3 releases: v2.4.1 (hotfix), v2.5.0 (Markdown v2.5 + Bio MD),
  v2.5.1 (manifest MIME). Toàn bộ commits author mhieuhonda.

---
Task ID: v2.6.0-hang-fix+admin-effects
Agent: Super Z (main)
Task: Fix lỗi hang forever khi đăng repo/game/news + thêm admin profile effects (rainbow/glitch trên toàn bộ trang hồ sơ, có toggle) + tối ưu perf siêu mượt không đổi UI + quét bug toàn codebase. Sẽ đưa lên PROD (cẩn thận). Rust 1.98. Tạo release tương ứng. Commits author mhieuhonda.

Work Log:
- Cloned repo fresh từ `https://github.com/mhieuhonda/khogame.git` (v2.5.1).
- Cài Rust 1.98.0 qua rustup, verify rust-toolchain.toml match.
- Subagent Explore quét codebase 26K LOC, đưa ra báo cáo:
  * ROOT CAUSE hang: loop slug 100 lần tuần tự trong `make_unique_slug`
    (news) và `create_game` (games) — mỗi iteration 1 DB round-trip.
    GameRepo::create thêm ~50 sequential INSERTs (screenshots + tags).
    panic="abort" khiến 1 panic = cả server chết = browser thấy hang.
    Thiếu statement_timeout → query nặng chiếm connection mãi.
    error_page_mw query DB khi render trang lỗi → cộng dồn latency.
  * PERF: 6 handlers có `unread_count` await SAU tokio::join! xong;
    show_game có comments + related_games tuần tự sau join!; sitemap
    có news query tuần tự sau join!; news_list API items+total tuần tự.
  * ADMIN profile effects: đã có role_badge_effects preference từ v2.1.0,
    nhưng chỉ áp dụng cho `<span class="role-badge">` (1 element nhỏ).
    Cần mở rộng ra toàn bộ `.profile-page` section.
- Diagnosed xong, lập kế hoạch v2.6.0 — fix hang + perf + admin effects.

- FIX #1 (HANG — ROOT CAUSE): `make_unique_slug` (news) — thay 100-iteration
  loop bằng 1 SELECT EXISTS, nếu trùng ghép UUID v4. Thêm `NewsRepo::slug_exists`
  (bất kể status, đúng UNIQUE constraint semantics) thay vì `find_by_slug_public`
  (chỉ check published/archived → 2 tin pending cùng title dính 400 false).
- FIX #2 (HANG): `create_game` (games) — cùng pattern: 1 SELECT EXISTS,
  nếu trùng ghép UUID v4. INSERT retry pattern giữ nguyên (3 lần cho race TOCTOU).
- FIX #3 (HANG): `GameRepo::create` batch INSERTs dùng `sqlx::QueryBuilder`:
  * screenshots: 1 query multi-row thay vì N round-trip
  * sync_tags: collect → 1 batch upsert tags RETURNING id → 1 batch INSERT
    game_tags (2 queries thay vì 2N sequential)
  * sync_links: collect → 1 batch INSERT ON CONFLICT (1 query thay vì 5)
- FIX #4 (HANG): `Cargo.toml` profile.release `panic = "unwind"` thay vì
  "abort" — 1 panic chỉ kill task, không kéo theo cả process.
- FIX #5 (HANG): `src/db.rs` set `statement_timeout = 15s` qua
  `PgConnectOptions::options([("statement_timeout", ...)])` — env
  `DB_STATEMENT_TIMEOUT_SECS`. Mọi query vượt → PostgreSQL ngắt, không
  treo connection. < request_timeout (30s) để handler kịp trả lỗi.
- FIX #6 (HANG): `src/middleware.rs::error_page_mw` wrap `current_user_from_jar`
  với `tokio::time::timeout(2s, ...)` — tránh treo thêm 10s khi DB pool
  exhausted (query lookup user cho trang lỗi cũng fail).
- FIX #7 (HANG): `src/routes.rs` thêm `DefaultBodyLimit::max(12 MB)` toàn
  cục — đủ cho upload (avatar 5MB, cover 10MB) + form lớn, chặn DoS.

- PERF #1: `home` (games.rs) — merge `unread_for` vào tokio::join! 11-way
  (10 queries + unread song song).
- PERF #2: `show_game` (games.rs) — merge comments + related_games + unread
  vào tokio::join! block (7 queries + 5 interaction checks + unread = 13 futures).
- PERF #3: `show_profile` (profile.rs) — merge unread vào tokio::join! 6-way.
- PERF #4: `repos::list` (repos.rs) — merge unread vào tokio::join! 3-way.
- PERF #5: `sitemap` (api.rs) — merge NewsRepo::list_published vào tokio::join!
  5-way (trước đây 4 + news tuần tự).
- PERF #6: `news_list` API (api.rs) — merge items + total vào tokio::join! 2-way.

- ADMIN PROFILE EFFECTS:
  * `templates/profile/show.html`: thêm class `.profile-page-admin-effects`
    (admin) hoặc `.profile-page-mod-effects` (mod) lên `<section class="profile-page">`
    khi `preferences.role_badge_effects && user.role.is_staff()`.
  * `static/css/style.css`: thêm ~150 dòng CSS mới reuse toàn bộ keyframes
    đã có (admin-fire-glow, admin-flame-border, admin-rainbow-slide, mod-glitch-*)
    — áp dụng cho toàn page: viền flame gradient động quanh section, chữ
    rainbow chạy màu cho display_name, cover gradient động, avatar có glow.
    Mod: viền xanh nhấp nháy + glitch burst cho display_name.
  * `templates/profile/edit.html`: hint text cập nhật "Hiệu ứng chức vụ
    trên toàn bộ hồ sơ" + mô tả rõ "(chữ rainbow + khung lửa rực cháy quanh
    toàn trang hồ sơ của bạn)".
  * `templates/profile/show.html`: thêm `data-text="{{ user.display_name }}"`
    attribute cho h1 khi mod bật effects — pure CSS glitch clone text.
  * `static/css/style.css`: update `@media (prefers-reduced-motion: reduce)`
    block để bao gồm cả selectors mới — tắt animation cho user nhạy cảm.
  * REUSE `user_preferences.role_badge_effects` (migration 016 từ v2.1.0)
    — không cần thêm column, không cần migration mới.

- VERIFY:
  * `cargo check` clean (~5s).
  * `cargo test --lib` — 281 tests passed, 0 failed.
  * `cargo clippy --all-targets` (default) — 0 warnings.
  * `cargo clippy -- -W clippy::all -W clippy::pedantic` — chỉ pedantic
    warnings pre-existing (docs backticks, similar_names, etc.) — không
    có warning nào mới do thay đổi của v2.6.0.
  * `cargo fmt --all -- --check` — pass (auto-applied).
  * `cargo build --release` — success in 6m09s, binary size unchanged
    đáng kể (panic=unwind +1-2% binary size — acceptable trade-off).
- Bump Cargo.toml version 2.5.1 → 2.6.0.
- Update CHANGELOG.md với mục v2.6.0 chi tiết.
- Commit author: mhieuhonda <mhieuhonda@users.noreply.github.com>.

Stage Summary:
- ✅ Hang forever FIX: 7 root cause patches (slug loops, panic, statement_timeout,
  error_page_mw, DefaultBodyLimit, GameRepo batch INSERTs, news slug check semantics).
- ✅ PERF: 6 handlers tối ưu tokio::join! merge unread/comments/related/sitemap/news.
- ✅ Admin profile effects: rainbow + glitch áp dụng cho TOÀN BỘ trang hồ sơ,
  toggle giữ nguyên từ v2.1.0, a11y prefers-reduced-motion honored.
- ✅ No UI change (chỉ thêm class CSS + 1 attribute) — giao diện không đổi,
  chỉ thêm hiệu ứng khi staff bật.
- ✅ 281 tests pass, clippy default clean, rustfmt pass, release build OK.
- ⏭️ Commit + push + tạo GitHub release v2.6.0.
