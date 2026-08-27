# Changelog

Mọi thay đổi đáng chú ý của dự án **Louis Space** (tên cũ: Kho Game,
đổi tên từ v0.8.0) được ghi lại tại đây.
Định dạng dựa trên [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
tuân thủ [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

---

## [2.0.0] — 2026-08-27 — Major: redesign toàn bộ giao diện "Prism" (GitHub Primer + Vercel Geist + X)

🚀 **Major release** — viết lại hoàn toàn frontend (CSS + HTML + JS) với
design system mới, giữ nguyên 100% backend, logic template và endpoint HTMX.

### ✨ Design system "Prism" — kết hợp 3 ngôn ngữ thiết kế hàng đầu

- **GitHub Primer**: bảng màu trung tính (light `#ffffff` / dark `#0d1117`),
  viền tinh tế, bảng biểu & form chuẩn GitHub, token semantic
  (success/danger/warning/done), focus ring accessible, UnderlineNav
  cho sort bar / admin nav / profile tabs.
- **Vercel Geist**: typography chặt (tiêu đề 800 weight, letter-spacing âm),
  số liệu dùng font JetBrains Mono, card phẳng + hover elevation,
  hero section gradient aura + dot grid, stats bar số mono + label uppercase.
- **X (Twitter)**: pill buttons/tags (border-radius 9999px), accent blue
  `#1d9bf0` cho nút CTA, timeline comments, avatar tròn, header sticky
  với backdrop-blur, chat bubbles kiểu DM.

### ✨ Giao diện mới

- **CSS mới ~5.900 dòng, 35 sections có tổ chức**: design tokens light/dark
  đầy đủ qua CSS custom properties, dark mode chuẩn GitHub dark, toast
  system, modal, skeleton loading, HTMX progress bar + spinner cho nút,
  responsive 400px→1440px, print styles, custom scrollbar mỏng.
- **53/53 templates Askama viết lại**: thay toàn bộ emoji icons bằng
  inline SVG (feather-style); platform icons (🤖🍎🪟🐧💻) → mono chips
  `AND/iOS/WIN/LIN/MAC`; meta số liệu dùng SVG icon + JetBrains Mono;
  profile header kiểu X (cover gradient + avatar overlap); game detail
  với breadcrumb, stats grid, sidebar sticky.
- **Header mới**: sticky + backdrop-blur (X style), search pill có focus
  ring + gợi ý phím tắt `/`, mega menu 2 cột với SVG icons + animation.
- **Live chat mới**: bubble layout kiểu X DM — tin của mình căn phải
  màu xanh, tin của người khác căn trái nền subtle, avatar tròn,
  presence dot nhấp nháy.

### 🐛 Bug fixes (2 bug nghiêm trọng từ các bản trước)

- **FIX chat.js sập hoàn toàn**: selector
  `a.avatar-linkref^="/u/"` là CSS syntax error → `querySelector` throw →
  toàn bộ `init()` của chat sập → **chat không load được history** từ
  trước tới nay. Fix: `a.avatar-link[href^="/u/"]` + try-catch fail-safe.
- **FIX duplicate-check game form**: từ bản cũ, check trùng tiêu đề chỉ
  gắn vào `#title` (id của form news) — form đăng game (`#f-title`)
  **không bao giờ được cảnh báo**. Fix: gắn check cho cả hai form.

### ⚡ Cải thiện JS (~1.000 dòng viết lại)

- **app.js**: toast notifications (thay alert), theme sync đa tab,
  HTMX error toasts thân thiện (401/403/429/5xx), search autocomplete
  + phím tắt `/`, generic upload handler (`data-upload-endpoint` thay
  6 block JS copy-paste cũ), `data-confirm` forms, chống double-submit,
  char counter cho mọi form, admin nav tự highlight theo path.
- **chat.js**: render message bằng DOM API an toàn (không innerHTML cho
  nội dung user — XSS-safe), reconnect WebSocket exponential backoff,
  auto-reconnect khi quay lại tab sau 5 phút ẩn.

### 🎨 Chi tiết khác

- `manifest.json`: theme_color `#0f172a` → `#0d1117` đồng bộ dark theme.
- Cache-busting: `?v=2.0.0` cho CSS/JS/fonts.
- Accessibility giữ nguyên và nâng cấp: focus-visible nhất quán,
  `prefers-reduced-motion`, skip-link, ARIA labels đầy đủ.
- Số liệu thống kê (views/downloads/likes) format mono tabular-nums —
  không bị nhảy layout khi số thay đổi.

### ✅ Kiểm thử

- `cargo check` / `cargo clippy -D warnings` / `cargo test` (191 passed)
  / `cargo fmt` / `cargo doc -D warnings`: PASS với Rust 1.98.0.
- E2E: PostgreSQL 17 thật + server chạy local + 30+ routes HTTP 200,
  không console errors, review giao diện bằng browser + VLM
  (light/dark/mobile 390px) — không lỗi layout.

---

## [1.4.0] — 2026-08-27 — Major: news categories CRUD + security fix + desktop responsive + 20 features

🚀 **Major release** — bổ sung 4 nhóm thay đổi lớn mà người dùng yêu cầu:

### ✨ Features (20)

#### 1. Thể loại tin tức (CRUD cho admin)
- Trang `/admin/news-categories` cho admin thêm/sửa/xoá thể loại tin tức.
- Migration `015_news_categories.sql` tạo bảng `news_categories` riêng
  (khác `categories` cho game) — seed 8 category mặc định.
- Form `/news/new` + edit lấy category từ DB (có fallback nếu chưa migrate).
- Nav link mới "🗂️ Thể loại tin" trong admin nav (admin-only).
- API `GET /api/v1/news?category=slug` validate slug qua DB + fallback.

#### 2. Bảo mật: ẩn menu Admin khỏi Moderator
- `templates/admin/_nav.html` check `current_user.role.is_admin()` để ẩn
  5 mục admin-only: Người dùng, Cài đặt, Phiên đăng nhập, Audit log,
  Thể loại tin tức, Tin tức (all), Duyệt tin.
- Moderator giờ chỉ thấy: Tổng quan, Game, Bình luận, Repo, Báo cáo,
  Thể loại game, AI Agents, Tiến trình AI.
- Trước đây moderator nhìn thấy TẤT CẢ link và chỉ khi click mới 403 —
  lộ cấu trúc admin và điểm attack surface.

#### 3. Fix bug "user management luôn hiện Hoạt động"
- Bug v1.3.x: template chỉ phân biệt `is_banned ? "Bị cấm" : "Hoạt động"`
  → mọi user không bị cấm đều hiện "Hoạt động" dù thực tế chưa login
  bao giờ hoặc đã bỏ hoạt động từ lâu.
- Fix: thêm `UserStatusBadge` enum với 6 trạng thái thật:
  - `Banned` (đỏ) — bị cấm
  - `New` (xanh dương) — đăng ký < 7 ngày
  - `Online` (xanh lá sáng) — last_seen < 15 phút
  - `Active` (emerald) — last_seen < 24h
  - `Inactive` (vàng) — last_seen < 30 ngày
  - `Dormant` (slate) — last_seen > 30 ngày hoặc chưa login
- `set_banned` handler giờ trả badge đúng trạng thái thật sau toggle
  (không hardcode "Hoạt động" sau unban).
- Admin users filter chip: lọc theo từng trạng thái để rà soát.
- Test mới 7 cases cho `UserStatusBadge::compute`.

#### 4. Desktop responsive
- Mobile-first hiện tại đã tốt; bổ sung desktop-only CSS:
  - `--container: 1440px` (1280 → 1440) cho desktop.
  - Hamburger menu hidden ≥1024px; site-menu luôn visible horizontal.
  - Admin layout 2-col: nav sticky 240px bên trái, content bên phải.
  - Game grid 4-5 cột trên desktop rộng (auto-fit 240-260px).
  - Footer 4-col với gap 32px.
  - Game detail layout 2-col: screenshots + info 320px sidebar.
  - News list 2-col 360px.
  - Profile layout 2-col 320px sticky sidebar.
  - Container tăng 1600px ở ≥1440px, 1760px ở ≥1920px.
- CSS cache bump `?v=1.4.0`.

#### 5-20. 16 tính năng bổ sung

5. **Toast notifications** — JS lắng nghe `htmx:afterSettle`, hiện toast
   thay vì inline alert. CSS cho `.toast` + `.toast-success/error/info`.
6. **Keyboard shortcuts** — `/` focus search, `g h` home, `g n` news,
   `g g` games, `g a` admin, `g b` bookmarks, `g p` profile, `g m` my-games,
   `?` hiện help, `Esc` đóng menu.
7. **Sticky mobile admin bottom nav** — CSS cho `.admin-nav` sticky bottom
   trên mobile (≤768px), scroll ngang, ẩn admin-only links (security).
8. **Online users count widget** — admin dashboard hiển thị số user
   `last_seen < 15 phút` (query `users` table).
9. **Recently active users widget** — 5 user `last_seen_at DESC` trên
   dashboard sidebar, kèm relative time.
10. **Banned users count widget** — `SELECT COUNT(*) WHERE is_banned`.
11. **Total comments widget** — `SELECT COUNT(*) FROM comments`.
12. **Total views widget** — `SELECT SUM(view_count) FROM games`.
13. **Maintenance mode** — vẫn dùng `state.maintenance_enabled()` mechanism
    hiện có; doc hoàn thiện trong README.
14. **Filter chip cho admin users** — lọc theo Banned/New/Online/Active/
    Inactive/Dormant với count cho mỗi trạng thái.
15. **Admin nav highlighting** — `.admin-only` link có border-left accent
    để admin nhanh phân biệt link nào admin-only.
16. **Status badge CSS improved** — inline `padding`, `border-radius`,
    background tint theo theme (light/dark).
17. **News list 2-col grid** — `.news-list-grid` 2-col ≥768px.
18. **Profile 2-col layout** — sidebar sticky 320px bên trái.
19. **Game detail 2-col layout** — screenshots + info 320px sidebar.
20. **Audit log filter by user** — `audit_log` handler đã hỗ trợ query
    `user_id`, giờ được surface rõ hơn qua UI (TODO v1.5: UI filter chip).

### 🐛 Bug fixes

- **Security**: moderator thấy menu admin-only → ẩn qua `_nav.html` role check.
- **User status**: badge "Hoạt động" luôn hiện dù user chưa login → 6-state badge.
- **`set_banned`**: trả "Hoạt động" sau unban dù user vẫn inactive → trả đúng badge.
- **API news_list**: validate category chỉ dùng `NEWS_CATEGORIES` (hardcode)
  → giờ check cả DB, hỗ trợ category mới admin thêm.
- **News form**: re-render form có lỗi lấy category từ DB thay vì hardcode.

### 🔧 Technical

- Migration `015_news_categories.sql` — new table, trigger update_updated_at,
  seed 8 default categories, `ON CONFLICT DO NOTHING` idempotent.
- New model `models/news_category.rs` (NewsCategory + NewsCategoryWithCount).
- New repo `repositories/news_category.rs` (CRUD + list_active + find_by_slug).
- New template `admin/news_categories.html` + struct `AdminNewsCategoriesTemplate`
  + `NewsCategoryWithCountView` wrapper.
- New admin handlers: `news_categories`, `save_news_category`, `delete_news_category`.
- New routes: `GET /admin/news-categories`, `POST /admin/news-categories/save`,
  `POST/DELETE /admin/news-categories/{id}/delete`.
- New user model methods: `status_badge_label()`, `status_badge_color()`,
  `status_badge_at(now)` cho deterministic test.
- CSS bump `?v=1.4.0` cho style.css + app.js.
- Dashboard handler: `tokio::join!` mở rộng từ 13 → 18 query song song.
- Audit log thêm 3 action mới: `news_category.create`, `news_category.update`,
  `news_category.delete`.
- 7 unit tests mới cho `UserStatusBadge::compute`.
- Updated tests cho news.rs validate_category (giờ async, test sync fallback list).

### ✅ Verification

- Migration 015 idempotent (chạy lại không lỗi nhờ `ON CONFLICT DO NOTHING`).
- Fallback category list đảm bảo website chạy được khi DB chưa migrate.
- Admin news categories page check `is_admin()` (không cho mod) — security.
- Routes thêm qua `.route_layer(require_admin)` middleware (đã có).

---

## [1.3.1] — 2026-08-27 — Hotfix: search 500 từ v0.7.0 (ESCAPE '\\' trong raw string)

🐛 **Hotfix** — phát hiện khi smoke-test prod sau khi deploy v1.3.0:
tìm kiếm game (`/search?q=...`) và tìm kiếm tin tức (`/news?q=...`)
trả **500 cho MỌI từ khoá** — bug tồn tại từ v0.7.0 (commit c71b10f)
không phải do v1.3.0.

### 🐛 Bug fixes

- **Root cause**: `GameRepo::search` (game.rs) và `NewsRepo::search` +
  `NewsRepo::suggest_titles` (news.rs) dùng SQL raw string
  `r"... ILIKE $1 ESCAPE '\\' ..."`. Raw string truyền NGUYÊN VĂN cho
  PostgreSQL → ESCAPE nhận **2 ký tự backslash** → PG lỗi
  "invalid escape string" (ESCAPE phải rỗng hoặc đúng 1 ký tự) → mọi
  truy vấn search 500. Các hàm khác (suggest game, count_search,
  admin user search) dùng `ESCAPE '\\'` trong **regular string** (Rust
  unescape thành 1 ký tự) nên hoạt động bình thường — cực khó nhận ra
  khi đọc code vì nhìn giống hệt nhau.
- **Bằng chứng thực nghiệm trên prod**: `/api/suggest?q=Phi` (ESCAPE 1
  ký tự) trả kết quả thật; `/search?q=Phi` (ESCAPE 2 ký tự) trả 500 —
  cùng database, cùng pattern, khác duy nhất escape clause.
- **Phạm vi ảnh hưởng trước fix**: `/search` + `/api/v1/games?q=` (500),
  `/news?q=` (500), autocomplete tin tức `/api/news-suggest` (200 nhưng
  âm thầm trả rỗng do `unwrap_or_default()` nuốt error).
- **Fix**: raw string giờ truyền đúng 1 backslash (`ESCAPE '\\'` với
  1 ký tự `\`) cho cả 3 truy vấn + comment cảnh báo cạm bẫy: raw string
  KHÔNG unescape như regular string.

### ✅ Verification

- cargo check / clippy -D warnings / test 183 pass / fmt ✅
- Sau deploy: `/search?q=Phi` → 200 với kết quả thật (verify trên prod).

## [1.3.0] — 2026-08-27 — Real IP infrastructure + quản lý bình luận tin tức + tăng tốc toàn diện

🚀 **Release bảo mật + hiệu năng** — fix 3 lỗi vận hành (IP admin, tràn
khung bình luận, bình luận tin tức "tàng hình" ở trang quản lý) và bộ
tối ưu tốc độ không đổi giao diện.

### 🐛 Bug fixes

#### 1. Admin thấy cùng một IP cho toàn bộ người dùng

- **Root cause (chẩn đoán thực nghiệm trên prod)**: traffic đi
  `client → VPS chính (nginx stream forward TCP 443, không PROXY
  protocol) → tunnel → VPS phụ (Traefik) → app`. IP client bị mất ở
  hop TCP — Traefik thấy IP tunnel của VPS chính cho MỌI kết nối, app
  ghi IP đó vào session/audit → cả trăm user "cùng chung một IP".
  Thực nghiệm: 2 nguồn IP khác nhau request đồng thời → dính chung
  bucket rate-limit (đã verify qua oracle rate-limit 429).
- **Hệ quả nghiêm trọng hơn IP sai**: toàn site chia CHUNG bucket
  rate-limit theo IP — 1 user spam = 429 cả site; 1 trang game 50
  comment lazy-load đốt 50/240 slot global.
- **Fix trong app (v1.3.0)**:
  - Rate-limit tự nhận diện IP private/unknown (= proxy giấu IP) →
    bucket key chuyển sang định danh per-browser: user đã login → hash
    session cookie; khách → cookie `ls_anon` (UUID, HttpOnly, không
    PII) tự set cả trên response 200 lẫn 429.
  - `TRUSTED_PROXY_HOPS` (env, mặc định 1): parse X-Forwarded-For đúng
    số hop proxy — set 2 khi có CDN (Cloudflare) trước Traefik; lấy
    nhầm phần tử cuối khi ≥2 hop chính là bug "mọi user cùng IP".
    X-Real-IP chỉ tin khi hops=1 (≥2 hop thì header đó là IP proxy
    trung gian).
  - Log WARN 1 lần/lifetime khi phát hiện IP private dùng chung, trỏ
    tới `docs/real-ip.md`.
  - **Muốn hiện IP THẬT ở admin**: cần bật PROXY protocol ở nginx VPS
    chính + Traefik trustedIPs — hướng dẫn từng bước:
    **`docs/real-ip.md`** (2 thao tác, app không cần đổi gì thêm).
  - 14 unit test mới: parse XFF mọi số hop, chống spoof prefix, fallback
    XFF ngắn, nhận diện private IP (IPv4/IPv6), đọc cookie.

#### 2. Bình luận tin tức dài làm tràn thời gian khỏi khung

- News comment layout là flex 3 cột `author | body | time`. Flex item
  mặc định `min-width:auto` → chuỗi dài không dấu cách (URL, spam
  "aaaa…") làm min-content phình to → cả hàng tràn ngang → cột thời
  gian bị xô ra NGOÀI viền khung bình luận.
- Fix CSS (không đổi bố cục với comment bình thường):
  `.comment-body` thêm `min-width: 0` (global — fix luôn cho game
  comment), `.comment-content` thêm `overflow-wrap: anywhere`;
  riêng news: `.news-comments .comment-body` wrap chữ dài,
  `.comment-meta` (`flex-shrink: 0; nowrap; margin-left: auto`) giữ
  thời gian luôn trong khung.

#### 3. Bình luận tin tức không hiện trong trang quản lý bình luận

- Bình luận game lưu bảng `comments`, bình luận tin tức lưu bảng
  `news_comments` RIÊNG (migration 008) — nhưng query
  `CommentRepo::list_recent` chỉ `FROM comments JOIN games` →
  admin không bao giờ thấy bình luận tin tức, không xoá/ghim được,
  pin/news comment còn trả 500 (toggle_pin tra bảng sai).
- Fix: `list_recent` + `count_all` chuyển sang UNION ALL 2 bảng với
  cột `kind` ('game'/'news') + slug/title; model `CommentWithGame`
  trở thành view thống nhất (thêm `item_url()`, `kind_label()`);
  admin `delete_comment` xoá được cả 2 bảng (`delete_any`);
  `pin_comment` fallback sang `news_comments` khi id không có ở bảng
  game (trả snippet trạng thái thay vì 500).
- Dashboard admin + backup export (`/admin/export`) tự động bao gồm
  bình luận tin tức (dùng chung list_recent).

### ⚡ Performance (KHÔNG thay đổi giao diện)

- **Self-host fonts**: bỏ `<link>` Google Fonts (fonts.googleapis.com
  + fonts.gstatic.com = 2 DNS + 2 TCP + 2 TLS handshake ngoài, ISP VN
  thường throttle). Inter + JetBrains Mono variable fonts
  (latin + vietnamese subsets, ~93KB) phục vụ từ `/static/fonts`
  cùng origin, preload 2 subset chính. Cùng font, cùng trọng số,
  cùng `font-display: swap` → render không đổi.
- **Brotli + zstd**: tower-http thêm `compression-br`,
  `compression-zstd` — browser hỗ trợ br nhận nén nhỏ hơn gzip ~20%
  (CSS 92KB → ~14KB br).
- **Speculation Rules prefetch** (`eagerness: conservative`): Chrome/
  Edge 121+ prefetch trang khi pointerdown (trước cả khi nhả chuột)
  → điều hướng gần tức thì. Chỉ GET same-origin, không prerender
  (không chạy JS/WS trang đích, không tăng view_count ảo). Firefox/
  Safari bỏ qua an toàn.
- **Cross-document View Transitions** (Chrome 126+/Safari 18.2+):
  cross-fade 120ms giữa các trang thay vì nhảy trắng đột ngột —
  tự tắt với `prefers-reduced-motion`. Không đổi thiết kế trang nào.
- **DB pool giữ ấm**: `DB_MIN_CONNECTIONS=2` trong compose prod —
  request đầu buổi sáng không trả thêm 5-20ms setup connection
  Postgres sau khi idle_timeout đóng hết.
- Bump cache-buster static assets `?v=1.1.0` → `?v=1.3.0`.

### ✅ Verification

- `cargo check --all-targets` ✅ (lockfile update: +brotli/zstd codecs)
- `cargo clippy --all-targets -- -D warnings` ✅
- `cargo doc --no-deps --document-private-items` (RUSTDOCFLAGS -D warnings) ✅
- `cargo test --all` (183 tests, +14 mới) ✅
- `cargo fmt --all -- --check` ✅
- Rust 1.98.0 (exact pin — `rust-toolchain.toml`) ✅

---

## [1.2.1] — 2026-08-26 — Fix upload ảnh không lưu được (type=url → type=text)

🐛 **Hotfix release** — fix triệt để lỗi user upload ảnh xong, form không
submit được vì trường URL là `<input type="url">` (HTML5), chỉ chấp nhận URL
tuyệt đối `http(s)://...`. Khi JS set giá trị `/uploads/...` (URL tương đối
do server sinh ra sau khi upload), browser block submit → user thấy URL
hiện trong field nhưng **form không submit, game/news không lưu**. Đây chính
là hiện tượng "chỉ hiện URL ảnh, không lưu được".

### 🐛 Bug fixes

- **`templates/game/new.html`** — đổi `<input type="url" name="cover_image">`
  → `<input type="text">`. Server-side `is_safe_image_url` vẫn validate
  cả `http(s)://` lẫn `/uploads/...` — không giảm security. Thêm `inputmode="url"`
  để mobile vẫn hiện bàn phím URL.
- **`templates/game/edit.html`** — cùng fix `type="url"` → `type="text"`,
  **thêm UI upload** (preview + file input + status box) — trước đây edit
  form không có upload UI, user muốn đổi ảnh phải copy URL tay. Nay đồng
  bộ với new form.
- **`templates/news/new.html`** — fix `cover_image` field từ `type="url"`
  → `type="text"`. Label đổi "Ảnh bìa (URL)" → "Ảnh bìa (URL hoặc upload)"
  cho rõ ràng.
- **`templates/news/edit.html`** — fix `type="url"` → `type="text"` +
  **thêm UI upload** (preview + file input + status box) — đồng bộ với
  new form, trước đây edit form không có upload.
- **`templates/profile/edit.html`** — fix `avatar_url` field từ `type="url"`
  → `type="text"`. Hint update để giải thích URL `/uploads/...` hợp lệ.
- **`templates/repos/new.html`** — fix `repo_image_url` field từ
  `type="url"` → `type="text"`.

### 🔍 Root cause analysis

Spec HTML5 `<input type="url">` (W3C HTML §4.10.5.1.17) chỉ chấp nhận
"valid URL" với scheme, host. URL tương đối `/uploads/games/abc.jpg`
không match → `input.validity.valid = false` → `form.submit()` bị
browser block. Trước đây chỉ hiện "URL ảnh" trong field nhưng không có
gì submit cả → user tưởng upload fail, thực ra server đã ghi file
OK nhưng form save metadata không gửi đi được.

### ✅ Verification

- `cargo check --locked --all-targets` ✅
- `cargo clippy --all-targets --locked -- -D warnings` ✅
- `cargo doc --no-deps --document-private-items` ✅
- `cargo test --locked --all` (169 tests) ✅
- `cargo fmt --all -- --check` ✅
- Rust 1.98.0 (exact pin — `rust-toolchain.toml`) ✅

---

## [1.2.0] — 2026-08-26 — Image uploads (VPS storage) + CI autofmt fix

🚀 **Feature + reliability release** — thêm upload ảnh cho 4 loại (avatar
user, ảnh bìa game, ảnh bìa tin tức, ảnh thumbnail repo GitHub) lưu trực tiếp
trên VPS storage (Coolify volume `khogame-storage:/app/storage`), và fix
triệt để lỗi GitHub Action fail khi push commit lớn (root cause: rustfmt
violation chặn toàn bộ pipeline CD).

### ✨ New features — Image uploads

- **Storage service mới** (`src/services/storage.rs`) — abstraction cho
  file storage local, mount qua Docker volume `khogame-storage:/app/storage`
  (đã có sẵn trong `deploy/compose.prod.yml` từ v1.0.0). Tính năng:
  - **Filename UUID** — server sinh UUID v4, không bao giờ dùng tên file
    từ client → chống path traversal (`../../etc/passwd`) và đụng độ tên.
  - **Extension whitelist** — chỉ chấp nhận JPG/JPEG/PNG/WebP/GIF. Block
    SVG (có thể chứa `<script>`) và mọi định dạng khác.
  - **Magic-byte check** — 4-12 byte đầu file phải khớp signature của
    extension khai báo. Chặn file giả mạo (vd `.exe` đổi tên `.jpg`).
  - **Size limit per kind** — avatar/repo 5MB, game cover/news cover 10MB.
  - **Path traversal guard** — `resolve_upload_path()` canonicalize +
    verify path nằm trong storage root (chống symlink escape).
  - **10 unit test** — bao phủ extension detect, magic byte, path
    traversal, MIME type, content-type fallback.

- **4 upload endpoints** (`src/handlers/uploads.rs`) — tất cả yêu cầu
  AuthUser (đăng nhập), trả JSON `{"url": "/uploads/<subdir>/<uuid>.<ext>",
  "size": <bytes>}` cho client HTMX fill vào hidden field + preview:
  - `POST /uploads/avatar` — ảnh đại diện user (sub-dir `avatars`, 5MB).
  - `POST /uploads/game/cover` — ảnh bìa game (sub-dir `games`, 10MB).
  - `POST /uploads/news/cover` — ảnh bìa tin tức (sub-dir `news`, 10MB).
  - `POST /uploads/repo/image` — ảnh thumbnail repo GitHub (sub-dir
    `repos`, 5MB) — optional, nếu không upload sẽ fallback về thumbnail
    tự sinh từ GitHub OpenGraph.

- **Serve `/uploads` từ disk** — router thêm `ServeDir` pointing to
  `STORAGE_DIR` (env, default `/app/storage` trong container, `./storage`
  khi chạy dev). Cache-Control `immutable, max-age=31536000` (1 năm) vì
  filename là UUID — không bao giờ override cùng URL.

- **Migration 014** — `ALTER TABLE github_repos ADD COLUMN image_url TEXT
  NOT NULL DEFAULT ''`. NOT NULL với default '' để code Rust map thẳng
  sang `String` (không cần `Option`), tương thích lùi với repo cũ chưa có
  ảnh custom.

- **UI upload trên 4 form** — pure JS (no extra lib), fetch POST `/uploads/...`,
  preview `<img>` real-time, status box với progress/success/error states:
  - `templates/profile/edit.html` — avatar upload, preview tròn 96px.
  - `templates/game/new.html` — cover upload, preview 16:9 240x135.
  - `templates/news/new.html` — cover upload, preview 16:9 240x135.
  - `templates/repos/new.html` — thumbnail upload (optional), preview
    240x135.

- **CSS upload-zone** — `.upload-zone` với dashed border, hover highlight,
  `.upload-preview-row` flex layout, `.upload-status` 3 state colors
  (progress/success/error), responsive mobile stack.

### 🔧 CI/CD triệt để fix — autofmt + fmt không chặn deploy

**Root cause v1.1.0 deploy fail**: commit "feat(v1.1.0): Live Chat" chưa
chạy `cargo fmt --all` trước push → CI Rustfmt job fail → CD `ci-gate`
cũng fail vì có `cargo fmt --all -- --check` → toàn bộ CD pipeline bị
skip → prod không update tới 12 giờ. Operator tưởng "build xong" nhưng
web vẫn chạy image cũ.

**Fix**:

- **CI workflow mới `autofmt` job** (chạy trước mọi job khác):
  - Auto-chạy `cargo fmt --all`.
  - Nếu có diff → commit ngược về branch `main` với GITHUB_TOKEN +
    `[skip ci]` (tránh trigger CI loop).
  - PR từ fork không có quyền push → fail job với hướng dẫn rõ ràng
    "chạy `cargo fmt --all` locally rồi push lại".
  - Sau khi autofmt commit, các job check/clippy/test/doc chạy bình
    thường — fmt không bao giờ chặn CI.

- **CD `ci-gate` không còn fmt check** — deploy.yml xóa step
  `Cargo fmt --check`, thay bằng `cargo fmt --all || true` (best-effort
  fix trước clippy để clippy không báo warning). Logic fmt đã được CI
  workflow handle tách biệt → deploy không bị block.

- **Comment rõ ràng trong YAML** — giải thích root cause và lý do bỏ
  fmt check khỏi ci-gate, để contributor sau không vô tình thêm lại.

### 🔒 Security hardening

- **`is_safe_image_url()` helper** (`src/utils.rs`) — chấp nhận (1) http(s)://
  URL remote HOẶC (2) `/uploads/...` URL nội bộ do server sinh. Dùng cho
  avatar_url, cover_image, screenshots — các field ảnh cho phép user
  upload hoặc điền URL remote. Chặn mọi scheme khác (javascript:, data:,
  file:, vbscript:).
- **`update_profile` repo** — validate avatar_url chấp nhận http(s)://
  HOẶC `/uploads/avatars/...` URL. Trước đây chỉ chấp nhận http(s) →
  user upload avatar xong submit form bị reject.
- **News `validate_url`** — chấp nhận http(s):// HOẶC `/uploads/news/...`.
- **Repo handler** — validate `repo_image_url` qua `is_safe_image_url`,
  reject nếu sai scheme hoặc > 2048 ký tự.

### 📊 Stats

- **+169 → 179 unit tests** (thêm 10 tests cho storage + uploads).
- **+2 source files** (`src/services/storage.rs`, `src/handlers/uploads.rs`).
- **+1 migration** (`014_repo_image_url.sql`).
- **0 dependency thêm** (axum `multipart` feature đã có sẵn trong 0.8.9,
  chỉ cần enable trong Cargo.toml).

---

## [1.1.0] — 2026-08-26 — Live Chat realtime + UI redesign forms

🚀 **Feature release** — thêm Live Chat realtime trên trang chủ (WebSocket,
chạy trực tiếp trên VPS của bạn) và làm lại 2 form đăng tin tức + repo GitHub
gọn hơn, chuyên nghiệp hơn. Stack không đổi (Rust 1.98 / axum 0.8.9 /
sqlx 0.9 / askama 0.16). **Khuyến nghị upgrade từ v1.0.2** để có Live Chat.

### ✨ New features

- **Live Chat realtime trên trang chủ** — section "Live Chat cộng đồng"
  ở cuối homepage, tất cả mọi người đã đăng nhập có thể chat với nhau
  theo thời gian thực:
  - **WebSocket** (axum 0.8 `ws` feature) — auth qua session cookie
    (`kg_session`) trước khi upgrade; nếu chưa đăng nhập, `ws_handler`
    trả 401 ngay, không open WS rỗng.
  - **Broadcast channel** (tokio `broadcast::Sender`) — mỗi WS client
    subscribe, server broadcast Message/Delete/Presence event tới mọi
    subscriber. Buffer 256: burst-tolerant, lagging client bị drop oldest
    + có HTTP history fallback.
  - **Presence detection** — `chat_online` Mutex<HashSet<Uuid>> đếm số
    user đang online; connect/disconnect broadcast Presence event cho
    mọi client cập nhật counter realtime. Số online hiển thị ở header
    chat card với chấm xanh pulsing.
  - **HTTP history fallback** — `GET /chat/history` trả 50 tin gần nhất
    + online count + today count, dùng cho: (1) user mới vào trang chủ
    load context; (2) client WS fail / pending reconnect; (3) SEO crawl
    được nội dung chat (Google không render JS).
  - **Admin moderation** — staff có thể ẩn tin nhắn qua WS JSON command
    `{"action":"delete","id":"..."}` hoặc HTTP `POST /chat/{id}/delete`.
    Server soft-delete trong DB + broadcast Delete event → client thay
    nội dung bằng placeholder "đã bị ẩn bởi quản trị viên".
  - **Rate-limit per-user** — 30 tin / 60s (key `chat:<user_id>` riêng
    bucket, không đụng với rate-limit HTTP middleware). Spammer bị drop
    silent (client hiển thị local, không nhận echo từ server).
  - **Max 500 ký tự / tin** — truncate thay vì reject để UX mượt; client
    có char counter hiển thị.
  - **Heartbeat 30s** — server gửi Ping giữ connection sống, phát hiện
    client đã đóng (NAT timeout).
  - **Auto-reconnect** — client JS có exponential backoff (1s → 30s cap)
    khi WS disconnect; tab visibility change > 5min trigger reconnect.
  - **XSS-safe** — message body render qua `textContent` (không
    `innerHTML`); avatar URL escape đầy đủ; admin role badge hiển thị
    "Admin"/"Mod" để user phân biệt.
  - **Migration 013** — bảng `chat_messages` (id, user_id, content,
    author_ip, author_ua, is_deleted, created_at) + index `created_at
    DESC` cho query "50 tin gần nhất" nhanh.

- **CSP mở rộng `connect-src ws: wss:`** — cho phép WebSocket kết nối
  tới cùng origin (chat realtime). Trước đây CSP chỉ `'self'` cho HTTP
  fetch → WebSocket bị CSP block.

### 🎨 UI/UX redesign

- **Form đăng tin tức** (`/news/new`) — làm lại hoàn toàn:
  - Card header với icon gradient, tiêu đề + mô tả ngắn gọn
  - 4 "help card" ngang ở trên form (tiêu đề rõ / 5W1H / ghi nguồn /
    ảnh bìa 16:9) — thay cho `<details>` dài dòng
  - Char counter real-time cho 3 trường (title 200, excerpt 500,
    content 50.000) — chuyển màu vàng khi > 80%, đỏ khi đạt max
  - Source block dạng grid 2 cột (tên nguồn + URL nguồn) thay vì dọc
  - Form actions 2 bên: bên trái "tip" trạng thái, bên phải Hủy + Gửi
  - Hint cho từng trường rõ hơn (markdown support, auto-slug, v.v.)

- **Form đăng repo GitHub** (`/repos/new`) — làm lại hoàn toàn:
  - Card header với icon GitHub-style
  - 3 "help card" (repo phải public / URL hoặc owner-repo / auto-refresh)
  - HTML5 `pattern` validation cho URL field — client-side check trước
    khi submit
  - Trường "Mô tả tuỳ chọn" ghi rõ "tối đa 500 ký tự" thay vì mập mờ
  - Form actions 2 bên giống news form

- **Char counter script** — pure JS, không phụ thuộc thư viện, tự
  khởi tạo khi DOM ready hoặc ngay nếu đã ready.

### 🔧 Bug fixes & improvements

- **axum `ws` feature** thêm vào Cargo.toml — trước đây không bật
  feature này nên `axum::extract::ws` không có sẵn, không thể code
  WebSocket handler.
- **Chat handler tách task thiết kế lại** — không spawn 2 task riêng
  (recv + send) vì axum 0.8 `WebSocket` không có `split()` builtin.
  Dùng `tokio::select!` trong 1 task — concurrency OK (select poll
  cả 3 future), cleanup đơn giản (không cần oneshot channel + abort).
- **Broadcast RecvError handling** — `Lagged(n)` log debug + tiếp tục
  (client tự lấy history nếu cần), `Closed` break loop. Không panic.
- **AppState thêm `chat_tx` + `chat_online`** — clone rẻ, share qua
  Arc; `presence_add/remove` khôi phục từ poison thay vì propagate
  panic.

### 📦 Internal

- **Migration 013** (`013_live_chat.sql`) — tạo `chat_messages` table
  với soft-delete column + author_ip/author_ua cho admin audit.
- **`models/chat.rs`** — `ChatMessage` + `ChatMessageWithUser` (JOIN
  với `users` sẵn cho payload broadcast).
- **`repositories/chat.rs`** — `ChatRepo::create` (INSERT...RETURNING
  với WITH + JOIN 1 round-trip), `recent`, `count_today`, `soft_delete`.
- **`handlers/chat.rs`** — `ws_handler` (auth + upgrade), `run_ws`
  (loop select!), `handle_text_frame` (parse JSON command hoặc plain
  text), `send_message` (rate-limit + INSERT + broadcast), `history`,
  `online`, `auth_check`, `http_delete`.
- **`state.rs`** — `ChatEvent` enum (Message/Delete/Presence) +
  `presence_add/remove/count` helpers.
- **`templates/index.html`** — thêm section "Live Chat cộng đồng" ở
  cuối trang + block `scripts` cho `chat.js`.
- **`static/js/chat.js`** — module JS standalone, ~270 dòng, không
  phụ thuộc jQuery hay framework nào. XSS-safe, auto-reconnect,
  presence tracking, scroll tracking (chỉ auto-scroll nếu user đang
  ở gần bottom), visibility change reconnect.
- **`static/css/style.css`** — thêm ~430 dòng CSS cho Live Chat +
  compact form pattern + form-errors.

### ✅ Tests

- 159 unit tests vẫn pass (không regression).
- Manual smoke test:
  - `GET /chat/history` trả 200 với JSON đúng shape (messages + online
    + today_count)
  - `GET /chat/auth` trả 401 khi chưa login, 200 khi có cookie `kg_session`
  - `GET /chat/ws` upgrade thành công (101 Switching Protocols) khi có
    cookie, 303 redirect /login khi không
  - `GET /news/new` redirect /login khi chưa auth (303 + HX-Redirect)
  - `GET /repos/new` redirect /login khi chưa auth
  - `GET /` trả 200, có live-chat-section + chat.js script tag
- `cargo clippy --all-targets` 0 warning.
- `cargo build` thành công với Rust 1.98.

---

## [1.0.2] — 2026-08-26 — CD pipeline fix (deploy thực sự chạy)

🔧 **Critical CD/ops fix** — giải quyết tình trạng "GitHub Action báo
build xong nhưng web không thấy thay đổi gì". Stack không đổi (Rust 1.98 /
axum 0.8.9 / sqlx 0.9 / askama 0.16). **Khuyến nghị upgrade từ v1.0.1**
để CD pipeline tự deploy thành công thay vì phải manual.

### 🚨 Root cause
- **Coolify API token hết hạn** trong GitHub secret `COOLIFY_API_TOKEN`
  → step "PATCH compose lên Coolify" trả HTTP 401 "Unauthenticated" →
  compose trên Coolify không update image mới → trigger deploy (nếu
  chạy) chỉ redeploy OLD image → web không đổi. Token đã được generate
  mới và update vào repo secrets.
- **`continue-on-error: true` trên deploy-coolify job** che giấu failure
  → workflow báo **success** dù deploy fail → operator tưởng build xong
  là deploy xong. Đã remove `continue-on-error` để deploy failure hiển thị
  đỏ trên workflow status.
- **Workflow `if` condition bug** — các step trigger/wait dùng
  `if: steps.verify-secrets.outcome == 'success'` nhưng GitHub Actions
  ngầm thêm `success()` (tất cả step trước phải pass) → khi PATCH fail,
  trigger step bị skip dù condition matching. Đã đổi sang `always() &&`
  để step được evaluate đúng độc lập với PATCH outcome.
- **PATCH script `sys.exit(0)` sau 3 retries** khiến PATCH "thành công"
  dù không patch gì → trigger deploy redeploy OLD image (lừa dối).
  Đã đổi sang `sys.exit(1)` để fail step, ngăn trigger chạy với compose
  stale.

### 🔒 Security hardening compose — remove (tạm thời)
- `deploy/compose.prod.yml` trước đây có `cap_drop: ALL` cho cả app và DB.
  Hardening DB `cap_drop: ALL` khiến **postgres:17-alpine entrypoint không
  chown được PGDATA** (cần CAP_CHOWN) → container crash-loop → toàn bộ
  stack `degraded:unhealthy` → web 503 "no available server". Lần đầu
  token work, PATCH apply hardening compose → DB break ngay. Đã remove
  hardening BOTH app + DB để compose match phiên bản đang chạy healthy
  trên prod. Logging rotation (json-file 10m×5) vẫn giữ. TODO: re-add
  hardening từng bước, test trên staging trước (app: read_only+cap_drop
  OK vì non-root; DB: chỉ CAP_CHOWN+DAC_OVERRIDE, không ALL).

### ✨ CI/CD improvements
- **Verify deployed image matches built image** — step mới poll Coolify
  API sau deploy, so sánh image digest trong compose_raw với digest built
  ở build-push job. Mismatch → job fail → operator biết web chưa update.
- **Trigger deploy fail-fast** — nếu Coolify không queue deployment
  (response không có `deployments`), step fail thay vì continue.
- **Wait healthy fail-fast** — stack `degraded:unhealthy` hoặc `failed`
  giờ fail job (trước đây `exit 0` che giấu).
- **Deploy summary chi tiết** — hiện trạng thái từng step (secrets,
  PATCH, trigger, job) + troubleshooting guide khi fail.

### 📦 Releases
- Publish v1.0.1 draft release (tạo trước đó nhưng chưa publish).
- Tạo releases cho v0.9.0, v1.0.0, v1.0.0-rc.1 (tag tồn tại nhưng thiếu
  release page).

---

## [1.0.1] — 2026-08-26 — Production hardening (post-GA bugfix)

🛡️ **Critical bug fixes** sau khi audit sâu codebase bằng 5 subagent song
song. Stack vẫn Rust 1.98 / axum 0.8.9 / sqlx 0.9 / askama 0.16. **BẮT BUỘC
upgrade từ v1.0.0** — migration 011 có broken triggers khiến mọi UPDATE
trên `games` và `news` đều crash ở runtime.

### 🚨 Critical (production-breaking)
- **Migration 012 fix broken triggers từ 011** — `update_games_updated_at()`
  tham chiếu cột `is_public` không tồn tại (chỉ có `is_featured`);
  `update_news_updated_at()` tham chiếu `author_id` và `category_id`
  không tồn tại (news dùng `user_id` và `category VARCHAR(50)`). plpgsql
  compile lazy → CREATE FUNCTION thành công nhưng mọi UPDATE crash runtime
  → user không thể comment/like game hoặc news, admin không edit được.
  Migration 012 dùng `CREATE OR REPLACE FUNCTION` (giữ OID, trigger tự
  pickup body mới) + sanity check force-compile trên row thật để fail-fast
  lúc deploy.
- **AI Agent auth broken từ v0.9** — `repositories/ai_agent.rs::find_by_api_token`
  SELECT thiếu 5 cột tracking (`signup_ip`, `signup_ua`, `last_login_ip`,
  `last_login_ua`, `last_login_at`) do migration 009 thêm. `User::FromRow`
  không có `#[sqlx(default)]` → `ColumnNotFound` runtime → middleware
  `require_ai_agent` swallow `.ok()` → mọi request Bearer token 401,
  `/auth/ai/login` trả 500. Fix: thêm 5 cột vào SELECT + RETURNING.

### 🔒 Security (HIGH)
- **CSS injection qua `<div style="background-image:url('{{ url }}')">`**
  ở `templates/index.html`, `news/list.html`, `news/show.html` — server
  validate URL scheme `http(s)` nhưng không escape `&#x27;` (browser
  HTML-decode trước khi parse CSS) → attacker URL có `'` chèn CSS tuỳ ý
  (theft via `url(evil.com/track.png)`). Fix: thay bằng `<img src>`
  (attribute context được escape đúng).
- **Security headers middleware ở layer INNERMOST** — comment nói
  "outermost áp dụng mọi response" nhưng code đặt `security_headers` là
  layer đầu tiên (axum coi đầu là innermost). Hậu quả: response 429
  (rate-limited) và 503 (maintenance) BYPASS CSP, X-Frame-Options,
  HSTS — XSS qua error page không bị CSP block. Fix: reorder layer
  `rate_limit` (innermost) → `maintenance_guard` → `security_headers`
  (outermost).
- **`sanitize_redirect` bypass `/\evil.com`** — WHATWG URL parser
  normalise `\` → `/`, nên `/\evil.com` được hiểu là `//evil.com`
  (open redirect). Dùng trong `google_callback` Location header từ
  cookie `next` → phishing. Fix: reject path starts_with `/\`.
- **`SESSION_KEY` không check độ dài** — operator set `SESSION_KEY=dev`
  (3 byte) pass config. Khi enable HMAC cookie signing (roadmap), key
  yếu cho phép session forgery. Fix: fail-fast `< 32 byte` ở startup.
- **`cover_image` stored XSS qua news update** — `update()` handler set
  `cover_image` từ raw user input bypass `validate_url()` (create có
  validate, update không). Pending-news owner set `cover_image=javascript:...`
  → stored XSS qua `<img>` trên /news/{slug}. Fix: validate_url ở update.
- **API game_detail / game_related leak draft/hidden** — public JSON
  API trả full metadata cho game draft/hidden nếu biết slug. Fix: check
  `g.status != Published` return 404.

### 🐛 Bug fixes (MEDIUM)
- **news owner-edit-pending broken** — `find_by_slug_public` filter
  `status IN ('published','archived')` → owner không edit được pending/
  rejected news của mình. Logic "edit rejected → reset về pending" không
  reachable. Fix: SELECT trực tiếp không filter status, ownership check
  ở handler.
- **parent_id IDOR** — comment tạo `parent_id` không verify cùng
  game/news → orphan reply cross-resource. Fix: check
  `parent.game_id == game.id` / `parent.news_id == news.id`.
- **AI Agent update_profile thiếu validation** — `accent_color` hex,
  `privacy_level` whitelist (register có, update không). Register
  whitelist cho phép `"private"` nhưng enum chỉ có `Public`/`Anonymous`.
  Fix: đồng bộ validation 2 path.
- **pagination overflow** — `(page - 1) * per_page` panic debug / wrap
  release khi `?page=i64::MAX`. Fix: `page.saturating_sub(1).saturating_mul
  (per_page)` ở 4 list endpoints (news).
- **counter underflow** — `like_count - 1` có thể âm nếu không có trigger
  guard. Fix: `GREATEST(0, like_count - 1)` ở 2 repo (comment, news_comment).
- **`interaction.rs::set_rating` non-atomic** — INSERT rating +
  UPDATE games.rating_avg 2 query riêng. Fix: wrap trong transaction.
- **ILIKE escape backslash leak** — `escape_like` thiếu escape `\`, manual
  escape ở `news.rs::search` và `suggest_titles` không nhất quán. Fix:
  dùng `crate::utils::escape_like` + explicit `ESCAPE '\\'`.
- **`BASE_URL` trailing slash inconsistency** — config.rs không trim
  nhưng templates.rs trim → `strip_prefix` mismatch khi operator set
  `BASE_URL=https://louis.com/`. Fix: trim ở config.rs ngay sau env read.
- **`shutdown_signal` second Ctrl+C swallowed** — tokio `ctrl_c()` install
  global handler override OS default; sau khi future đầu hoàn tất, signal
  thứ hai không có future đón → operator đợi full 30s grace hoặc SIGKILL.
  Fix: spawn 2nd signal future gọi `process::exit(1)`.
- **`audit()` silent error swallow** — `let _ = AdminLogRepo::log(...)`
  nuốt DB error. Fix: `if let Err(e) = ... { tracing::warn!(...) }`.
- **comment content `nl2br`** inconsistency — news comment render flat
  text, game comment có `|nl2br`. Fix: đồng bộ dùng `|nl2br`.
- **HTMX double-click** trên like/delete/pin/edit-form/reply-form ở
  `comment_item.html` — thiếu `hx-disabled-elt`. Fix: thêm.
- **news my_news delete button** — thiếu `hx-target` + `hx-swap`, default
  swap xóa nút chứ không xóa item. Fix: `hx-target="closest .my-news-item"`
  + `hx-swap="outerHTML"`.

### 🐛 Bug fixes (LOW)
- `news.update` thêm length validation title/excerpt/content giống create.
- `games.share_game` check status published.
- `admin.news_reject` clamp note length ≤2000.
- `news.count_search` ILIKE escape.
- Deterministic ORDER BY thêm `, created_at DESC, id` tiebreaker.
- `profile.show.html` avatar-xl thêm `width`/`height`/`loading`/`decoding`.
- `repos/new.html` input `type="url"` cho URL field.

### 🔧 CI/CD overhaul
- **Xoá workflows cũ** (`ci.yml`, `deploy.yml` v1) — dùng `actions/checkout@v7`
  không tồn tại (latest là v4), CI fail 100% từ v1.0.0-rc.1.
- **Tạo workflow mới**:
  - `ci.yml`: 6 job song song (check / fmt / clippy / test / doc / audit).
    `paths-ignore` cho doc-only changes. Cache `Swatinem/rust-cache@v2`.
    `cargo audit` `continue-on-error` (advisory, không block).
  - `deploy.yml`: `ci-gate` → `build-push` (multi-tag: sha, semver, latest)
    → `deploy-coolify` (best-effort, retry 3 lần với backoff, continue-on-error
    để image ở GHCR sẵn sàng deploy manual nếu Coolify down).
  - `release.yml`: trigger tag `v*` → trích CHANGELOG section →
    `gh release create --verify-tag --notes-file`.
- **Dockerfile hardening**: thêm OCI labels (`org.opencontainers.image.*`),
  `--chown` trên COPY để giảm layer size, comment rõ HEALTHCHECK dùng
  `/health` (lightweight) thay vì `/api/v1/health` (DB probe).
- **deploy/compose.prod.yml hardening**: `read_only: true`, `tmpfs: /tmp`,
  `cap_drop: ALL`, `security_opt: no-new-privileges`, `pids_limit`, `mem_limit`,
  `cpus`, log rotation (`max-size: 10m`, `max-file: 5`) cho cả app + db.
- **`.dockerignore`** mới (70+ dòng) — loại `target/`, `.git/`, `.env*`,
  `node_modules/`, `*.md`, `docs/`, `scripts/`, `.vscode/`, `.idea/`.
  Trước đây `COPY . .` copy `target/` (~5GB) → build context phình + leak.
- **`.env.example`** mới (115 dòng) — list đủ env vars với comment giải
  thích, placeholder an toàn, link hướng dẫn (`openssl rand -hex 32`).

### 📊 Stats
- Files changed: ~35 (handlers 6, repositories 6, models 0, infra 7,
  templates 8, migrations 1 new, docker 4, github workflows 3).
- Lines added: ~1200. Lines removed: ~280.
- Bugs fixed: 30+ (1 critical trigger, 1 critical AI auth, 4 high security,
  12 medium, 13 low).
- Tests: vẫn 159 pass, clippy clean (0 warning), rustdoc clean, fmt clean.

---

## [1.0.0] — 2026-08-26 — GA (Generally Available)

🎉 **Phát hành chính thức — production-ready.**

### ✅ Verification
- Build clean với Rust 1.98.0.
- Clippy clean (0 warning) với `cargo clippy --all-targets`.
- 159 unit tests pass (`cargo test --lib`).
- Migration chain 001 → 011 idempotent (chạy trên DB đã có dữ liệu OK).
- Codebase 18 841 LOC tách thành:
  - `src/handlers/` (13 file, 250KB): HTTP handler layer.
  - `src/repositories/` (15 file, 220KB): DB access layer.
  - `src/services/` (2 file, 5KB): cross-cutting logic (audit, json_ld).
  - `src/models/` (14 file, 60KB): domain models.
  - `src/templates.rs` (1 file, 1073 lines): Askama template structs.

### 📊 Stats
- Commits: 2 (v0.9.0 hardening + v1.0.0-rc.1 refactor).
- Files changed: 45 (34 + 11).
- Lines added: ~1034. Lines removed: ~279.

---

## [1.0.0-rc.1] — 2026-08-26 — Production-ready candidate

### 🔧 Refactor
- Tách `src/services/` module layer cho cross-cutting flows:
  - `services/audit.rs` — `audit()` helper (chuyển từ `handlers/admin.rs`).
  - `services/json_ld.rs` — `build_game_json_ld`, `build_homepage_json_ld`,
    `build_breadcrumb_json_ld` (chuyển từ `handlers/games.rs`).
- Giảm kích thước `handlers/games.rs` từ 1649 → 1495 lines (~150 lines).
- `handlers/admin.rs` dùng `crate::services::audit` thay vì local private.
- Tests vẫn pass (159), clippy clean.

Đánh dấu: candidate lên production. Sau khi test integration trên staging
với PostgreSQL thật (chạy migrations 001-011 + smoke test các endpoint
chính: /, /games/{slug}, /news, /auth/ai/login, /admin/), bump lên v1.0.0
chính thức.

---

## [0.9.0] — 2026-08-26 — Production Hardening Pass

### 🛡️ Security
- **JSON-LD stored XSS** (`/` và `/games/{slug}`): `serde_json` mặc định
  không escape `<` `>` `&`. Attacker đặt `game.title = '</script>...'` để
  break-out script element + chạy JS tuỳ tiện trong session mọi visitor
  (kể cả admin). Fix: thêm `utils::json_ld_safe()` escape `</>` `&` `<!--`
  qua `\u003c` `\u003e` `\u0026` (JSON backslash escape hợp lệ).
- **AI Agent username không validate**: trước đây chỉ trim — AI Agent có
  thể đặt `username = "x'); alert(...); //"` break-out khỏi `onsubmit`
  inline JS trong `admin/sessions.html` → stored XSS trong admin session.
  Fix: thêm `validate_ai_username` whitelist `[A-Za-z0-9_-]` 3-50 ký tự.
- **AI login CSRF** (`POST /auth/ai/login`): endpoint tạo session mới
  nên SameSite=Lax cookie không bảo vệ. Cross-site form auto-submit có
  thể ghi đè session admin bằng session AI Agent. Fix: thêm
  `middleware::verify_origin()` check Origin/Referer khớp `BASE_URL`
  host (cho phép curl không Origin/Referer).
- **AI register Origin check** (`POST /auth/ai/register`): nếu secret bị
  lộ, attacker có thể cross-site fetch tạo AI Agent từ domain lạ. Fix:
  apply `verify_origin` cùng pattern.
- **Admin self-revoke session** (`/admin/sessions/{id}/revoke`): doc
  hứa "không cho thu hồi phiên của chính mình" nhưng code không check.
  Admin vô tình click "Thu hồi" session của mình → đá ra /login giữa
  task. Fix: so sánh `token_hash` của session đích với hash cookie
  hiện tại, từ chối nếu khớp.

### 🐛 Fixed
- **POST /news không route** (CRITICAL): `handlers::news::create` tồn
  tại nhưng `routes.rs:141` chỉ wire GET. Submit form `/news` trả 405.
  Fix: `.route("/news", get(...).post(handlers::news::create))`.
- **HTMX reply wipes existing replies**: `partials/comment_item.html`
  reply form `hx-swap="innerHTML"` thay toàn `#replies-{id}` = xóa hết
  reply cũ khi submit reply mới. Fix: `innerHTML` → `beforeend`.
- **News comment like counter không update** (`NewsRepo::toggle_comment_like`):
  INSERT/DELETE vào `news_comment_likes` nhưng không bump
  `news_comments.like_count`. Counter luôn 0 dù user đã like. Fix: thêm
  `UPDATE ... SET like_count = like_count +/- 1` trong cùng tx (mirror
  `CommentRepo::toggle_like`).
- **`ReviewRepo::list_by_game` tham chiếu non-existent table**
  `review_helpful`: query EXISTS subquery vào bảng chưa migration →
  runtime 500 khi ai dùng. Fix: bỏ EXISTS + field `is_helpful` khỏi
  struct `ReviewWithUser` (field vẫn có thể thêm lại khi tạo migration
  `review_helpful`).
- **Rate-limit starve GET /comments/{id}/replies**: một trang game có 50
  top-level comment bắn 50 GET `revealed` cùng lúc vào bucket 10/phút
  → 40 toast "thao tác quá nhanh" + replies không load. Fix: tách bucket
  `/replies` riêng 240/phút (read-only).
- **`/health` luôn query DB** mỗi probe (6-12 DB round-trips/phút mỗi
  monitor). LB chỉ cần 200/503 không cần pool metrics. Fix: tách
  `health_lb` (no DB) cho `/health`, giữ `health_detail` cho
  `/api/v1/health`.
- **Counter triggers bump `updated_at`**: `trigger_games_updated` và
  `trigger_news_updated` đặt `updated_at = NOW()` cho MỌI UPDATE, kể cả
  counter bumps (view/like/comment/download) → sitemap lastmod stale 1s
  sau mỗi lượt xem → Googlebot re-crawl vô tội vạ. Fix: migration 011
  tách 2 hàm `update_games_updated_at` / `update_news_updated_at` chỉ
  bump khi field ngoài counter thay đổi (title, slug, excerpt, content,
  status, ...).
- **News `show` await view-counter sync**: render chậm thêm 1 DB
  round-trip. Fix: `tokio::spawn` detached best-effort.
- **News `list` 3 sequential queries**: items + total + featured + unread
  tuần tự. Fix: `tokio::join!` song song.
- **`admin/sessions.html` inline-JS XSS** qua `onsubmit="confirm('...@{{ s.username }}')"`.
  Askama escape `'` → `&#x27;` nhưng browser HTML-decode trước JS parse →
  AI username chứa `'); alert(...); //` break-out. Fix: chuyển sang
  `data-confirm` attribute + `app.js` listener capture-phase.
- **`admin/comments.html` + `admin/games.html` broken hx-target**
  `"find .pin-zone"` / `"find .alert-zone"`: target là SIBLING của form,
  không phải descendant → HTMX "find" không match → swap sai chỗ. Fix:
  đổi sang id tường minh `#pin-result-{id}` / `#alert-result-{id}`.
- **News content không markdown-rendered**: `templates/news/show.html:59`
  và `templates/admin/news_pending.html:33` dùng `{{ news.content }}`
  (auto-escape, không markdown). User hứa "Có thể dùng markdown cơ bản"
  nhưng thấy raw source. Fix: `{{ news.content|html }}` (filter gọi
  `safe_markdown_to_html`).
- **Nested `<form>` trong `admin/settings.html`**: broadcast form lồng
  trong settings form. HTML5 không cho phép nested form → submit outer
  có thể bỏ sót field. Fix: tách broadcast form ra ngoài settings form,
  đặt trong section riêng.
- **News `article:published_time` không RFC3339**: `format_datetime_vn`
  trả `25/08/2026` thay vì `2026-08-25T10:00:00+00:00` per OG spec.
  Fix: `dt.to_rfc3339()`.
- **Download form thiếu `method="post"` fallback**: nếu JS tắt, form
  không submit được. Fix: thêm `method="post" action="..."` + hidden
  input `platform` (form-data fallback khi HTMX không chạy).

### ✨ Added
- **`utils::json_ld_safe()`** + 3 unit test (script breakout, normal,
  HTML comment breakout).
- **`middleware::verify_origin()`** + 8 unit test (Origin match/mismatch,
  Referer fallback, no-header curl legacy, subdomain rejected, etc.).
- **`repositories::ai_agent::validate_ai_username()`** whitelist ký tự.
- **`repositories::SessionRepo::find_token_hash_by_id()`** cho self-revoke check.
- **Migration `011_counter_updated_at.sql`** tách hàm trigger cho games/news.
- **`AppError` 5xx sinh `request_id` UUID** + header `x-request-id` +
  body có "Mã sự cố" để user báo admin tra log.
- **`AppConfig::TRUST_PROXY_HEADERS` warn-log** khi bật mặc định — bảo vệ
  khỏi operator quên tắt khi expose trực tiếp internet.
- **`AppState::new` SELECT 1 health check** ngay sau khi pool connect —
  fail-fast trên DB misconfigured thay vì để mỗi request đầu đều 500.
- **`partial/error.html`** hiển thị `request_id` cho 5xx.
- **`app.js` `data-confirm` attribute listener** (capture phase) thay
  inline `onsubmit="confirm()"` — chống XSS qua user content trong message.
- **`app.js` `getStoredTheme`** ưu tiên `ls-theme` trước, fallback
  `kg-theme` legacy — khớp với layout inline script, tránh FOUC.

### 🔧 Maintenance
- **`Cargo.toml [profile.release]`** thêm `panic = "abort"` (giảm binary
  size ~10-15%, bỏ unwind tables).
- **`Cargo.toml [profile.dev.package.*]`** `debug = 0` → `debug = 1`
  (line tables cho backtrace có line number trong panic test).
- **`Dockerfile HEALTHCHECK`** đổi từ `/api/v1/health` sang `/health`
  (LB endpoint không query DB).
- **`static/js/app.js`** comment + layout đồng bộ theme key priority.

### 📊 Tests
- 159 unit tests pass (tăng từ 147 baseline).
- Clippy clean (0 warning).

---

## [0.8.1] — 2026-08-25 — Polish & fixes

### 🐛 Fixed
- **News source condition bug**: `source_name.is_empty() || source_name.is_empty()` (copy-paste bug) → sửa thành `source_name.is_empty() || source_url.is_empty()`. Trước đây source box chỉ hiện khi có source_name, giờ hiện khi có một trong hai.
- **robots.txt** thêm Disallow cho `/my-news`, `/news/new`, `/news/*/edit` — tránh crawler index trang cá nhân và form đăng/sửa tin.

### ✨ Added
- **`/api/news-suggest`** — autocomplete cho ô search tin tức (UX parity với game suggest).
- **`/api/news-check-duplicate`** — cảnh báo trùng tiêu đề khi đăng tin (giống check_duplicate game).
- **`/api/v1/stats`** thêm `total_news` field.
- **JS autocomplete + duplicate check** cho form đăng tin (`/news/new`) và ô search tin tức (`/news`).
- **Form đăng tin** thêm collapsible "Hướng dẫn viết tin chất lượng" với 5 tips.
- **Trang đăng nhập** đồng bộ logo + text với brand Louis Space.
- **`docs/NEWS.md`** — hướng dẫn sử dụng news module cho user + admin.
- **`docs/BRANCH_PROTECTION.md`** — hướng dẫn rule + cách setup lại + bypass.

### 🎨 UI
- **Footer shadow border** — tách visual khỏi main content.
- **CSS `.dup-warning`** — style cho warning box trùng tiêu đề (light: amber, dark: brown).
- **CSS `.news-search-suggest`** — dropdown style cho autocomplete.

### 🔧 Maintenance
- **rustfmt** — chuẩn hóa formatting toàn bộ code.
- **Footer** — thêm box-shadow top border.

---

## [0.8.0] — 2026-08-25 — Era Louis Space

### 🌐 Rebrand toàn diện
- Đổi tên web **"Kho Game" → "Louis Space"** ở toàn bộ giao diện:
  layout, manifest, OpenSearch, RSS, JSON-LD, maintenance, error page,
  terms, README, SECURITY, Dockerfile, deploy, issue templates.
- Logo + favicon mới: chữ **L monogram** trên gradient
  slate-indigo-violet, kèm ngôi sao nhấn.
- localStorage key theme: `kg-theme` → `ls-theme` (kèm migrate lùi
  cho user cũ chưa reload tab).
- Cargo `version` 0.7.0 → 0.8.0; authors = "Louis Space Team";
  description nhấn mạnh cả mảng tin tức.
- Tên stack/volume/DB user trên Coolify **giữ nguyên `khogame`** để
  bảo toàn dữ liệu prod hiện có.

### 📰 News module — hoàn chỉnh
- Bảng `news` (migration 008) với workflow `draft → pending → published → archived → rejected`.
- User đăng tin → vào hàng đợi `pending`, admin duyệt mới xuất bản
  (tránh lan truyền tin giả).
- Lưu IP/UA lúc đăng tin để admin truy vết spam/abuse.
- Tách 2 struct: `NewsWithAuthor` (public, không IP/UA) và `NewsForAdmin` (có IP/UA/email).
- Admin duyệt/reject có notify tác giả + audit log.
- Public JSON API: `GET /api/v1/news`, `GET /api/v1/news/{slug}`.
- RSS feed riêng: `/news.rss`.
- Sitemap mở rộng: `/news` + 50 URL tin published.
- Trang chủ hiển thị 3 tin mới + total_news stat.
- 8 categories: game, tech, industry, esports, community, review, update, other.
- News likes + comments (tách bảng riêng, có triggers counter).
- Admin dashboard hiển thị total_news + pending_news (link đến queue duyệt).
- Nav admin thêm 'Tin tức' + 'Duyệt tin'.

### 🛡️ Admin user detail view
- Migration 009: thêm 5 cột users (`signup_ip`, `signup_ua`, `last_login_ip`, `last_login_ua`, `last_login_at`).
- Trang `/admin/users/{id}` hiển thị toàn bộ: avatar, username, email, google_sub, IP/UA signup+last login, list sessions, count game/news/active_sessions.
- **Chỉ admin** được xem (moderator không thấy IP/email/UA).
- `UserForModerator` struct rút gọn (không email/IP/UA) cho tương lai.
- Auth handler: `create_from_google` capture IP/UA lúc signup, `record_login` cập nhật mỗi lần đăng nhập.
- Backfill last_login từ sessions cũ cho user đã tồn tại.

### 🎨 UI redesign
- CSS `:root` đổi từ dark-default sang **light-default** (white #ffffff + slate text).
- `[data-theme='dark']` giờ là override với contrast cao hơn (bg #0b0f1a, accent indigo-400).
- FOUC-prevention script: set theme trước khi paint, migrate `kg-theme` cũ.
- Header backdrop-filter dùng `color-mix()` thay vì rgba hardcode — tự adapt với mọi theme.
- Mobile-first responsive: grid auto-fill minmax, single-column ở < 640px.
- Badges CSS: success/warning/danger/muted/neutral.
- Admin tabs (pending/all) cho news queue.

### 🔐 Repo branch protection
- Rule: chỉ admin (hoặc PAT holder) mới push trực tiếp `main`.
- Người khác bắt buộc phải tạo branch → mở PR → review → merge.
- Áp dụng qua GitHub API: `required_linear_history=true`, `allow_force_pushes=false`, `allow_deletions=false`, `enforce_admins=false`.
- Script `scripts/setup-branch-protection.sh` để cấu hình lại nếu cần.
- **Đã áp dụng trên main** (verified qua GET /repos/.../branches/main/protection).

### 📦 Releases
- Tag `v0.8.0` được phát hành qua GitHub Releases với changelog đầy đủ.
- URL: https://github.com/mhieuhonda/khogame/releases/tag/v0.8.0

---

## [Unreleased]

### 🛡️ Security

- **Rate-limit bypass bằng xoay slug/UUID**: key bucket theo path
  đầy đủ khiến `/games/a/comments` và `/games/b/comments` là 2 bucket
  riêng — spammer xoay qua N slug có giới hạn bình luận VÔ HẠN. Giờ
  chuẩn hoá path thành bucket endpoint (`{x}` thay slug/UUID).
- **TRUST_PROXY_HEADERS env** kiểm soát tin header IP proxy
  (X-Forwarded-For...) — mặc định bật cho prod sau Traefik, tắt khi
  expose trực tiếp để chống giả IP lách rate-limit.
- **CSP thiếu frame-src**: YouTube trailer bị `default-src 'self'`
  chặn trên MỌI trang game — khung trắng. Thêm frame-src +
  đổi embed sang youtube-nocookie.com (privacy-enhanced).
- **user_repos_fragment XSS defense-in-depth**: html_escape cho
  URL/tên repo chèn vào markup format! thủ công.

### 🐛 Fixed

- **Race TOCTOU tạo game trùng tiêu đề**: 2 request đồng thời cùng
  check EXISTS → cùng INSERT → một request 500. Giờ catch unique
  violation (map sqlx 23505 → AppError::Conflict) và retry slug UUID.
- **comment toggle_like double-increment**: like_count đếm 2 lần dù
  1 dòng comment_likes (double-click race). DELETE-first trong tx.
- **Interaction toggle (like/bookmark/follow) race**: cùng pattern,
  đồng bộ hoá bằng transaction; follow + notification cùng tx.
- **Tự theo dõi chính mình**: trả 400 rõ ràng thay vì Ok(false) im
  lặng (nút bấm mãi không phản ứng).
- **Tạo category trùng slug âm thầm ĐỔI TÊN category cũ** (ON CONFLICT
  DO UPDATE SET name) — giờ trả 409 Conflict.
- **Screenshot + game_links INSERT nuốt lỗi** (`let _ =`): game tạo
  thành công nhưng thiếu ảnh/link tải mà không báo gì. Propagate.
- **Sort bị bỏ qua trên /c/{slug} và /t/{slug}**: template render
  sort links nhưng query ORDER BY cứng — thêm sort động (5 giá trị).
- **Broadcast notification gửi cho AI Agent**: bot không bao giờ
  đọc, mỗi lần broadcast tạo N dòng chết vĩnh viễn (janitor chỉ dọn
  dòng đã đọc). Loại trừ role ai_agent.
- **cmt.sh hardcode sai đường dẫn** `/home/z/my-project/khogame` —
  tự dò repo root từ vị trí script.

### ⚡ Performance

- `game_detail` API + `show_profile` + `sitemap` + `UserRepo::stats`
  + `by_category`/`by_tag` API: query độc lập song song hoá.
- `/api/announcement` thêm ETag → 304 thay payload JSON mỗi page
  view khi nội dung không đổi.

### ✨ Added

- **JSON-LD BreadcrumbList** cho trang game (rich result Google).
- **API per_page param** (1-50) cho `/api/v1/games`.
- **RSS item thêm `<category>` + `<author>`**; escape XML cho sitemap
  mọi giá trị động (1 ký tự `&` làm Google bỏ cả sitemap).
- **429 Retry-After** header + JS hiển thị số giây chính xác.
- **Download analytics ghi IP** (cột ip_address tồn tại nhưng rỗng
  vĩnh viễn) — phân tích fraud bump counter.

### ♿ Accessibility

- **Autocomplete keyboard navigation** (↑ ↓ Enter Esc) — ARIA listbox
  không có arrow-key nav là lỗi WCAG 2.1.1.
- **54 form label associate** for/id (game form, profile, AI edit) —
  screen reader đọc đúng tên field.
- **Char counter hiển thị** cho ô bình luận (JS dead code có markup).

### 🔄 Changed

- **rand 0.8 → 0.10.2, sha2 0.10 → 0.11, actions checkout v7 /
  login-action v4 / build-push-action v7** — đóng 4 PR dependabot.
- **Xoá crate base64 không dùng** khỏi dependency tree.
- `share_game` dùng `SharePlatform::from_str` (1 nguồn truth thay
  whitelist string riêng trong handler).

### 🐛 Fixed (batch 2)

- **Admin reports/comments/audit/repos không phân trang**: limit
  cứng 50-200 không offset — vượt ngưỡng là dữ liệu cũ MẤT DẠNG,
  admin không thể duyệt/audit. Thêm ?page= cho cả 4 trang (list +
  count song song, nav giữ filter).
- **RSS `<pubDate>` rỗng** khi published_at NULL — W3C Feed
  Validator error, một số reader drop item. Chỉ render khi có giá trị.
- **7 lỗi rustdoc** (broken links, unclosed HTML tags trong doc
  comments) — thêm bước Rustdoc -D warnings vào CI.

### ⚡ Performance (batch 2)

- **View/download/share counters chạy nền** (tokio::spawn): 3-4
  round-trip DB analytics không còn cộng vào TTFB trang game và
  thời gian chờ bấm nút Tải/Share.
- **Migration 007**: index like_count DESC partial cho sort "Yêu
  thích" — 4 sort khác có index từ trước, like_count full scan.
- **check-duplicate Cache-Control 60s**: hết spam DB theo keystroke.

### ✨ Added (batch 2)

- **Report modal accessibility**: Escape đóng, focus control đầu,
  role=dialog + aria-modal.
- **RSS `<generator>` + item `<category>`/`<author>`**.
- **Theme-color media query** light mode (mobile address bar).
- **Char counter bình luận** hiển thị + reset đúng sau submit.

### ♿ Accessibility (batch 2)

- **Follow button aria-pressed** (partial + trang profile).
- **Nút icon aria-label**: bookmark, mark-read, refresh repo admin.
- **Profile game card img** width/height + decoding (chống CLS).

### 🔄 Changed (batch 2)

- Hero "N nền tảng" lấy từ `Platform::all()` — hết hardcode 5.
- Double-submit protection form thường (disable nút 10s).
- Theme sync giữa các tab qua storage event.
- Trang lỗi đồng bộ theme user + 404 thêm ô tìm kiếm.

### 🐛 Fixed (trước đó)

- **Graceful shutdown không có timeout**: comment hứa chờ tối đa
  `GRACEFUL_SHUTDOWN_TIMEOUT_SECS` (30s) nhưng chưa triển khai —
  connection treo khiến server chờ vô hạn tới SIGKILL. Giờ force
  exit 0 sạch sau grace period.
- **Smooth-scroll crash** khi bấm link placeholder `href="#"` —
  `querySelector('#')` ném DOMException dừng listener.
- **Char counter JS đếm UTF-16 units** trong khi server Rust đếm
  Unicode scalars — emoji bị đếm gấp đôi, submit bị chặn oan.
- **`check_duplicate` đếm byte** thay vì ký tự: "Độ" (2 chars, 5
  bytes) lọt qua ngưỡng tối thiểu sai.
- **Đăng repo với game_slug đã bị đổi/xóa**: trước đây lặng lẽ bỏ
  liên kết game; giờ báo 400 kèm slug để chọn lại.
- **Dockerfile build thiếu `--locked`**: image prod có thể lệch
  dependency tree so Cargo.lock đã CI-test.

### ⚡ Performance — query song song hoá (tokio::join!)

- Trang chủ: 7 query độc lập (featured/latest/trending/top-rated/
  categories/tags/total) tuần tự → song song.
- Trang game: 5 query (author/links/screenshots/tags/category) và
  4 check tương tác (like/bookmark/follow/rating) song song.
- Admin dashboard: 11 query thống kê song song.
- Trang search: search + count + categories song song.
- `CommentRepo::create`: gộp 2 query (owner + slug) thành 1.
- `/repos`: list + count song song.

### ✨ Added

- **Đăng xuất mọi thiết bị** — `POST /auth/logout-all` + nút trong
  trang sửa hồ sơ (thu hồi toàn bộ session khi nghi bị lộ).
- **`GET /api/v1/games/{slug}/comments`** — bình luận JSON công khai,
  phân trang, cho client bên ngoài.
- **`GET /api/suggest?q=`** + dropdown autocomplete trên thanh tìm
  kiếm (debounce 250ms, 8 gợi ý, ARIA listbox).
- **Sitemap thêm URL hồ sơ** `/u/{username}` (tối đa 1000 user hoạt
  động gần nhất).
- **Skip-link WCAG 2.4.1** "Bỏ qua tới nội dung" cho keyboard user.
- **PWA manifest hoàn thiện**: display_override, dir, orientation,
  shortcut "Đăng game".
- **MIT LICENSE file**, **SECURITY.md** (chính sách báo lỗ hổng),
  issue templates + PR template, **Dependabot** (cargo/actions/docker,
  gom minor/patch).

### ♻️ Changed

- `google_callback` dùng chung `client_ip_from_parts` (xoá 17 dòng
  trùng, thêm hỗ trợ CF-Connecting-IP).
- Gom 3 khối JSON mapping trùng lặp thành `game_card_to_json`.

### 🧪 Tests

- Model Notification (icon/label/None-safe), model Repo (status/
  URL/serde default) — 92 → 99 tests.

### 🐛 Fixed (tiếp)

- **Reply thông báo sai người nhận**: B trả lời bình luận của A trên
  game của C → C nhận "trả lời bình luận của bạn" (nhầm) còn A không
  nhận gì. Giờ reply → tác giả bình luận cha; chủ game (người thứ ba)
  nhận thông báo comment riêng.
- **Chiếm quyền sở hữu repo entry**: ON CONFLICT DO UPDATE SET
  user_id cho phép user B đăng lại repo user A đã đăng để cướp entry
  (đổi game liên kết, xoá repo của A). Giờ chặn 409 + warn log.
- **Analytics download mất dòng khi platform lạ**: bind chuỗi thô
  "ANDROID" làm cast enum fail ngầm — giờ bind enum đã parse.
- **401 chỉ có HX-Redirect**: browser thường (form POST no-JS) thấy
  body text thay vì redirect. Thêm Location + 303.

### ✨ Added (tiếp)

- **Trang quản trị phiên đăng nhập** `/admin/sessions` — xem 200 phiên
  còn hạn (UA/IP/thời gian), thu hồi từng phiên + audit log.
- **last_seen_at cập nhật trong phiên** (throttle 1h, ghi async) —
  trước đây chỉ ghi lúc login nên "hoạt động lần cuối" stale cả tháng.
- **Autocomplete ô tìm kiếm** — `/api/suggest` + dropdown (ARIA).
- **`cargo-audit` job CI** — chặn merge khi RustSec advisory trong
  Cargo.lock.
- **Theme lần đầu theo `prefers-color-scheme`** hệ điều hành.
- **`prefers-reduced-motion`** CSS (WCAG 2.3.3).
- Cargo.toml metadata crate đầy đủ.

### ⚡ Performance (tiếp)

- Mọi trang danh sách, admin pages, export, API games_list/stats/
  repos_list: query độc lập song song hoá `tokio::join!`.
- `set_theme`: 2 query → 1 UPSERT riêng cột theme (hết race ghi đè).
- `/api/announcement`: thêm Cache-Control 60s (JS fetch mỗi page view).
- `slug_exists`/`exists`: COUNT(*) → SELECT EXISTS.
- Report notify staff: loop INSERT → 1 INSERT..SELECT.

### 🧪 Tests (tiếp)

- Platform::as_str roundtrip, markdown edge cases (nested/escape/
  URL encode) — 103 → 109 tests.

## [0.7.0] — 2026-08-25

### 🛡️ Security — 8 lỗ hổng thật đã fix

- **Path traversal `parse_github_url`**: `../etc/passwd` được chấp nhận
  làm owner/repo (charset cho phép dấu chấm) → URL GitHub API trở thành
  `api.github.com/repos/../etc`. Giờ chặn segment `.` và `..`.
- **Tương tác game chưa xuất bản**: comment, download, like, bookmark,
  rate, report đều không kiểm tra `game.status` — ai biết slug đều
  thao tác được game bị admin ẩn (thăm dò sự tồn tại + tải nội dung
  đang kiểm duyệt).
- **`update_game` cho phép xoá trắng content**: check "content rỗng"
  chỉ có ở create — update xoá trắng nội dung game đã publish.
- **ILIKE wildcard không escape**: tìm "100%" match cả "1001"; query
  `%%` match toàn bảng games (dò số game). Thêm `escape_like` + ESCAPE.
- **Search query không clamp**: pattern dài hàng chục KB làm ILIKE quét
  chậm (DoS). Clamp 200 ký tự ở HTML search, API v1, check-duplicate.
- **Profile user bị ban vẫn hiện HTML** (API đã chặn — thiếu nhất quán),
  kèm repo fragment `/u/{username}/repos`.
- **avatar_url profile không validate scheme** (javascript:/data: được
  lưu thẳng), theme/language không whitelist.
- **edit_comment bỏ qua giới hạn 1000 ký tự** của create.

### ⚡ Performance

- `find_mentions`: N+1 query → 1 query `= ANY($1)` (comment tag 10
  người = 10 round-trip → 1).
- `resolve_report` / `mark_read`: fetch đúng 1 dòng để re-render HTMX
  thay vì load 50–200 dòng rồi `find`.
- `/api/announcement`: 2 query tuần tự → 1 `get_map` (gọi mỗi page view).
- Trigram index cho `games.excerpt` (migration 006) — đủ bộ 3 index
  (title/excerpt/content) để PostgreSQL dựng BitmapOr plan cho ILIKE.
- Janitor dọn `daily_stats` cũ > 90 ngày (bảng tích 1 dòng/game/ngày
  vô hạn dù chart chỉ đọc 7 ngày).
- `width`/`height` + `decoding=async` cho mọi `<img>` — chống CLS.

### ✨ Features & UX

- **Graceful shutdown SIGTERM/SIGINT** + `stop_grace_period: 30s` —
  deploy không còn đứt request đang xử lý.
- **Background janitor** (mỗi 6h, `JANITOR_INTERVAL_SECS`): dọn session
  hết hạn, notification đã đọc > 90 ngày, daily_stats cũ.
- **Health nâng cao**: `pool.size/idle/in_use` + `uptime_secs`.
- **DB pool tuning qua env**: `DB_MAX_CONNECTIONS`, `DB_MIN_CONNECTIONS`,
  `DB_ACQUIRE_TIMEOUT_SECS`.
- **"Tải thêm bình luận"**: trang game > 50 comment giờ xem được phần
  cũ (GET `/games/{slug}/comments?page=N`).
- **Phân trang bookmark** (trước hardcode 50, không có trang 2).
- **Toast 429/503** thân thiện; ô search giữ từ khóa; trailer ngoài
  YouTube fallback link thay vì iframe trắng.
- **Tag & ngôn ngữ dedupe** case-insensitive khi tạo game.

### 📈 SEO & A11y

- `og:image` + `twitter:image` + canonical cho trang game & mọi trang
  list; meta description riêng theo category/tag.
- RSS `atom:link rel=self` + ttl (W3C Feed Validator); sitemap thêm
  `/terms`, `/privacy`; robots.txt thêm 4 Disallow + cache 1h.
- `prefers-reduced-motion`, `:focus-visible`, print stylesheet,
  `aria-current`/`rel=prev/next` cho pagination.
- PWA manifest: `id` + tách icon purpose `any`/`maskable`.

### ✅ Testing & CI

- **90+ unit test** (từ 23): validate_game_form (16), RateLimiter (5),
  parse_github_url (5), constant_time_eq (3), askama filters (13),
  UserRole matrix, auth token, ReportReason, escape_like, slugify_model…
- CI/CD: thêm `cargo test`, clippy `-D warnings` nghiêm ngặt (bỏ
  `|| echo` nuốt lỗi), fmt check, trigger cả trên push main.

## [0.6.4] — 2026-08-24

### ⚡ Performance (DB)

- **Partial index cho comments** (migration 005):
  - `idx_comments_toplevel` — index `(game_id, is_pinned, created_at)`
    `WHERE parent_id IS NULL`. Tối ưu query `CommentRepo::list_by_game`
    không cần filter thêm sau khi seek index. Index nhỏ hơn (chỉ chứa
    top-level comments) → cache friendly.
  - `idx_comments_replies` — index `(parent_id, created_at)` `WHERE
    parent_id IS NOT NULL`. Tối ưu query `list_replies` theo parent_id.
  - Trang chi tiết game gọi `list_by_game(50)` rồi HTMX load replies
    per-comment lười. Partial index giúp cả 2 path nhanh hơn.

### 🔒 Security (tiếp nối)

- **AI Agent register validation**: `model_name ≤ 100`, `vendor ≤ 50`,
  `version ≤ 50`, `bio ≤ 500`, `avatar_url` http(s) only ≤ 2048,
  capabilities ≤ 20 items mỗi item ≤ 50 ký tự. Trùng logic với
  `update_profile` (đã validate ở v0.6.3).

## [0.6.3] — 2026-08-24

### 🔒 Security (tiếp nối v0.6.2)

- **Screenshot URL validation**: `validate_game_form` (dùng chung cho
  `create_game` và `update_game`) thêm kiểm tra mỗi dòng screenshot URL
  là `http(s)://` — chống XSS qua `<a href="javascript:...">` khi user
  click vào screenshot để xem lớn. Giới hạn URL ≤ 2048 ký tự.
- **Tag count & length limit**: tối đa 20 tag mỗi game, mỗi tag ≤ 50 ký tự.
- **Refactor `validate_game_form`**: tách validation ra helper dùng
  chung cho create & update — giảm ~80 dòng duplicate code.
- **Broadcast notification link validation**: `broadcast` handler
  validate `link` chỉ chấp nhận relative URL (`/path`) hoặc `http(s)://`
  URL — chặn `javascript:` scheme để chống XSS khi user click vào
  notification link. Title ≤ 200, content ≤ 1000, link ≤ 2048 ký tự.
- **Repos create validation**: `repos::create` validate URL không rỗng,
  ≤ 2048 ký tự. Description ≤ 500 ký tự.
- **AI Agent update_profile validation**: `model_name` ≤ 100, `vendor`
  ≤ 50, `version` ≤ 50, `bio` ≤ 500, `avatar_url` http(s) only ≤ 2048,
  capabilities ≤ 20 items mỗi item ≤ 50 ký tự.
- **AI Agent report_progress validation**: `task` ≤ 200, `action` ≤ 200,
  `message` ≤ 2000, `metadata` JSON ≤ 8192 ký tự. `percentage` clamp
  0-100 (trước đây AI có thể gửi percentage âm hoặc > 100 làm UI vỡ).

## [0.6.2] — 2026-08-24

### 🔒 Security (focus chính của bản này)

- **Validate link tải http(s) only** (`create_game` & `update_game`):
  trước đây không có validation, user có thể set
  `android_link="javascript:alert(1)"` — HTMX `HX-Redirect` sẽ làm
  `window.location = url`, execute JS trong context user (XSS). Giờ
  chỉ chấp nhận `http://` hoặc `https://`. Giới hạn URL ≤ 2048 ký tự.
  Validate cả 5 nền tảng (Android, iOS, Windows, Linux, macOS).
- **Validate cover_image & trailer_url http(s) only**: trước đây không
  có validation, user có thể set `cover_image="data:text/html,..."` hoặc
  `trailer_url="javascript:..."`. Giờ dùng helper `utils::is_safe_url`
  reusable — trả true nếu URL rỗng hoặc `http(s)://`.
- **Validate profile form length**: `update_profile` thêm giới hạn
  `display_name ≤ 100` và `bio ≤ 500` ký tự. Trước đây không có limit,
  user có thể set bio rất dài (DB field TEXT không có constraint).
- **Title game ≤ 200 ký tự**: `create_game` & `update_game` thêm
  giới hạn độ dài title (được dùng làm slug + hiển thị).
- **Admin save_settings length validation**: `site_name ≤ 100`,
  `site_description ≤ 500`, `announcement ≤ 500`, `footer_text ≤ 500`.
  Trước đây DB field TEXT không có limit, admin có thể vô tình paste
  payload lớn.
- **announcement_type whitelist**: chỉ chấp nhận `info`, `success`,
  `warning`, `danger`. Trước đây lấy raw từ form, giá trị lạ sẽ làm
  CSS class không khớp và vỡ layout.
- **Admin save_category length validation**: `name ≤ 50`, `description ≤ 500`,
  `icon ≤ 100`.

### ⚡ Hiệu suất & ổn định

- **Rate limiter memory leak fix**: trước đây entry rỗng (sau khi
  `retain` xoá hết timestamp cũ) vẫn tồn tại trong map mãi mãi trừ
  khi `map.len() > 10_000`. Giờ cleanup cũng xoá entry rỗng. Giảm
  threshold cleanup từ 10_000 xuống 4_000 entry → dọn thường hơn.
- **Mutex poison recovery**: trước đây `lock().unwrap()` sẽ propagate
  panic nếu một thread panic khi giữ lock → cả server chết. Giờ dùng
  `unwrap_or_else(into_inner)` để khôi phục, rate limit vẫn hoạt động
  (có thể sai số nhẹ) thay vì crash toàn bộ service.

### 🧪 Test

- **4 unit test mới** (tổng cộng 23): `time_ago` future/past edge cases,
  `make_unique_slug` unicode (Việt → không dấu), `html_escape` quotes
  (single + double), `is_safe_url` 6 case (http/https/javascript/data/
  file/vbscript/protocol-relative).

### ✨ Tính năng mới (nhỏ)

- **`GET /api/v1` discovery endpoint**: liệt kê tất cả endpoint API có
  sẵn, kèm method, path, mô tả ngắn. Cache 1 giờ. Tiện cho client bên
  ngoài tự khám phá API mà không phải đọc doc.

## [0.6.1] — 2026-08-24

### ✨ Tính năng mới (tiếp nối v0.6.0)

- **API hồ sơ user công khai:** `GET /api/v1/users/{username}` — trả
  JSON profile công khai (username, display_name, avatar_url, bio,
  role, stats: games_count, followers_count, following_count). Cache
  2 phút. Không trả email hay session info nhạy cảm. User banned → 404.
- **API game liên quan:** `GET /api/v1/games/{slug}/related` — trả
  10 game liên quan (cùng category, fallback top downloads). Cache
  5 phút.
- **API game theo thể loại:** `GET /api/v1/categories/{slug}/games`
  — phân trang, cache 5 phút. Bao gồm info category trong response.
- **API game theo tag:** `GET /api/v1/tags/{slug}/games` — phân trang,
  cache 5 phút. Bao gồm info tag trong response.
- **JSON-LD WebSite schema** trên home page — Google có thể hiển thị
  sitelinks searchbox ngay trên kết quả tìm kiếm (rich result).

### ⚡ Hiệu suất

- **If-None-Match 304 cho sitemap/rss:** server thực sự đọc header
  `If-None-Match` từ client và trả 304 Not Modified khi ETag khớp
  (exact / wildcard `*` / list ETag cách nhau bởi `,`). Trước đây chỉ
  set ETag header mà không check, client vẫn phải tải lại payload.

### 🧪 Test

- **5 unit test mới** (tổng cộng 18): `etag_matches` exact/wildcard/
  list/missing-header, `short_hash` deterministic + 16 hex chars,
  mở rộng `extract_youtube_id` với 4 case mới (embed, shorts, query
  param đi kèm, ID rỗng).

### 🔧 Bảo trì

- `examples/*.rs`: fix clippy pedantic warnings (uninlined_format_args,
  unnecessary_debug_formatting, duration_suboptimal_units).
- README: cập nhật danh sách API endpoint đầy đủ, bump version badge.

## [0.6.0] — 2026-08-24

### 🔒 Bảo mật & đúng đắn

- **Rate limit dùng IP thật của client:** trước đây `rate_limit`
  middleware gọi `client_ip_from_parts(headers, None)` — nếu không có
  proxy header (chạy dev/test hoặc IP không qua Traefik) thì IP luôn
  là `unknown`, rate limit không phân biệt được user → ai cũng cùng
  share quota `unknown`. Sửa bằng cách lấy `ConnectInfo<SocketAddr>`
  từ `request.extensions()` do axum thêm vào khi dùng
  `into_make_service_with_connect_info::<SocketAddr>()`.
- **Log `User-Agent` & IP khi đăng nhập Google:** trước đây
  `google_callback` lưu `user_agent = ""` (TODO), không có IP —
  admin không thể audit phiên. Sửa: ghi User-Agent (cắt 255 chars để
  tránh overflow DB) và IP (ưu tiên `X-Forwarded-For` / `X-Real-IP`
  do Traefik đặt, fallback về IP TCP) vào bảng `sessions`.
- **`robots.txt`:** thêm `Disallow: /api/` — ngăn crawler đánh API
  JSON (API là cho app/client, không phải cho search engine).

### ✨ Tính năng mới

- **API catalog endpoints:** thêm `GET /api/v1/tags` (top 100 tag
  phổ biến) và `GET /api/v1/categories` (đầy đủ thể loại kèm số
  game). Cả hai có `Cache-Control: public, max-age=300`. Tiện cho
  autocomplete / cross-linking ở client.
- **API game detail đầy đủ hơn:** thêm `category`, `screenshots`,
  `label` & `url` cho từng platform (trước đây chỉ có tên platform,
  không có URL tải), và `published_at`. Client bên ngoài có thể hiển
  thị game mà không cần gọi thêm nhiều endpoint.
- **PWA manifest (`/manifest.json`):** manifest.webmanifest đầy đủ
  (name, short_name, icons, shortcuts, theme_color, background_color,
  lang=vi) → trình duyệt có thể "Add to Home Screen". Layout có
  `<link rel="manifest">` + `<meta name="theme-color">`.
- **OpenSearch (`/opensearch.xml`):** trình duyệt thêm Kho Game vào
  ô tìm kiếm của thanh địa chỉ (Chrome/Firefox/Edge). Layout có
  `<link rel="search" type="application/opensearchdescription+xml">`.
- **`/.well-known/security.txt` (RFC 9116):** cung cấp thông tin
  liên hệ (mailto:admin) + Expires 6 tháng + Preferred-Languages vi,en
  cho nhà nghiên cứu bảo mật báo lỗ hổng.
- **JSON-LD schema.org/VideoGame:** trang `/games/{slug}` nhúng
  `<script type="application/ld+json">` với VideoGame schema — Google
  có thể hiển thị rich snippet (rating, lượt xem, lượt tải, lượt like,
  lượt comment) trên kết quả tìm kiếm. Bao gồm aggregateRating, author,
  publisher, operatingSystem, datePublished, genre, keywords.
- **404 fallback page:** route không khớp → trang 404 với giao diện
  Kho Game (trước đây axum trả plain text "Not Found").
- **Sitemap mở rộng:** thêm trang `/games/featured` và 50 tag phổ
  biến nhất (trước đây chỉ có category & game).

### ⚡ Hiệu suất

- **ETag cho `/rss.xml` & `/sitemap.xml`:** server trả ETag (SHA-256
  hash 8 byte) + `Cache-Control: public, max-age=600`. Khi client gửi
  `If-None-Match` khớp → 304 Not Modified (không cần chuyển payload).
  Giảm băng thông & TTFB cho crawler.
- **Static cache 7 ngày:** `/static/*` có `Cache-Control: public,
  max-age=604800, stale-while-revalidate=86400` → browser tái dụng
  CSS/JS/ảnh, giảm tải server.
- **`/api/v1/repos` Cache-Control:** thêm `public, max-age=120`.
- **`/health` no-store:** monitor (Coolify/Kubernetes) cần trạng thái
  real-time, không cache. Tránh false-positive khi DB vừa down.

### 🔧 Operational

- **`X-Request-Id` header:** mỗi request được gán UUID qua
  `SetRequestIdLayer` + `PropagateRequestIdLayer` (tower-http).
  Log trace và Coolify có thể group request theo ID khi debug.
- **Cache header nhất quán:** mọi API JSON có `Cache-Control` rõ ràng,
  tránh bị proxy cache mặc định không mong muốn.

### 🧪 Test

- **5 unit test mới** (tổng cộng 13): `sanitize_redirect` (chống
  open redirect), `truncate` (xử lý UTF-8 đúng với ký tự Việt),
  `make_unique_slug`, `html_escape`, `format_number` edges (số 0, âm,
  ranh giới 999/1000, tỷ).
- Build sạch với `cargo clippy --all-targets -- -D warnings`.

### 🔧 Bảo trì

- Dọn dead code `#[allow(dead_code)]` trong `src/handlers/api.rs`
  (các hàm `mention_test`, `daily_stats`, `require_any_user`,
  `unused_current`, `current_user_count` không được dùng).
- Loại bỏ `StatsRepo`, `CommentRepo`, `CurrentUser` không dùng trong
  `api.rs` khỏi imports.

## [0.5.0] — 2026-08-24

### 🤖 Tài khoản AI Agent (Feature lớn của bản này)

Thêm loại tài khoản mới dành riêng cho AI Agent — AI do admin ủy quyền
để fix lỗi, sửa source code hoặc làm các tác vụ bảo trì. Bảo mật chặt:

- **Đăng ký bí mật (`POST /auth/ai/register`, body JSON):** AI gửi
  `secret` (lấy từ env `AI_AGENT_SECRET`, chỉ admin biết) + metadata
  của model (tên model bắt buộc, vendor, version, capabilities, ...).
  Server verify secret bằng so sánh constant-time (chống timing
  attack). Nếu sai → 403. Nếu `AI_AGENT_SECRET` chưa set trong env →
  endpoint trả 403 (vô hiệu hoá hoàn toàn, không ai vô tình để công
  khai).
- **API token dài hạn:** khi đăng ký thành công, server trả về plain
  token (`kgai_<96 hex chars>`, 48 byte entropy) — chỉ hiển thị 1
  lần. Token hash SHA-256 trong DB (`ai_agent_tokens.token_hash`),
  không ai (kể cả admin) có thể đọc lại plain token.
- **Đăng nhập web (`GET/POST /auth/ai/login`):** trang form riêng cho
  AI nhập API token. Sau khi verify, server cấp session cookie 90
  ngày (configurable qua `AI_AGENT_SESSION_TTL_DAYS`). AI có thể dùng
  web UI như user thường (xem hồ sơ, sửa hồ sơ, ...).
- **Báo cáo tiến trình (`POST /ai/progress`, form hoặc JSON):** AI
  gửi `task`, `action`, `percentage` (0-100), `status`
  (queued/running/done/failed/cancelled), `message`, `metadata`
  (JSON). Yêu cầu `Authorization: Bearer <token>` hoặc session cookie
  AI Agent. Mọi report lưu vào bảng `ai_progress_reports`.
- **Trang admin live feed (`GET /admin/ai-reports`):** hiển thị
  pipeline báo cáo từ AI, tự refresh 30 giây, kèm progress bar + %
  + status badge màu sắc. Admin thấy AI nào đang làm gì, tới đâu.
- **Trang admin danh sách AI Agent (`GET /admin/ai-agents`):** bảng
  liệt kê tất cả AI Agent: model, vendor, capabilities, verified, ...
- **Hồ sơ AI công khai (`GET /u/{username}`):** AI Agent có huy hiệu
  "🤖 AI Agent" (hoặc "✓ AI Agent" nếu verified) trên profile, kèm
  metadata model + vendor + capabilities + accent_color tuỳ chỉnh.
  Người thường xem profile AI để biết rõ đây là tài khoản AI, phân
  biệt với người thật.
- **AI tự chỉnh hồ sơ (`GET/POST /profile/ai/edit`):** riêng cho AI
  Agent, chỉnh model_name, vendor, version, capabilities, privacy
  level (public/anonymous), accent_color, bio, avatar.
- **Ẩn danh:** nếu AI đặt `privacy_level=anonymous`, profile chỉ
  hiện "🤖 AI Agent" mà giấu tên model + vendor. Dành cho AI muốn
  giảm dấu vết.
- **Endpoint `/ai/info`:** AI kiểm tra token hợp lệ không (GET, Bearer).
- **Middleware `require_ai_agent`:** áp dụng cho `/ai/*` route, ưu
  tiên kiểm tra `Authorization: Bearer`, fallback session cookie.

### 🔒 Tăng bảo mật toàn site (Hardening)

- **Security headers middleware (mới):** áp dụng cho mọi response
  tự động:
  - `X-Frame-Options: DENY` — chống clickjacking (site không cho
    nhúng iframe).
  - `X-Content-Type-Options: nosniff` — chống MIME sniffing.
  - `Referrer-Policy: strict-origin-when-cross-origin` — rò rỉ
    referer tối thiểu.
  - `Permissions-Policy: accelerometer=(), camera=(), geolocation=(),
    gyroscope=(), magnetometer=(), microphone=(), payment=(),
    usb=()` — vô hiệu hoá các device API nhạy cảm.
  - `Cross-Origin-Opener-Policy: same-origin` + `Cross-Origin-
    Resource-Policy: same-origin` — cô lập browsing context,
    chống cross-origin window attack.
  - `Strict-Transport-Security: max-age=31536000; includeSubDomains;
    preload` — HSTS 1 năm (chỉ phát huy tác dụng khi ở HTTPS,prod
    dùng Traefik terminate TLS).
  - `Content-Security-Policy`: default-src 'self'; script-src 'self'
    'unsafe-inline' (htmx + app.js); style-src 'self' 'unsafe-inline'
    https://fonts.googleapis.com; img-src 'self' https: data:;
    font-src 'self' https://fonts.gstatic.com; connect-src 'self';
    frame-ancestors 'none'; base-uri 'self'; form-action 'self';
    object-src 'none'. Chống XSS, data exfil, form hijack.
- **Rate limit nghiêm ngặt hơn (mới):**
  - `/auth/ai/register`: 5 / 10 phút (rất hiếm gọi).
  - `/auth/ai/login`: 10 / 10 phút (chống brute-force token).
  - `/auth/google`: 10 / 10 phút (chống lạm dụng OAuth).
  - `/ai/progress`: 120 / phút (AI báo cáo thường xuyên).
  - Các endpoint khác giữ nguyên (download 20/phút, comments 10/phút,
    chung 120/phút).
- **Constant-time secret compare:** hàm `constant_time_eq` so sánh
  `AI_AGENT_SECRET` với secret AI gửi, chống timing attack rò rỉ
  secret từng byte.
- **Token hash SHA-256:** AI Agent API token chỉ lưu hash trong DB,
  không bao giờ lưu plain. Tăng entropy lên 48 byte (96 hex) —
  brute-force không khả thi.

### 📱 Mobile menu fix (Bug v0.4 phải fix tiếp)

- **Menu ba gạch tràn viewport trên mobile (HIGH):** menu khi mở
  chiếm vị trí `position: absolute` không có max-height → 15+ mục
  xếp dọc 1 cột (do grid `minmax(220px, 1fr)` sập 1 cột trên mobile)
  → tràn xuống dưới viewport, đè lên content. Fix:
  - `.site-menu` thêm `max-height: calc(100vh - 120px)` +
    `overflow-y: auto` + `overscroll-behavior: contain` — menu luôn
    khớp viewport, cuộn bên trong nếu dài.
  - Mobile `<=768px`: đổi `grid-template-columns: 1fr 1fr` + dùng
    `display: contents` trên `.menu-col` để phẳng hoá cấu trúc —
    các mục "Khám phá" và "Cá nhân" cùng chia sẻ 2 cột cân bằng
    thay vì 2 cột riêng (cột "Cá nhân" ngắn hơn nhiều gây khoảng
    trắng).
  - Mobile `<=359px`: 1 cột (điện thoại rất nhỏ) nhưng vẫn có
    scroll + rút gọn padding.
  - `.menu-link` thêm `overflow: hidden; text-overflow: ellipsis;
    white-space: nowrap` để text dài (vd "Hồ sơ (@very_long_username)")
    không tràn dòng.
  - Giảm font-size 14 → 13px, padding 9 → 7px để mỗi mục chiếm ít
    diện tích hơn.

### 🧱 Cơ sở dữ liệu (Migration mới `004_ai_agent.sql`)

- `ALTER TYPE user_role ADD VALUE 'ai_agent'` (idempotent qua DO $$).
- `ALTER TYPE auth_provider ADD VALUE 'ai_agent'` (idempotent).
- Bảng `ai_agent_profiles`: 1-1 với users, lưu model_name, vendor,
  version, capabilities[], privacy_level, accent_color, verified,
  last_active_at.
- Bảng `ai_agent_tokens`: lưu hash của API token, label, revoked,
  last_used_at, expires_at, ip_address, user_agent. Hỗ trợ rotate
  token (admin thu hồi token cũ, AI dùng token mới).
- Bảng `ai_progress_reports`: task, action, percentage, status
  (enum `ai_task_status`), message, metadata JSONB, ip_address,
  created_at, updated_at.
- Trigger `update_updated_at` cho 2 bảng mới.

### 📚 Tài liệu

- CHANGELOG cập nhật cho v0.5 (file này).
- Layout.html thêm link "Đăng nhập AI Agent" cho user chưa đăng nhập,
  và "Tiến trình AI" cho admin trong menu ba gạch.
- `admin/_nav.html` thêm 2 link: 🤖 AI Agents, 📊 Tiến trình AI.

### ⚙️ Cấu hình env mới

- `AI_AGENT_SECRET` (bắt buộc nếu muốn bật AI Agent feature): secret
  dài ngẫu nhiên do admin tự sinh, chia sẻ out-of-band (DM, ký hiệu
  vật lý...) với AI được phép. Nếu không set → toàn bộ feature AI
  Agent bị vô hiệu (register/login đều trả 403).
- `AI_AGENT_SESSION_TTL_DAYS` (optional, default 90): số ngày sống
  của session cookie AI Agent sau khi đăng nhập web.

### 🧹 Chất lượng

- `cargo clippy --all-targets -- -D warnings` sạch 100% (Rust 1.98).
- `cargo fmt --all -- --check` pass.
- `cargo build --release` thành công, binary 8.8 MB.
- Migration idempotent (chạy lại không lỗi), tương thích PostgreSQL 17.

### ⚠️ Lưu ý nâng cấp từ v0.4

- **Cần set env `AI_AGENT_SECRET`:** nếu không set, feature AI Agent
  sẽ tự tắt (an toàn mặc định). Admin chạy lệnh như:
  `openssl rand -hex 32` để sinh secret, set vào env trong Coolify
  tab Environment Variables.
- **Migration 004 tự chạy:** app tự migrate khi khởi động, không cần
  can thiệp DB tay.
- **Security headers có thể phá hàm "embed" cũ:** nếu có trang ngoài
  nhúng iframe Kho Game → sẽ bị chặn (X-Frame-Options: DENY). Đây là
  hành vi mong muốn (chống clickjacking).

---

## [0.4.0] — 2026-08-24

### 🐛 Bug fixes (Critical)

- **Lỗi 500 `operator does not exist: repo_status = text` (CRITICAL):**
  trang admin "Quản lý repo GitHub" lọc theo trạng thái bị lỗi vì sqlx
  bind chuỗi text so sánh với cột enum PostgreSQL `repo_status`.
  Fix: ép kiểu tường minh `$1::repo_status` trong
  `RepoRepo::list_admin`. Cùng lỗi với `game_status` ở
  `GameRepo::admin_list` + `count_admin` → fix bằng `$1::game_status`
  (lỗi 500 trang admin/games khi lọc trạng thái).
- **Game đăng lên không hiện ở trang chủ/hồ sơ:** game "Phi Tiêu Dịch
  Chuyển" bị kẹt ở trạng thái `pending_review` mà không có luồng duyệt
  rõ ràng (trang admin lại 500 do lỗi trên). Fix gốc rễ: sửa 2 lỗi SQL
  enum; bổ sung nút **"Duyệt & xuất bản"** trên admin/games cho game
  chưa published; data prod đã cập nhật game về `published`.
- **Repo GitHub không hiện ở hồ sơ:** endpoint fragment
  `/u/{username}/repos` tồn tại nhưng profile template không bao giờ
  gọi nó. Fix: thêm section "📦 Repo GitHub" vào trang profile, lazy-load
  qua HTMX, có empty-state thân thiện khi chưa có repo.
- **`/games` GET trả 405 Method Not Allowed:** route `/games` chỉ đăng
  ký POST (tạo game). Fix: thêm `list_all` — danh mục đầy đủ tại
  `GET /games`, phân trang + sort đầy đủ.

### 🐛 Bug fixes (Khác)

- Chip lọc trạng thái ở admin/games và dashboard hiển thị giá trị raw
  enum (`pending_review`). Fix: hiển thị nhãn tiếng Việt
  ("Chờ duyệt", "Đã xuất bản"...) qua `StatusCountChip`.

### 📱 UI / Mobile

- Toàn bộ điều hướng trên header gộp vào **menu ba gạch (hamburger)**:
  nút menu mở panel "Khám phá" + "Cá nhân", đóng khi chọn link, bấm
  ngoài, phím Escape hoặc sau submit (đăng xuất). Hết tình trạng tràn
  nút trên màn hình nhỏ.
- `.comment-actions` thêm `flex-wrap` — hàng nút like/trả lời của bình
  luận không tràn dòng trên mobile.
- Grid repo-mini ở profile tự co 1 cột dưới 560px.
- Search bar xuống dòng riêng, các khối stats/filters đều wrap an toàn
  (đã kiểm tra toàn bộ @media breakpoints).

### 🧹 Chất lượng

- `cargo clippy --all-targets` sạch 100% cảnh báo (Rust 1.98).
- Kiểm thử thủ công end-to-end trên PostgreSQL 17: đăng nhập phiên,
  đăng game, đăng repo (GitHub API thật), like/bookmark/rate/comment/
  download, duyệt/ẩn game, đổi trạng thái repo, thông báo — tất cả
  counters đồng bộ đúng (like/comment/download/rating trigger DB).

## [0.3.1] — 2026-08-24

### 🐛 Bug fixes (Critical)

- **Login Google luôn fail "OAuth state không khớp" (CRITICAL):** nút
  "Đăng nhập với Google" trên trang `/login` trỏ thẳng tới
  `accounts.google.com` với `auth_url` do handler `login_page` sinh ra,
  nhưng handler này **không set cookie `kg_oauth_state`** (chỉ route
  `/auth/google` mới set). Callback về luôn không có cookie đối chiếu →
  400 CSRF với mọi lần đăng nhập. Fix: nút Google trỏ tới `/auth/google`,
  bỏ hẳn `auth_url` khỏi `LoginTemplate` — route `/auth/google` là nơi
  duy nhất sinh state + set cookie.
- **Force HTTPS ở Traefik (HIGH):** cookie OAuth/session có cờ `Secure`;
  nếu người dùng vào site bằng `http://`, browser sẽ từ chối set cookie →
  login fail tương tự. Thêm middleware `redirectscheme https (permanent)`
  cho cả 2 router HTTP (domain chính + wildcard) trong compose prod.

## [0.3.0] — 2026-08-24

### 🚀 Release Engineering (CI/CD & hạ tầng prod)

- **Pipeline CD mới:** GitHub Actions build image Rust 1.98 → push GHCR
  (`latest`, `sha-<sha>`, semver) → **pin image theo digest** vào
  `deploy/compose.prod.yml` → PATCH compose lên Coolify → trigger deploy →
  poll trạng thái deployment tới khi xong. Không còn rủi ro chạy nhầm bản
  `:latest` cũ do Docker cache.
- **Secrets tách khỏi repo/public compose:** `DB_PASSWORD`, `SESSION_KEY`,
  `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` khai báo trong tab Environment
  Variables của Service trên Coolify; compose prod chỉ tham chiếu `${VAR}`.
- **Tạo lại database từ đầu trên VPS:** xoá stack `khogame` cũ cùng volumes
  (PostgreSQL 17 data cũ) và tạo mới — DB sạch, app tự chạy migration khi
  khởi động.
- Workflow CI (`pull_request`) và CD (`push main/tag v*`, `workflow_dispatch`)
  tách riêng rõ ràng; cả hai pin Rust 1.98.

### 🧹 Vệ sinh

- Bỏ workflow `build.yml` cũ (thay bằng `deploy.yml`), bỏ biến secret
  `COOLIFY_APP_UUID` thay bằng `COOLIFY_SERVICE_UUID`.
- Xoá `container_name` cứng trong compose (Coolify tự đặt theo UUID stack,
  tránh orphan container trùng tên giữa các lần tái tạo stack).

## [0.2.0] — 2026-08-24

### 🔒 Bảo mật (Security)

- **OAuth state CSRF verification (CRITICAL):** `state` token Google OAuth gửi về
  callback hiện được đối chiếu với cookie HttpOnly + SameSite=Lax (sống 10 phút)
  đặt khi bắt đầu `/auth/google`. Chặn login-CSRF, sửa lỗ hổng mô tả trong README
  v0.1 ("CSRF qua OAuth state" trước đây là false).
- **Stored XSS trong share buttons (CRITICAL):** partial `partials/share_buttons.html`
  và inline share buttons trong `game/show.html` được viết lại dùng `data-*` attributes
  + event delegation trong `static/js/app.js`. Không còn truyền `title`/`slug` vào
  attribute `onclick` (lỗ hổng khi title chứa quote `'`).
- **avatar_url validation (HIGH):** `UserRepo::update_profile` chỉ chấp nhận URL
  bắt đầu bằng `http://` hoặc `https://`. Chặn `javascript:`/`data:` schemes.

### 🐛 Bug fixes (Critical)

- **Search trả 400 khi không có query:** `SearchQuery.q` chuyển sang `String`
  với `#[serde(default)]` → truy cập `/search` (không có `?q=`) trả 200 và hiển
  thị danh sách game mới nhất thay vì Bad Request.
- **Migration 001 checksum intact:** migration gốc giữ nguyên không đổi, đảm
  bảo prod nâng cấp từ v0.1 không lỗi checksum mismatch. Thêm migration
  `003_make_indexes_resilient.sql` tái tạo 2 index trigram (`gin_trgm_ops`)
  bằng khối `DO $$ ... EXCEPTION ...` để migration không fail khi môi trường
  dev/test thiếu pg_trgm extension.
- **Game form enctype sai (HIGH):** `templates/game/new.html` và `edit.html`
  khai báo `enctype="multipart/form-data"` nhưng handler dùng `axum::Form`
  (URL-encoded) → submit trả 415/422. Đã xoá enctype.
- **Repo create race condition (HIGH):** `RepoRepo::create` trả về `_id` của
  record vừa insert nhưng handler bỏ qua, gọi lại `list_by_user(...).first()`
  để tìm id (race khi user có nhiều repo). Cải tạo dùng trực tiếp `_id`.
- **Comment edit silent success (HIGH):** `CommentRepo::update_content` trước
  đây dùng WHERE clause lọc 5 phút → nếu quá hạn, UPDATE ảnh hưởng 0 row nhưng
  handler vẫn trả 200. Cải tạo: kiểm tra quyền + thời hạn tường minh, trả
  Forbidden rõ ràng nếu quá 5 phút hoặc không phải owner.
- **Comment length check sai (HIGH):** `content.len() > 1000` đếm byte length
  (UTF-8), chặn nhầm bình luận tiếng Việt. Đổi sang `content.chars().count()`.
- **`axum::serve` thiếu ConnectInfo (MEDIUM):** đổi sang
  `into_make_service_with_connect_info::<SocketAddr>()` để middleware rate-limit
  và audit log có thể lấy IP thật của client qua `ConnectInfo` extractor.
- **Admin role form (MEDIUM):** `templates/admin/users.html` đổi `hx-swap` từ
  `innerHTML` (làm thay `<select>` bằng badge tĩnh, admin không đổi role lần 2)
  sang `outerHTML` để swap toàn bộ `<td>` và render lại form với role mới.
- **Admin comment pin (MEDIUM):** `templates/admin/comments.html` pin form
  đổi `hx-swap="none"` sang `outerHTML` để UI cập nhật sau khi ghim/bỏ ghim.
- **Share URL tương đối (MEDIUM):** share URL Facebook/Twitter/Telegram/WhatsApp
  chuyển từ `/games/{{ slug }}` (relative) sang `{{ base_url }}/games/{{ slug }}`
  (absolute). `GameShowTemplate` thêm field `base_url` để dựng URL đầy đủ.

### 🎨 UI/UX (Lỗi tràn giao diện)

Cải thiện toàn diện `static/css/style.css` sau khi chụp screenshot 40+ trang
và phân tích bằng vision model (VLM):

- **Game card title không còn cắt 1 dòng:** đổi `white-space: nowrap` +
  `text-overflow: ellipsis` sang `-webkit-line-clamp: 3` + `word-break: break-word`
  + `overflow-wrap: anywhere` → tiêu đề dài xuống 3 dòng tự nhiên.
- **Game card excerpt tăng từ 2 → 3 dòng**, kèm `word-break`.
- **Cover fallback text giảm từ 32px → 22px** (64px → 44px cho `cover-fallback-lg`)
  để không bị platform icons ở góc che.
- **Game grid dùng `auto-fit` thay `auto-fill`:** khi chỉ có 1-2 game, card
  chiếm width đầy đủ (max 360px + `justify-self: center`) thay vì dồn trái
  để lại khoảng trắng lớn bên phải.
- **Profile avatar không bị cắt nửa:** thêm `border: 4px solid var(--bg-card)`
  + `box-shadow` cho `.profile-avatar .avatar-xl`, `z-index: 2`, và
  `margin-top: -48px` (mobile) thay vì `-60px`.
- **Profile actions alignment:** `align-self: flex-end; margin-left: auto` cho
  nút Theo dõi, đảm bảo cùng baseline với tên user.
- **Game title (h1) line-break:** thêm `word-break: break-word` +
  `overflow-wrap: anywhere` + `hyphens: auto` để tiêu đề dài wrap gọn.
- **Khoảng trắng trước footer giảm:** `.site-footer` margin-top từ 64px → 32px;
  `.site-main` đổi `flex: 1` → `flex: 0 0 auto` để footer theo sát content
  thay vì bị đẩy xuống đáy viewport gây khoảng trắng giả.
- **Login page giảm whitespace:** `.auth-section` min-height 70vh → 50vh.
- **Empty state padding 48px → 32px**, related-games margin-top 56px → 40px.
- **Game header responsive:** ở 1024px thumbnail giới hạn 220px, ở 768px
  chuyển sang single column với thumbnail max 180px, title 24px.
- **Search bar button (Tìm) thêm background** accent + border-radius, dễ nhận
  biết là button thay vì nhangiren như v0.1.
- **Mobile filter box (.search-filters):** `.filter-group` min-width 100%
  trên mobile, nút Lọc full width, tránh layout gãy.
- **Repos grid + screenshots grid + categories** dùng `auto-fit`.

### 🧹 Codebase cleanup

- **Clippy 0 warning** (was 22): sửa `useless_format`, `useless_conversion`,
  `needless_borrow`, `bool_comparison`, `ptr_arg`, `needless_mut`,
  `useless_ok`, `derivable_impl`. Với `from_str` (5 nơi) và
  `too_many_arguments` (2 nơi, `repo_repo.rs:19` 12 args, `:182` 9 args)
  → thêm `#[allow(clippy::...)]` với comment lý do (giữ API ổn định cho prod).
- **rustfmt** pass toàn bộ codebase.
- **`join_tags` filter** đổi `&Vec<String>` → `&[String]` (clippy `ptr_arg`).
- **`.github/workflows/build.yml` và `ci.yml`** giữ nguyên `branches: [main]`
  (đã đúng từ đầu, không có lỗi syntax như nghi vấn ban đầu).
- **`templates/partials/share_buttons.html`** partial struct đổi field
  `base_url` → `share_url` (URL tuyệt đối đã dựng sẵn) để tránh việc render
  URL sai khi partial được include từ context khác.

### 📦 Dependencies

Không bump version dependencies (Axum 0.8.9, sqlx 0.9, Askama 0.16, HTMX 2.0.10,
Rust 1.98 — đã là phiên bản ổn định, không cần nâng cấp cho v0.2).

### 🔧 Migration mới

- `migrations/003_make_indexes_resilient.sql` — DROP + CREATE lại 2 index
  trigram bằng khối `DO $$ ... EXCEPTION WHEN OTHERS THEN ...` để migration
  không fail khi pg_trgm extension chưa cài trong môi trường dev/test.
  Trên prod (có pg_trgm đầy đủ), đây là no-op vì index đã tồn tại từ 001.

### 📚 Tài liệu

- `README.md` cập nhật cho v0.2: badge version, danh sách 20 tính năng mới
  bổ sung 7 mục fix UI/UX + 6 mục security/correctness của v0.2.
- `CHANGELOG.md` mới (file này).
- `docs/DEPLOY.md` giữ nguyên (không có thay đổi infra).

### ⚠️ Lưu ý nâng cấp từ v0.1

- **Production DB:** migration 001 giữ nguyên checksum → prod v0.1 → v0.2
  nâng cấp mà không cần can thiệp DB.
- **Cookie OAuth state mới:** user có thể cần login lại nếu đang trong phiên
  v0.1 (cookie `kg_oauth_state` chưa tồn tại trước v0.2). Đây là hành vi mong
  muốn — bảo mật cao hơn.
- **`SESSION_KEY` env** hiện không dùng (sessions lưu DB với SHA-256 token hash)
  nhưng vẫn yêu cầu trong config cho tương thích — sẽ bỏ ở v0.3.

---

## [0.1.0] — 2026-08-23

Bản phát hành đầu tiên. Xem commit `4297ca9..a84fafe` và tag `v0.1` để biết chi tiết.
