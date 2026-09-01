# Worklog — Multi-Agent Shared Work Log

---
Task ID: v3.10.0-profile-polish
Agent: Super Z (main)
Task: Polish hồ sơ theo yêu cầu chủ sở hữu — bỏ bóng đổ chữ (quá tối),
sửa rainbow admin bị xỉn, vùng thông tin chi tiết AI Agent đen → trắng,
admin upload avatar AI Agent, đổi tên huy hiệu lặp/nhạt + huy hiệu ĐỘC
QUYỀN AI Agent do admin cấp, siêu quét bảo mật, release v3.10.0.

Work Log:
- CSS hồ sơ: gỡ text-shadow `.profile-meta h1/.profile-username`
  (nguyên nhân kép — chữ tối VÀ làm gradient rainbow xỉn vì bóng vẽ
  sau nền background-clip:text), nâng màu trắng tinh #ffffff.
- Rainbow: 3 điểm gradient (khung role + chữ badge + .rainbow-text)
  nâng sắc 500 đậm → bảng sáng #fb7185/#fbbf24/#a3e635/#34d399/#38bdf8/
  #c084fc; @media print đổi màu fallback tương ứng.
- `.ai-params-card`: nền trắng cố định + chữ slate (AA cả 2 theme),
  viền/chip trộn --ai-accent, amber-700 cho nhóm kích hoạt, shadow nhẹ.
- Admin upload avatar AI Agent: `.upload-zone` trong ai_edit.html tái
  dùng /uploads/avatar + initUploads generic (magic bytes, random tên,
  quota) — tự điền URL #e-avatar + preview; lưu khi submit form.
- Huy hiệu (migration 043): đổi title 30 badge thuộc 16 "họ từ" lặp
  (Huyền Thoại ×4, Đế Tôn ×3, Thánh Nhân ×3, Vô Cực ×3...) + tên nhạt
  (Bộ Sưu Tập 10 Game → Kho Báu Cá Nhân...); chỉ đổi title — id/icon/
  XP/điều kiện giữ nguyên. INSERT `ai_agent_core` "Linh Hồn Nhân Tạo"
  🤖 category ai_agent, xp 0. Script check duy nhất 163 title → PASS.
- Badge admin-cấp: POST /admin/ai-agents/{id}/badge-ai (grant/revoke)
  guard 3 lớp (staff + is_ai_agent_user + whitelist), audit log, PRG;
  `AdminAiAgentEditTemplate.has_ai_badge` mới; repo thêm has_achievement
  + revoke_achievement. Engine check_and_award không match id → không
  thể tự trao.
- Siêu quét bảo mật lần N+1: require_admin 2 lớp cho route mới, CSRF
  origin_check toàn cục, SQL parameterized, upload magic bytes, XSS
  (autoescape + json_ld_safe), avatar URL whitelist scheme, CSP/HSTS/
  COOP, rate-limit, secrets — 0 lỗ hổng mới.
- GLM 5.3 báo cáo 6 mục sanitize vào "Hoạt động gần đây" (migration 044).
- Timeline /about thêm mốc v3.10.0; CHANGELOG 3.10.0 đầy đủ; bump
  Cargo.toml/lock 3.10.0.
- PROD INCIDENT (bắt được nhờ chẩn đoán): deploy v3.10.0 đầu tiên →
  stack degraded:unhealthy, /health 503. Tái hiện chuỗi migration 001→044
  trên PostgreSQL 17.2 portable → bắt đúng gốc rễ: 044 action >200 ký tự
  vượt VARCHAR(200) → INSERT fail lúc startup → app exit. Fix: rút gọn
  task/action ≤200 (chi tiết vào message TEXT) + guard RAISE EXCEPTION
  trong migration; chạy lại chuỗi trên DB mới → PASS.
- Verify: cargo check --locked + clippy -D warnings + fmt + 351/351
  test PASS (Rust 1.98.0) + chuỗi migration 001→044 PASS trên PG 17.

Stage Summary:
- v3.10.0 sẵn sàng tag: migrations 043 + 044, CSS polish, upload zone,
  huy hiệu độc quyền AI Agent, báo cáo hoạt động GLM 5.3 công khai.
- Chiến lược deploy: push main trước → CD main xong → mới tag (tránh
  race CD 2 lần chạy như bài học v2.9.1).
- BÀI HỌC MỚI: BẮT BUỘC test migration trên Postgres thật trước khi
  push (sqlx migrate fail = web sập hoàn toàn). Bài học varchar(200).

---
Task ID: v3.5.1-superfix
Agent: Super Z (main) + 6 sub-agent audit độc lập (5-a/5-b/5-c/5-d/5-e/5-f)
Task: Siêu fix lỗi GitHub Actions (ưu tiên tối cao) + 15 vòng quét-fix bảo
mật toàn codebase, build Rust 1.98, release v3.5.1.

Work Log:
- Phân tích 3 workflow run fail (2026-08-30) trực tiếp từ logs GitHub API:
  Release v3.4.2 (bash -e giết step trước fallback), CD main + CD v3.5.0
  (3 deploy đua nhau + tag v3.4.2 lệch version Cargo.toml=3.4.1).
- FIX GitHub Actions (xem CHANGELOG 3.5.1 mục CI/CD): release.yml `if !`
  pattern + version gate; deploy.yml job-level concurrency `coolify-deploy`
  + stale-tag guard + version gate + least-privilege permissions + buildkit
  pin v0.32.2; ci.yml checkout theo ref sau autofmt + retry push.
- 15 vòng quét bảo mật: 6 vòng agent độc lập (auth/CSRF/XSS, SQLi/IDOR/
  upload/economy, Docker/supply-chain/secrets, verify round, background
  jobs/email/AI API/referral/quest/shop, templates/JS XSS sâu) + cargo
  audit + các vòng build/test/validate.
- FIX 2 HIGH mới phát hiện: XP farm draft→publish (~3.000 XP/phút) —
  publish() FOR UPDATE + gate published_at; Mystery Box in XP vô hạn —
  migration 032 giá 100 XP + cap 5/ngày + advisory lock; Reflected XSS
  report-form — resolve slug qua DB.
- FIX 8 MEDIUM: rate-limit bypass xoay cookie (validate session +
  HMAC-signed ls_anon + shared bucket /auth/ai/*), upload quota reserve
  atomic, RPS advisory locks 3 path, daily caps post_game/post_news/
  review/repo, supply-chain CI, Postgres dev bind 127.0.0.1.
- FIX 9 LOW: cookie Secure/HttpOnly, WS Origin check, health metrics
  gating staff, bỏ last_seen_at API, email validation, janitor retention
  ai_progress_reports, quest toggle farm (migration 033 like_history).
- Fix Service Worker chết im lặng từ trước (Service-Worker-Allowed: /).
- Prod compose hardening stage 2 đúng TODO của file (read_only, cap_drop
  ALL, tmpfs, no-new-privileges, pids_limit).
- Validate: cargo check/clippy -D warnings/fmt/rustdoc + cargo audit sạch;
  353/353 test PASS trên Rust 1.98.0. YAML 3 workflow hợp lệ.

Stage Summary:
- Version 3.5.1, migration 032 + 033, Cargo.lock + hmac 0.13.
- Sẵn sàng tag v3.5.1 → CD deploy serialize + Release tự tạo từ CHANGELOG.

---
Task ID: v2.9.1-deploy-verify
Agent: Super Z (main)
Task: Deploy verify + incident recovery sau release v2.9.1.

Work Log:
- Push main (9b51d20) → CI fail ở 2 job: Rustdoc (broken intra-doc link
  [`GithubApiError`] — type không import vào scope) + cargo audit (3
  advisory transitive deps của syntect: bincode/yaml-rust unmaintained +
  chacha20 yanked — KHÔNG do thay đổi dependency, RustSec DB mới cập
  nhật; job audit có continue-on-error nên không block).
- Fix rustdoc link → commit 99117b2 "fix(ci): rustdoc broken intra-doc
  link GithubApiError — CD gate fail". Local verify: RUSTDOCFLAGS="-D
  warnings" cargo doc --no-deps --document-private-items → clean.
- Re-push → CI SUCCESS. CD build + push image ghcr.io@sha256:2b4acf...
  + PATCH compose OK + trigger deploy OK. NHƯNG: 2 CD chạy đồng thời
  (main + tag v2.9.1) race cùng 1 service Coolify → stack kẹt
  "starting:unknown", web 503 ~25 phút, bước "Chờ stack healthy" timeout.
- Recovery: trigger 1 deploy sạch qua Coolify API (POST /deploy?uuid=...
  &force=true) → 2 phút sau stack "running:healthy", web 200.
- Verify prod: HTML có ?v=2.9.1 (cache-bust mới), /health 200, / 200,
  /repos 200, /login 200; image digest đang chạy == digest CD build
  (2b4acfc4...) — deploy đúng bản v2.9.1.
- Release v2.9.1 trên GitHub: workflow Release tự tạo từ CHANGELOG,
  đã rename title khớp convention ("v2.9.1 — Fix hồ sơ desktop, menu
  mobile, số sao GitHub + 8 bug audit").

Stage Summary:
- Prod louis.vangioitutien.com CHẠY v2.9.1 healthy — verified end-to-end.
- BÀI HỌC: tránh push main + tag cùng lúc (CD race). Lần sau: push main,
  đợi CD main xong rồi mới tag. CD step "wait healthy" có thể thêm
  retry/re-trigger deploy tự động khi timeout.
- Cargo audit advisory (syntect transitive deps) — theo dõi, chờ syntect
  release bản thay thế yaml-rust/bincode; không action ngay (chỉ
  unmaintained warning, không có CVE thực tế).

---
Task ID: v2.9.1-ui-bugfixes
Agent: Super Z (main)
Task: Fix tên hiển thị lệch trên desktop + menu ba gạch tràn mobile + số sao GitHub không cập nhật + quét codebase lần 2 fix tuyệt đối mọi lỗi. Rust 1.98, prod-ready, tạo release.

Work Log:
- Clone repo + quét cấu trúc (src/ 18 handlers, 60 templates, 7802 dòng CSS, 1089 dòng app.js).
- Dựng trang hồ sơ tĩnh với đúng CSS site + headless Chromium (Playwright) chụp ảnh
  1280px desktop + 375×667 mobile → XÁC MINH ĐƯỢC 2 lỗi thật trước khi sửa:
  1. Desktop: tên hiển thị trôi lên đè cover, tách khỏi avatar (meta cao ~300px sau
     khi v2.9.0 thêm Level/XP + showcase, cộng align-items: flex-end).
  2. Mobile: menu mega cao 716px > viewport 667px, không cuộn được, "Đăng xuất" unreachable.
- FIX CSS (style.css):
  * .profile-info align-items flex-end → flex-start (layout hồ sơ kiểu X) + avatar
    flex-shrink: 0 + nút hành động align-self: flex-end giữ vị trí đáy như cũ.
  * .profile-meta h1 overflow-wrap: anywhere — tên dài 1 từ không còn tràn ngang.
  * .site-menu max-height calc(100dvh - header) + overflow-y auto + overscroll-behavior
    contain; mobile ≤640px menu 1 cột full-width.
- FIX SỐ SAO GITHUB (root cause: RepoRepo::refresh_all_stars dead code, không bao giờ
  được gọi): tạo services/github.rs dùng chung (fetch_repo_meta + GithubApiError có
  is_rate_limited); handler repos.rs ủy thác + map lỗi giữ nguyên regression suite;
  janitor thêm run_repo_star_refresh (3h/lần, batch 100 repo stale >1h, delay 1.5s,
  dừng khi rate limit, 404 bỏ qua); lib.rs spawn task; repo_repo.rs thêm
  list_stale_approved (thay refresh_all_stars).
- FIX TÊN LỆCH FONT (nguyên nhân thứ 2): utils::normalize_nfc — Google OAuth có thể
  trả name NFD → dấu combining (U+031B/U+0323) ngoài unicode-range font subset
  Inter vietnamese → fallback font khác cho riêng dấu → lệch nét. Áp tại OAuth +
  edit profile + AI register. Test tổ hợp dấu tiếng Việt đầy đủ.
- Quét codebase lần 2 (Explore agent toàn repo) → fix thêm 7 lỗi:
  5. list_replies game_slug rỗng → reply-to-reply POST /games//comments 404.
  6. do_checkin trả xp đã lưu cho re-click → luôn báo "thành công +N XP" oan.
  7. Race do_checkin: ON CONFLICT DO NOTHING không check rows_affected → XP x2.
  8. POST /games/{slug}/share dead code → app.js fire-and-forget fetch (data-slug có sẵn).
  9. Incident ID 5xx hiển thị cho user không được log → tracing::error! kèm ID.
  10. sw.js cache HTML cá nhân hoá trên máy dùng chung → request có kg_session → network-only.
  11. SMTP_* không được nội suy trong compose (email không thể bật ở prod) → thêm
      ${SMTP_*:-} vào 2 compose + tài liệu .env.example (REQUEST_TIMEOUT_SECS,
      DB_STATEMENT_TIMEOUT_SECS, COOKIE_SECURE, REPO_REFRESH_INTERVAL_SECS).
- Bump version 2.9.1: Cargo.toml + ?v=2.9.1 toàn template + CACHE_VERSION sw.js.
- Build + verify: cargo check clean, clippy 0 warning, 306/306 unit tests pass
  (Rust 1.98.0). Headless verify sau fix: tên cạnh avatar ✓, tên dài wrap ✓,
  menu cuộn tới đáy nút Đăng xuất fully visible ✓.

Stage Summary:
- 0 migration mới, 0 schema change — deploy an toàn, không rủi ro dữ liệu.
- Commit v2.9.1 + tag + GitHub Release kèm binary note; prod compose thêm SMTP
  passthrough (default rỗng — không đổi hành vi hiện tại).

---
Task ID: v2.9.0-gamification
Agent: Super Z (main)
Task: Super-fix toàn bộ lỗi + thêm 50 tính năng giữ chân người dùng + bỏ icon lửa khung chức vụ admin. Rust 1.98, production-ready, tạo releases.

Work Log:
- Audit toàn diện (cargo check/clippy/test + review thủ công ~34k dòng) → 9 bug thật:
  1. [CRITICAL] Trigger email queue (017) sai enum news_approval/news_rejection
     → mọi notification system/review/reply... cho user có email đều rollback âm thầm
     từ v2.2.0. Fix migration 022.
  2. [HIGH] cache_control_html check 'ls_session' nhưng cookie thật 'kg_session'
     → trang đã login bị cache public. Fix dùng SESSION_COOKIE.
  3. [MEDIUM] like comment partial luôn hiện "chưa like" (FALSE as is_liked hardcode).
  4. [MEDIUM] load-more comments trộn counter replies/gốc → nút treo vĩnh viễn.
  5. [MEDIUM] email_queue kẹt 'sending' sau crash — thêm requeue_stuck_sending.
  6. [MEDIUM] spam notification toggle like/follow — dedup unread same (actor,type,target).
  7. [MEDIUM] OFFSET overflow 17 call sites — saturating_mul + clamp.
  8. [LOW] db.rs redact credential giữ nhầm password — viết lại + test.
  9. [LOW] sw.js cache trang private — route private network-only.
- Bỏ icon lửa SVG khỏi khung chức vụ admin (giữ chữ rainbow, viền đổi rainbow).
- Migration 021 (gamification, 8 bảng + seed 25 huy hiệu) + 020 (notif dedup
  trigger) + 022 (fix trigger enum).
- Gamification engine: models + repo + services (XP/level/streak/achievements/
  leaderboard) + hooks tại 9 handler — toàn bộ best-effort fire-and-forget.
- 50 tính năng: xem danh sách đầy đủ trong CHANGELOG [2.9.0].
- Verify tích hợp trên PostgreSQL 17 portable (port 5433): 22 migrations sạch,
  boot server thật, smoke test 30+ endpoints, flow end-to-end checkin → XP
  (+5) → 3 huy hiệu → notification → review (+15 XP) → collection →
  leaderboard → level 2 "Tập Sự".

Stage Summary:
- 303 unit tests pass, clippy clean, 0 warning.
- Toàn bộ endpoints mới đã verify HTTP 200/303/400 đúng kỳ vọng.
- Sẵn sàng release v2.8.1 (bugfix) + v2.9.0 (gamification).

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

---
Task ID: v2.7.0-upgrade
Agent: Super Z (main)
Task: Thêm mạng xã hội vào hồ sơ người dùng (github, facebook, zalo, discord + 5 nền
tảng khác). Quét codebase + VPS/site prod để fix hoàn toàn lỗi 500. Fix lỗi logic,
chính tả, lỗi khác trên toàn codebase. Rust 1.98. Tạo bản phát hành v2.7.0.

Work Log:
- Clone repo, quét codebase 26K LOC (Rust 1.98 + Axum 0.8.9 + Askama 0.16 + sqlx 0.9).
- Quét VPS qua Coolify API: service `khogame` (uuid dwa5tq871zxdxgaysjdw7gge)
  status running:healthy, image pin digest GHCR, VPS 10.187.247.3 reachable.
- Quét site prod louis.vangioitutien.com: 28 endpoint + edge-case (page=abc,
  -5, 999999, slug lạ, invalid UTF-8 %FF%FE, XSS payload trong search,
  .well-known, sitemap, rss...) — 0 lỗi 500, mọi lỗi trả 4xx đúng.
- Quét codebase: unwrap/expect ngoài test = 0; repos.rs/uploads.rs/middleware.rs
  đã harden tốt từ v2.4.x; phát hiện 2 bug thật (chi tiết ở CHANGELOG v2.7.0).

Implemented changes:
1. **Migration 019_social_links.sql** — bảng `user_social_links` (user_id PK,
   links JSONB, updated_at trigger). Bảng riêng thay vì cột trên `users`
   để zero-rủi-regression với ~15 SELECT tường minh của FromRow<User>.
2. **`src/models/social.rs`** — `SocialLinks` + `SocialPlatform` + PLATFORMS
   (10 nền tảng: github, facebook, zalo, discord, youtube, tiktok, instagram,
   twitter, telegram, website). Validation: allowlist hostname từng platform
   (chỉ www. prefix, chặn gist.github.com), chặn control byte TRƯỚC trim
   (trim() ăn tab cuối), chặn scheme lạ có :// (ftp://x trước đây ghép
   prefix thành https://ftp://x parse host "ftp"), auto https://, max 300
   chars, rỗng = xóa. 13 unit test.
3. **`src/repositories/user.rs`** — `social_links()` (fail-open rỗng) +
   `save_social_links()` UPSERT.
4. **`src/handlers/profile.rs`** — show_profile query socials SONG SONG
   (wave tokio::join!, 7 queries); edit_profile_form 3 queries song song;
   ProfileForm thêm 10 field social_*; update_profile validate-form →
   BadRequest rõ ràng trước khi ghi DB.
5. **`src/handlers/api.rs`** — `/api/v1/users/{username}` thêm
   `social_links: [{platform,label,url}]`.
6. **`src/templates.rs`** — ProfileTemplate.socials; EditProfileTemplate
   socials + platforms (&'static [SocialPlatform]).
7. **Templates** — show.html: hàng icon SVG simple-icons (CC0) dưới bio,
   rel="noopener noreferrer nofollow ugc"; edit.html: grid 10 input +
   placeholder theo platform.
8. **CSS** — 77 dòng mới: .profile-social, .social-link + hover màu brand
   10 nền tảng, dark theme, .social-edit-grid.
9. **FIX cache-bust nợ v2.5.1/v2.6.0**: `?v=2.5.0` → `?v=2.7.0` toàn bộ
   (layout.html 7, error.html 1, app.js 1, sw.js 6, middleware.rs 3).
   Trước đây v2.6.0 thêm CSS/JS mới mà không bump → cache cũ không cập nhật.
10. **FIX OAuth 500 sai sự thật** (`handlers/auth.rs`): access_denied →
    BadRequest (400) + message hướng dẫn, thay vì OAuth → 500.
11. **FIX artifact tiếng Trung** trong comment: style.css ("nền选中"),
    state.rs ("cùng到这里"), middleware.rs ("人气度高"), json_ld.rs
    ("bị破") → tiếng Việt chuẩn.
12. Bump version 2.6.0 → 2.7.0 + CHANGELOG.md chi tiết.

- VERIFY (cục bộ, khớp CI pipeline):
  * `cargo fmt --all -- --check` — pass.
  * `cargo check --locked --all-targets` — pass.
  * `cargo clippy --all-targets --locked -- -D warnings` — pass (fix 1
    warning manual_contains mới).
  * `cargo test --locked --all` — 293 tests passed, 0 failed (+13 mới).
  * `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` — pass.

Stage Summary:
- ✅ Social links 10 nền tảng: DB → model → repo → handler → template →
  CSS → API, validation allowlist chặt (anti-XSS/anti-header-injection),
  fail-open không chết trang hồ sơ.
- ✅ Quét prod + codebase: không còn 500 thật nào trên public routes;
  fix 500 sai sự thật (OAuth access_denied) + nợ cache-bust 2 bản.
- ✅ 293 tests pass, clippy -D warnings clean, rustfmt clean, rustdoc clean.
- ⏭️ Commit author mhieuhonda + push main + tag v2.7.0 → GitHub Release.

---
## [2.9.2] — 2026-08-29 — Fix CI/CD trigger chết + 15 bug từ audit toàn diện

- CONTEXT: Yêu cầu quét toàn bộ codebase 2 vòng độc lập trước khi đưa lên
  prod. Vòng 1: cargo check/clippy/fmt/test(306) đã PASS từ v2.9.1 → chuyển
  trọng tâm sang các lớp compiler không bắt được (CI YAML, security runtime,
  frontend). Vòng 2: 2 agent quét sâu song song (backend Rust 30.8k dòng +
  templates/frontend 9k dòng) → 15 lỗi thật sự + 1 false positive (script
  auto-refresh ai_reports.html thực tế NẰM TRONG block content — không sửa).

- CI/CD (2 lỗi nghiêm trọng nhất):
  * ci.yml + deploy.yml: `branches: ain]` → `branches: [main]`. YAML vẫn
    parse hợp lệ (string scalar) nên không ai phát hiện — CI/CD không bao
    giờ tự chạy khi push main. Validate bằng PyYAML sau fix.
- Security (5): rate-limit bypass bucket `x:anon-unknown` cho request
  không-cookie (bot không lưu Set-Cookie trước đây được bucket mới mỗi
  request); cap 5 WS connection/user + close 1013; request_timeout chỉ skip
  cho /chat/ws thật (trước đây mọi request có header Upgrade); broadcast
  link chặn `/\evil.com`; OAuth state so sánh constant-time (constant_time_eq
  chuyển vào utils.rs, ai_agent.rs dùng lại).
- Backend (5): STATIC_SEGMENTS +25 segment thiếu của v2.9.0 (typing/
  leaderboard/collections/uploads/chat... từng gộp chung bucket /{x});
  matcher 10/phút mở rộng cho /news_comments/; POST /repos bucket riêng
  6/phút chống đốt quota GitHub API (GET vẫn 120/phút); thống nhất MỘT chuẩn
  "hôm nay" = giờ VN (SQL_TODAY_VN / SQL_TODAY_START_VN / today_vn — không
  còn phụ thuộc timezone server Postgres; CURRENT_DATE/date_trunc UTC/
  Utc::now() trước đây lệch nhau 17:00–24:00 UTC) + AssertSqlSafe cho SQL
  động; create_from_google idempotent khi race OAuth callback (fetch lại
  theo google_sub / thử username suffix, tối đa 3 lần); profile bỏ 1 query
  user_achievements trùng; require_admin trả AppError (303 → /login + trang
  lỗi đầy đủ thay vì text trơ).
- Frontend (7): nút Xóa review 405 → button form="review-delete-form" POST
  (form không lồng nhau); my_games empty-state render nhầm khi có dữ liệu;
  chat badge Admin/Mod so role lowercase; notifications mark-all-read
  hx-swap innerHTML (trước outerHTML vỡ DOM); button "đã đọc" tách khỏi <a>
  (HTML invalid) + CSS flex row; highlight tin nhắn của mình so username
  (currentUser.id luôn null); login.html đổi SVG gradient id "g" → "g-auth"
  (trùng với layout.html).
- Testability: presence tách thành struct PresenceMap (state.rs) + 4 unit
  test mới (multi-tab refcount, cap connection, remove noop, 2 users).
  Middleware: +2 regression test cho normalize_path_for_rate_limit.
- Version/cache-bust: Cargo.toml 2.9.2; ?v=2.9.2 toàn bộ layout/error/index/
  sw.js/app.js; CACHE_VERSION ls-sw-v2.9.2; README badge 2.2.0 → 2.9.2
  (lệch hụt từ v2.3.0); CHANGELOG.md mục [2.9.2] đầy đủ.
- KHÔNG đổi schema, KHÔNG migration mới — deploy an toàn. Chấp nhận Transition
  1 ngày: daily_stats/checkin ghi theo chuẩn cũ có thể lệch biên ngày khi
  đổi sang chuẩn VN (analytics only, không ảnh hưởng dữ liệu user).
- ĐÃ CÂN NHẮC NHƯNG KHÔNG LÀM (giữ release nhỏ, an toàn prod): quota upload
  per-user + janitor dọn file mồ côi (rủi ro xoá nhầm file prod — làm riêng
  v2.9.3+); cache metadata GitHub 60s (chỉ cần rate-limit 6/phút vì handler
  check duplicate DB TRƯỚC khi gọi GitHub); gộp N+1 check_and_award (bounded
  ~26 query nhỏ mỗi login); admin/users fetch 2000 (TODO v3.0 sẵn).

- VERIFY (khớp toàn bộ CI pipeline):
  * YAML 3 workflow — PyYAML parse OK, branches: ['main'].
  * `cargo fmt --all -- --check` — pass.
  * `cargo check --locked --all-targets` — pass (khogame 2.9.2).
  * `cargo clippy --all-targets --locked -- -D warnings` — pass.
  * `cargo test --locked --all` — 312 passed, 0 failed (+6 mới).
  * `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items`
    — pass.
  * `cargo build --release --locked` — kiểm tra binary prod cuối.

Stage Summary:
- ✅ 15 lỗi fix (2 CI/CD + 5 security + 6 backend + 7 frontend, tính cả
  sub-fix trong từng mục), 6 test mới, không regression.
- ✅ Full CI pipeline xanh local: fmt/check/clippy/test/doc + YAML validate.
- ⏭️ Commit author mhieuhonda + push main + tag v2.9.2 → GitHub Release.

---
Task ID: v3.1.0-achievements-rps-wordchain
Agent: Super Z (main)
Task: Fix bug auto-grant danh hiệu + thêm 100 Danh Hiệu + MAX_LEVEL 500 tỷ + game Oẳn tù tì + game Nối từ.

Work Log:
- Clone repo khogame, cài Rust 1.98.0 exact (rustup), verify build baseline.
- Scan codebase: 30.8k dòng Rust + 9k dòng templates, 818 dòng trong
  repositories/gamification.rs. Identify bug met() trong check_and_award.
- Migration 024 mới: BIGINT cho user_xp_totals.total_xp + 100 danh hiệu
  mới (level/streak/comments/games/likes/downloads/followers/reviews/
  bookmarks/repos/news/chat/collections/social/rps/word_chain) + 2 bảng
  mới (rps_plays, word_chain_plays).
- Update src/models/gamification.rs: LevelInfo i32→i64, level_from_xp
  mở rộng tier 2 công thức + cap 500 tỷ (MAX_LEVEL = 500_000_000_000),
  title_for_level cho level 13+ (Vô Song → Vô Biên).
- Update src/repositories/gamification.rs: total_xp/award_xp trả i64;
  mở rộng met() cho 25+100=125 ID; thêm 5 cột thống kê mới
  (social_links_count via LATERAL jsonb_object_keys, collections_count,
  rps_wins, word_chain_valid, total_checkins).
- Update src/repositories/{arcade,shop,quests}.rs: i64 cho total_xp return.
- Update src/models/{gamification,review,retention}.rs: i64 cho total_xp/
  author_xp fields.
- Update src/templates.rs: ShopTemplate.total_xp i64; level_for/title_for
  filter parse i64; thêm RpsTemplate + WordChainTemplate (path rps.html +
  word_chain.html).
- Tạo src/repositories/rps.rs (RpsChoice/RpsOutcome/RpsRepo::play) +
  src/repositories/word_chain.rs (VI_VOCAB ~370 từ, WordChainRepo::play).
- Tạo src/handlers/rps.rs (rps_page + play_rps HTMX endpoint) +
  src/handlers/word_chain.rs (word_chain_page + play_word_chain).
- Tạo templates/gamification/rps.html + word_chain.html với CSS+HTMX
  integration.
- Update src/routes.rs: 4 route mới (/rps, /rps/play, /word-chain,
  /word-chain/play) + đăng ký trong layout.html menu.
- Fix bug normalize_word với NFD decomposition cho tiếng Việt có dấu.
- Fix 4 test thất bại: boundary level 12/13 tại XP=12000, normalize_word
  "Cà Phê" → "caphe" (không phải "caph").
- Update Cargo.toml: version 3.0.0 → 3.1.0; CHANGELOG.md thêm entry v3.1.0.

Stage Summary:
- ✅ Bug auto-grant danh hiệu FIXED: met() mở rộng 25 → 125 ID match arms.
- ✅ 100 Danh Hiệu mới seed (migration 024) — 16 category tier.
- ✅ MAX_LEVEL 500 tỷ (500_000_000_000) qua công thức tier 2 + BIGINT.
- ✅ Game Oẳn tù tì /rps + /rps/play (HTMX, 3 nút, daily cap 30).
- ✅ Game Nối từ /word-chain + /word-chain/play (HTMX, NFD normalize,
  vocabulary embedded ~370 từ, daily cap 20).
- ✅ 337 test pass + 0 clippy warning + cargo fmt sạch.
- ✅ cargo build --release thành công — binary production-ready.
- ⏭️ Commit + push + tag v3.1.0 → GitHub Release → Coolify deploy.

---
Task ID: v3.1.1-hotfix-prod-migration
Agent: Super Z (main)
Task: HOTFIX v3.1.0 — container restart-loop trên prod do migration 024 fail.

Work Log:
- Push v3.1.0 → CD workflow triggered → CI gate + Docker build OK nhưng
  Coolify deploy fail (stack degraded:unhealthy, app restarting:unknown).
- Investigate: container image digest = 3524843d (v3.1.0 image, NOT old).
- Inspect migration 024 INSERT values — phát hiện xp_reward của
  level_max = 5_000_000_000 (5 tỷ), PostgreSQL INT max = 2,147,483,647
  (~2.1 tỷ) → INSERT fail với "integer out of range" → toàn bộ migration
  rollback → schema không update (rps_plays/word_chain_plays không tồn
  tại) → app crash ngay startup "Migration failed".
- Fix: thay xp_reward của level_max từ 5_000_000_000 → 2_000_000_000
  (trong khoảng INT an toàn, vẫn là phần thưởng lớn nhất hệ thống).
- Bump version 3.1.0 → 3.1.1; CHANGELOG.md thêm entry v3.1.1.
- Push main + tag v3.1.1 → CD workflow auto-trigger → build + deploy.
- Wait ~10 phút cho CD pipeline (CI gate + Docker build + Coolify deploy
  + verify healthy) → workflow SUCCESS.

Stage Summary:
- ✅ Migration 024 giờ apply thành công trên prod (DB đã có BIGINT total_xp,
  125 danh hiệu trong catalog, rps_plays + word_chain_plays tồn tại).
- ✅ Container healthy, web live tại https://louis.vangioitutien.com.
- ✅ /health trả {"status":"ok","version":"3.1.1"}.
- ✅ /api/v1/health trả {database:up, pool:4, version:3.1.1}.
- ✅ Route /rps và /word-chain trả 303 (redirect login, đúng behavior).
- ⏭️ User test đăng nhập và thử 2 game mới + xem 100 danh hiệu mới.

---

## [3.4.0] — 2026-08-30 — Feedback system + AI Agent login rework + UI mobile fixes

### Tổng quan
Bản phát hành tập trung: (1) hệ thống góp ý 2 chiều user→admin, (2) rework
hoàn toàn đăng nhập AI Agent sang username + mật khẩu có thời hạn do admin
đặt, (3) fix toàn bộ lỗi UI mobile (comment tin tức chữ dọc, bảng xếp hạng
tràn), (4) tạm dừng arcade với trang "đang được Hieu Louis xem xét",
(5) hiệu ứng hồ sơ AI mới theo accent color, (6) báo cáo hoạt động AI
công khai (sanitized), (7) upgrade CI/CD actions hết cảnh báo Node 20.

### Chi tiết kỹ thuật
- **Migrations mới**: 028 `ai_agent_credentials` (Argon2id + thời hạn +
  lockout + functional index LOWER(username)), 029 `user_feedback` (+ enum
  feedback_category/feedback_status + notification_type thêm
  'feedback_status'), 030 cập nhật bio/capabilities GLM 5.3.
- **Argon2id** (crate argon2 0.5): hash tại auth.rs (salt qua encode_b64
  tránh xung đột rand_core 0.6 vs rand 0.10).
- **Login AI**: verify_password_login (atomic lockout CASE WHEN, dummy
  hash timing-equalizer, expiry check sau verify, uniform error messages).
- **Feedback**: 5 danh mục, security chỉ admin (filter SQL WHERE $n OR
  category != 'security'), notification ngoài transaction, rate-limit
  10/24h, page_url chặn //, /\, CR/LF.
- **Comment news restructure** giống comment game (avatar flex-shrink:0) —
  root-cause chữ dọc: author-link chiếm hết flex width ép body về 0.
- **Arcade gate**: const ARCADE_UNDER_REVIEW trong handlers/mod.rs — gate
  cả page + play/match/move endpoints.
- **Audit độc lập 2 vòng** (agent riêng): vòng 1 phát hiện 2 HIGH (base64
  decode sai byte — đã đổi hướng bỏ hẳn, XSS list_replies) + 6 MED; vòng 2
  verify lại + phát hiện 1 HIGH (locked_until không reset → không re-lock
  được) + 2 MED (confirm dialog kép, data-confirm trên button). Tất cả đã
  fix + kèm giải thích trong code.

### Kiểm định trước release
- cargo check / clippy -D warnings / rustdoc -D warnings: PASS
- cargo test: 352/352 PASS
- node --check app.js: PASS
- CI/CD local gates tương đương CI GitHub Actions

### Deploy
- Tag v3.4.0 → CI (fmt/check/clippy/test/doc/audit) → CD (build image
  GHCR + deploy Coolify + verify /health version) → Release tự tạo từ
  CHANGELOG.

---
## 2026-08-31 — v3.6.0: Admin XP Boost + micro-cache + 74 câu đố + quét-fix 400/500

**Nhiệm vụ**: (1) thêm nhiều câu hỏi hằng ngày; (2) mục admin XP boost
1000 XP/0,15s start/stop (chỉ admin thấy); (3) web load cực nhanh KHÔNG đổi
UI; (4) fix nút đăng nhập AI Agent trên hồ sơ glm53; (5) fix "cực nhiều"
lỗi 400/500; (6) quét-fix bảo mật; (7) fix GitHub Actions triệt để; (8)
tạo các bản phát hành.

**GitHub Actions** (ưu tiên #1): run v3.5.1 đã xanh cả 3 workflow — các
fail cũ (Release v3.4.2 bash -e; CD main/v3.5.0 deploy tranh chấp) do
7974a7f đã vá. Việc còn lại: merge Dependabot #9 (uuid 1.26, CI xanh) +
deploy.yml thêm paths-ignore cho doc-only (bỏ deploy vô ích, giảm nguy
cơ tranh chấp).

**Quét độc lập (6 agent)**: trivia/quests, bug nút AI Agent, audit hiệu
năng, quét 400/500, audit bảo mật, chuẩn bị XP boost. Kết quả đã fix
trong v3.6.0 (chi tiết CHANGELOG): panic byte-slice OAuth, OAuth 500→400
thân thiện, plain-text 4xx/5xx được trang trí giao diện (nguồn lớn nhất
"nhiều lỗi 400"), like-comment nhân bản, delete-game/news swap sai,
report modal khách, form collection 400, ETag RSS không khớp, >4MB 500
rỗng, sw.js precache rác 350KB, restore impersonation TTL 30d→4h, staff
AI login mất phiên gốc, janitor dọn impersonation_tickets, nút AI Agent
dùng role thay ai_profile fail-open.

**Hiệu năng**: micro-cache anonymous TTL 5s (MICRO_CACHE_SECS, hit
x-micro-cache), precompressed .br/.gz ở Docker build + ServeDir
precompressed_*, sitemap cache 10p, gộp also_liked/has_downloaded vào
wave, compression bỏ font/*, fetchpriority cover, REQUEST_TIMEOUT OnceLock.

**Tính năng**: XP Boost (/admin/xp-boost — 4 route, state AppState +
task janitor::run_xp_boost 1000XP/150ms, partial HTMX poll 1s, audit,
tự dừng 20 lỗi DB); migration 034 +74 câu đố (bank 90 câu, 3→5
câu/ngày).

**Kiểm định**: cargo fmt + clippy -D warnings + 353/353 test + rustdoc
-D warnings — sạch toàn bộ trên Rust 1.98.0.

---
## 2026-08-31 — v3.6.1: HOTFIX micro-cache OnceLock

Sau khi v3.6.0 deploy xong, verify prod bằng curl (Accept: text/html)
thiếu header `x-micro-cache: hit` — root cause: nhánh lookup + store của
micro_cache_mw đều dùng `MICRO_CACHE.get()` (chỉ đọc, không khởi tạo) →
OnceLock unset vĩnh viễn → middleware no-op. Fix: helper `micro_cache_map()`
dùng `get_or_init` cho cả 2 nhánh + 4 unit test tower-oneshot (hit/bypass
session/bypass HTMX/bypass non-allowlist) — chạy 5 lần ổn định. 357/357 test
pass, clippy -D warnings sạch.

---
Task ID: v3.6.2-superfix
Agent: Super Z (main)
Task: Fix GitHub Actions triệt để (ưu tiên 1) + hồ sơ AI Agent /ai/ + nút
admin login-as + fix "thanh tím nhấp nháy" + hồ sơ GLM 5.3 bớt lag +
quét-fix 400/500 + quét bảo mật vòng 21 + release v3.6.2.

Work Log:
- Clone repo, quét 43k dòng: routes/profile/shop/comments/middleware/
  templates/3 workflows. Dựng env thật: PostgreSQL 17.6 local + Rust
  1.98.0, chạy migrate 34 file + smoke test 90 request + deep test
  (user thật, admin thật, mua hộp XP, bình luận, admin pages, probe
  prod https://louis.vangioitutien.com).
- GitHub Actions: xem logs API các run fail lịch sử → nguyên nhân chính
  verify step 3 phút quá ngắn khi deploy serialize. FIX: verify 40×15s
  = 10 phút; pin SHA toàn bộ 8 action third-party (checkout/rust-cache/
  install-action/setup-buildx/login/metadata/build-push).
- /ai/{username}: route mới + /ai/{username}/repos; /u/{ai} 303 redirect;
  User::profile_href() + test; template layout/game/admin/profile;
  json_ld; chat.js fix selector cũ hỏng (a.avatar-linkref syntax error).
- Thanh tím #htmx-progress: skip background triggers (load/revealed/every)
  + pendingRequests counter chống nhấp nháy overlap + rAF batch.
- Hero FX GLM 5.3: tĩnh mặc định, JS initHeroFx() probe (CPU/RAM/reduced-
  motion/FPS 45) mới thêm .fx-full; bỏ blur(52px); orbs/scan transform-
  only; bỏ vignette breathing + blur mobile.
- 400/500: error_page_mw render error partial thân thiện cho HTMX
  (rejection text/plain thô → HTML tiếng Việt); app.js responseError
  đọc .error-message từ response → toast đúng nguyên nhân; CatchPanicLayer
  panic → 500 sạch thay vì connection reset.
- Bảo mật vòng 21: thu hẹp maintenance bypass /ai/ → chỉ /ai/info +
  /ai/progress; re-audit 27 is_admin check, upload magic bytes, WS
  origin, cookie flags, sanitize_redirect, escape_like — nguyên trạng.
- Verify end-to-end trên local build: /ai/glm53 200, /ai/testuser 404,
  /u/glm53 303, /ai/info 401 (không đụng router), nút admin chỉ hiện
  cho admin, HTMX lỗi trả partial đẹp, daily cap 5 hộp đúng message.

Stage Summary:
- v3.6.2: 11 file code + 2 workflow + CHANGELOG/WORKLOG, 358/358 test
  pass, build/clippy sạch Rust 1.98.0.
- Key decision: redirect /u/{ai} → /ai/{ai} thay vì sửa từng link
  (100% link cũ tự hoạt động); hero FX static-first + capability gate.

---
Task ID: 2
Agent: Super Z (main)
Task: HOTFIX v3.6.3 — /ai/glm53 404 trên prod (role Moderator).

Work Log:
- Probe prod sau deploy v3.6.2: /health=3.6.2 OK, HTMX error partial OK,
  NHƯNG /ai/glm53=404 + /u/glm53=200 (không redirect).
- Điều tra: prod glm53 mang role Moderator (data drift — đổi tay qua
  admin). Toàn bộ cơ chế v3.6.2 dựa trên role → tắt lặng lẽ.
- FIX 2 lớp: (1) is_ai_agent_user() = role AiAgent HOẶC google_sub
  'ai_agent:default-glm53' — áp cho route/redirect/profile/impersonate;
  (2) migration 035 khôi phục role + require_admin chặn AI Agent vào
  /admin/* (hole: bot với role staff truy cập dashboard).
- 359 test pass; mô phỏng prod local (glm53 role=moderator):
  /ai/glm53=200, /u/glm53=303, nút admin hiện, hero FX render.

Stage Summary:
- Commit + tag v3.6.3: prod glm53 luôn là AI Agent mọi trạng thái role;
  vá hole AI-Agent-staff vào admin.

---
## 2026-08-31 — v3.7.0: KHUNG AVATAR (Rồng Lửa 5000 XP) + shop x3 + admin sửa AI Agent + fix GH Actions

**Nhiệm vụ**: (1) fix GitHub Actions triệt để — ưu tiên 1; (2) fix admin
đăng nhập không vào được tài khoản AI Agent; (3) admin sửa thông tin
chi tiết/thông số AI Agent; (4) thêm nhiều vật phẩm cửa hàng; (5) thêm
khung avatar nhiều kiểu — đặc biệt khung Rồng Lửa bán cực đắt, vẽ chuẩn,
NHÌN để fix; (6) quét-fix UI/UX desktop + mobile; (7) quét bảo mật;
(8) tạo bản phát hành.

**GitHub Actions**: quét 500 run API — v3.6.0→v3.6.3 xanh đủ 3 workflow;
fail cũ (Release v3.4.2 bash -e, CD 2026-08-30 verify 3') đã được các
bản trước vá. Fix còn lại latent v3.7.0: release.yml shell injection
qua tag name (tag git hợp lệ chứa `$(`/backtick — interpolate thẳng
vào run: = RCE với GITHUB_TOKEN) → mọi bước truyền TAG qua env + regex
siết `[A-Za-z0-9._-]`; deploy.yml verify thêm điều kiện trigger.queued
+ healthy-wait fail-fast exited/stopped:unhealthy.

**Xây env thật để NHÌN**: Rust 1.98.0 (rustup) + PostgreSQL 17.6
user-space (zonky binary, port 5433) + migrate 36 file. Phát hiện
sandbox env `DATABASE_URL=file:...` đè .env (dotenvy không override) →
app nối nhầm localhost:5432 — relaunch bằng `env -u`. Dựng user test
(admin + user 999999 XP) bằng session thật để browse.

**Khung avatar** (migration 036 + Rust + CSS): 6 khung — Đồng 150 /
Bạc 300 / Vàng 600 / Neon 900 / Phượng Hoàng 1500 / **Rồng Lửa 5000 XP
(đắt nhất, unit-test guard)**. Vẽ thuần CSS: conic metallic ×3, neon
pulse, phoenix xoay, dragon 2-lớp vảy+lửa xoay 3.2s + flicker hào quang;
@property --frame-angle; prefers-reduced-motion tôn trọng. Bẫy đã né:
pseudo không render trên <img> → class đặt trên thẻ bọc; chat.js dùng
whitelist class cứng; session cache invalidate khi mua để hiện ngay.
Hiển thị 3 vị trí (profile 96px / header 32px / chat) — verify bằng
screenshot cả 3.

**Shop**: tách 2 khu (Khung Avatar có swatch preview / Booster);
Rồng Lửa hero card full-width grid-areas + nhãn "👑 ĐẮT NHẤT CỬA HÀNG";
+2 vật phẩm mới (name_glow_7, xp_boost_3d); duration_hours từ DB
(guard ≤0); mua frame → invalidate cache + toast riêng.

**Admin sửa AI Agent**: trang /admin/ai-agents/{id}/edit (GET+POST)
sửa display_name/model/vendor/version/caps/màu/privacy/verified/bio/
avatar + param edit inline POST params/{param_id}/edit; audit log;
validation tiếng Việt; test thật: sửa bio ✓, sửa param ✓, khôi phục
dữ liệu test bằng chính endpoint mới.

**Login-as AI Agent**: verify end-to-end (impersonate → /admin 403
đúng spec → stop khôi phục admin); password login /auth/ai/login sai
đúng → error thân thiện. Hardening: verify_password_login + 4 handler
admin đổi sang is_ai_agent_user() (chống role drift — root cause lịch
sử của bug này).

**UI/UX**: fix tương phản tên trên cover tối (light mode chữ trắng +
shadow — nhìn trước/sau bằng screenshot); mobile 390px kiểm tra
home/shop/profile — clean, không overflow; desktop 1280px toàn bộ.

**Kiểm định**: fmt sạch · clippy -D warnings sạch · 362/362 test ·
rustdoc -D warnings sạch trên Rust 1.98.0. Smoke 25+ request thật.
---
Task ID: v3.9.0-superfix
Agent: Super Z (main) + 3 sub-agent audit song song (A: handlers/auth, B: middleware/infra, C: bug/UI/template)
Task: Fix 403 admin sửa hồ sơ AI Agent + xóa toàn bộ hiệu ứng hồ sơ GLM 5.3 (trắng) + fix ảnh đại diện bị che + thêm Lịch sử phát triển Louis Space vào /about + quét-fix bảo mật toàn codebase. Rust 1.98, prod-ready, releases tương ứng.

Work Log:
- Clone repo v3.8.1, đọc WORKLOG/CHANGELOG, dựng env thật: Rust 1.98.0 (rustup) + PostgreSQL 17.6 (zonky, port 5433), 42 migrations sạch.
- 3 luồng quét song song (agent độc lập): (A) auth/handlers — 55/55 handler admin có check quyền, SQLi/IDOR/XSS sạch, 1 LOW mới; (B) middleware/infra — 1 HIGH (CF-Connecting-IP spoof), 2 MEDIUM, 4 LOW; (C) bug/UI — xác nhận bug 403, root cause avatar bị che, danh sách hiệu ứng cần xóa, 4 bug khác.
- FIX 403 (root cause): update_profile + edit_profile_form (handlers/ai_agent.rs) check role.is_ai_agent() thuần → role drift = 403 oan cho admin login-as. Đổi sang is_ai_agent_user() (role HOẶC google_sub "ai_agent:") tại 5 điểm code: 2 handler + require_ai_agent + AuthAiAgent (middleware.rs) + 4 template (layout/index/game.show/profile.show).
- FIX avatar bị che (root cause thật): .profile-page-ai .profile-cover { position: relative } (v3.4.0) đưa cover sang phase positioned → vẽ ĐÈ lên vùng avatar chồng cover (margin-top −40px). Xóa cùng hiệu ứng + thêm position:relative; z-index:1 phòng vệ cho .profile-info.
- XÓA hiệu ứng GLM 5.3 ("trắng"): hero FX full màn (aurora/sao/lưới/orbs/quét/vignette), cover gradient + sheen, avatar breathe glow, tên shimmer, badge pulse, viền gradient + scanline, LIVE dot blink — ~13.8KB CSS + IIFE initHeroFx (app.js). GIỮ: badge AI Agent, model info, params card, báo cáo hoạt động. Tách keyframes riêng cho dot arcade (đang mượn ai-dot-blink).
- FIX kèm theo: bio AI handler 500→1000 (khớp maxlength form) + vendor maxlength 100→50; confetti dọn bằng document.querySelectorAll (frag rỗng sau appendChild — 45 node/lần tồn tại vĩnh viễn); fetch notif-read thêm .catch; collections/reviews .len() → chars().count() (tiếng Việt bị siết 2-3 lần); xác minh báo cáo selector a[href^="#"] "hỏng" là DƯƠNG TÍNH GIẢ (byte-level check).
- BẢO MẬT: [HIGH] bỏ cf-connecting-ip khỏi header tin cậy (site không sau Cloudflare — Traefik không ghi đè, client tự gắn xoay bucket rate-limit + poison IP audit; giữ X-Real-Ip do Traefik sinh); [LOW] thu hồi session admin gốc NGAY khi impersonate (phiên bị ghi đè cookie nhưng row sống tới 30 ngày = credential mồ côi); [LOW] guard server-side gamification/shop cho AI Agent (do_checkin/spin/trivia/buy — trước đây chỉ ẩn UI); [LOW] maintenance bypass thêm /ai/progress.json; fix comment drift SESSION_KEY + .env.example TTL 90→30. Audit xác nhận sạch: CSRF fail-closed, upload magic-bytes, Argon2id+lockout, không SQLi (AssertSqlSafe), không leak PII API.
- FEATURE: /about thêm "Lịch sử phát triển Louis Space" — timeline 7 cột mốc v0.x→v3.9.0 + CSS timeline riêng.
- MIGRATION 041: reset role ai_agent cho TOÀN BỘ google_sub LIKE 'ai_agent:%' drift (mở rộng 035 — idempotent). 042: báo cáo hoạt động v3.9.0 công khai trên hồ sơ GLM 5.3 (5 entry sanitized).
- Version 3.9.0: Cargo.toml + CACHE_VERSION sw.js + fallback app.js.
- VERIFY end-to-end (server thật, role drift mô phỏng prod glm53 = moderator): login-as → GET /profile/ai/edit = 200 (trước fix: 403); POST /profile/ai = 303 → /ai/glm53; admin session gốc bị thu hồi (0 row còn lại); /impersonate/stop khôi phục admin mới → /admin = 200; /ai/glm53 không còn ai-hero-fx/profile-page-ai, giữ badge + params + activity (5 entry v3.9.0 hiện); /about có timeline.
- cargo check/clippy -D warnings/fmt sạch; 351/351 test PASS (Rust 1.98.0).

Stage Summary:
- 22 file code + 2 migration mới, 0 schema breaking — deploy an toàn.
- Sẵn sàng commit/tag v3.9.0 → CD deploy + GitHub Release tự tạo từ CHANGELOG. Lesson carried: push main, đợi CD xong rồi mới tag.
