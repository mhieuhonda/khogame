# 🚀 Louis Space

> Nền tảng chia sẻ game độc lập & tin tức cộng đồng cho Việt Nam, xây dựng bằng Rust.

![Rust](https://img.shields.io/badge/Rust-1.98-orange?logo=rust)
![Axum](https://img.shields.io/badge/Axum-0.8.9-blue)
![PostgreSQL](https://img.shields.io/badge/PostgreSQL-17-blue?logo=postgresql)
![HTMX](https://img.shields.io/badge/HTMX-2.0-blue)
![Askama](https://img.shields.io/badge/Askama-0.16-purple)
![sqlx](https://img.shields.io/badge/sqlx-0.9-green)
![Tests](https://img.shields.io/badge/tests-159%2B-brightgreen)
![Version](https://img.shields.io/badge/version-1.0.0-0f172a)

## 🛠️ Công nghệ

| Thành phần | Công nghệ |
|-----------|-----------|
| Ngôn ngữ | Rust 1.98  |
| Web framework | Axum 0.8.9 + axum-extra 0.12 |
| Template engine | Askama 0.16 |
| Frontend interactivity | HTMX 2.0.10 (self-hosted) |
| Database | PostgreSQL 17 |
| ORM | sqlx 0.9 (runtime-tokio + rustls) |
| Auth | Google OAuth 2.0 (CSRF state verification) |
| HTTP Client | reqwest 0.12 |
| Styling | Custom CSS (dark/light mode) |

## ✨ Tính năng cốt lõi
- 🔐 **Đăng nhập Google OAuth 2.0** (duy nhất cho người dùng thường)
- 🤖 **Tài khoản AI Agent** (v0.5): loại tài khoản đặc biệt cho AI do admin ủy quyền — đăng ký bằng secret, đăng nhập bằng token dài hạn, báo cáo tiến trình về trang admin, hồ sơ công khai có huy hiệu phân biệt
- 🎮 **Đăng game** với link tải ẩn cho 5 nền tảng: Android, iOS, Windows, Linux, macOS
- 💬 **Bình luận** threaded + trả lời + **@mention** (kèm notification)
- ❤️ Like game & bình luận · ⭐ Đánh giá 1-5 sao · 🔖 Bookmark · 👥 Theo dõi tác giả
- 🔔 **Thông báo** (comment, like, follow, mention, hệ thống)
- 🚩 **Báo cáo** nội dung với workflow pending → reviewing → resolved/dismissed
- 🏷️ Tags · 📁 Thể loại · 🖼️ Screenshot gallery · 🎬 Trailer YouTube nhúng
- 🔍 Tìm kiếm + lọc (thể loại, nền tảng, sắp xếp) · phân trang
- 🌓 Dark/Light mode (đồng bộ server-side) · 📱 Responsive

## 🆕 20 tính năng mới trong v0.1
1. 📦 **Repo GitHub** — đăng repo mã nguồn, tự fetch stars/forks/ngôn ngữ từ GitHub API, liên kết với game, admin duyệt/ẩn
2. 🛡️ **Admin dashboard nâng cao** — chart 7 ngày (views/downloads/game mới/user mới)
3. 👥 **Admin quản lý users** — tìm kiếm, đổi role (user/moderator/admin), ban/unban
4. 🎮 **Admin quản lý games** — lọc trạng thái, ẩn/hiện, đặt nổi bật, xóa
5. 💬 **Admin quản lý comments** — ghim, xóa, xem mới nhất
6. 📁 **Admin quản lý categories** — CRUD đầy đủ
7. ⚙️ **Admin settings** — tên site, mô tả, footer, auto-approve repo
8. 🛠️ **Maintenance mode** — chặn truy cập khi bảo trì (admin bypass)
9. 📜 **Audit log** — ghi mọi hành động quản trị
10. 📢 **Broadcast** — gửi thông báo hàng loạt tới toàn bộ user
11. 📢 **Announcement banner** toàn site (4 kiểu màu, tự ẩn được)
12. 🌐 **Public JSON API v1** — `/api/v1` (discovery), `/api/v1/games`, `/api/v1/games/{slug}`, `/api/v1/games/{slug}/related`, `/api/v1/games/{slug}/comments`, `/api/v1/repos`, `/api/v1/tags`, `/api/v1/categories`, `/api/v1/categories/{slug}/games`, `/api/v1/tags/{slug}/games`, `/api/v1/users/{username}`, `/api/v1/stats`, `/api/v1/health`
13. 📡 **RSS feed** — /rss.xml
14. 🗺️ **Sitemap + robots.txt** — SEO thân thiện
15. 🔍 **OG meta tags** — chia sẻ đẹp trên mạng xã hội
16. 🚦 **Rate limiting** — download 20/phút, comment 10/phút theo IP
17. 📋 **My Games** — quản lý game của tôi (kể cả draft), xuất bản 1 click
18. ✏️ **Sửa bình luận** trong 5 phút
19. ⚠️ **Cảnh báo trùng tiêu đề** khi đăng game (AJAX realtime)
20. 📥 **Export backup JSON** (admin) + Health check nâng cao kèm DB status

## 🤖 Tính năng mới trong v0.5 — AI Agent account system

Loại tài khoản thứ 4 (sau User / Moderator / Admin) dành riêng cho AI Agent
được admin ủy quyền để fix lỗi hoặc làm tác vụ bảo trì:

1. **Đăng ký bí mật** (`POST /auth/ai/register`): AI gửi `AI_AGENT_SECRET`
   từ env (chỉ admin biết) + metadata model. Server verify constant-time.
2. **API token dài hạn** (48 byte entropy, hash SHA-256 trong DB): chỉ
   trả 1 lần khi đăng ký. AI dùng cho mọi request sau này.
3. **Đăng nhập web** (`/auth/ai/login`): AI nhập token → nhận session
   cookie 90 ngày, dùng web UI như user thường.
4. **Báo cáo tiến trình** (`POST /ai/progress`): AI gửi task/action/
   percentage/status. Admin xem live feed tại `/admin/ai-reports` (tự
   refresh 30s, kèm progress bar + % + status màu sắc).
5. **Hồ sơ AI công khai**: huy hiệu "🤖 AI Agent" (hoặc "✓ AI Agent"
   nếu verified) trên profile, kèm metadata model/vendor/capabilities.
   AI có thể ẩn danh (privacy_level=anonymous) để giấu tên model.
6. **AI tự chỉnh hồ sơ** (`/profile/ai/edit`): chỉnh model_name, vendor,
   capabilities, accent_color, privacy level.

### 🔒 Tăng bảo mật toàn site (v0.5)
- Security headers middleware (CSP, X-Frame-Options DENY, HSTS, COOP,
  COEP, Referrer-Policy, Permissions-Policy) áp dụng cho mọi response.
- Rate limit nghiêm ngặt hơn cho AI/auth endpoints (5-10/10 phút cho
  register/login, 120/phút cho progress).
- Constant-time secret compare (chống timing attack).
- Token SHA-256 + 48 byte entropy (brute-force không khả thi).

### 📱 Mobile menu fix (v0.5)
- Menu ba gạch trên mobile tràn viewport → fix bằng max-height + scroll
  + 2 cột cân bằng (display: contents phẳng hoá cấu trúc).

---

## 🚀 Cải thiện trong v0.6 — Bảo mật + SEO + Perf + Ops

### 🔒 Bảo mật & đúng đắn
- ✅ **Rate limit dùng IP thật** — trước đây `client_ip_from_parts(headers, None)`
  → khi không có proxy header, IP luôn là `unknown`, ai cũng cùng share quota.
  Giờ lấy `ConnectInfo<SocketAddr>` từ request extensions.
- ✅ **Log User-Agent & IP khi login Google** vào bảng `sessions` để admin audit
  (trước đây lưu `user_agent=""` do TODO chưa hoàn thành).
- ✅ **robots.txt** thêm `Disallow: /api/` — ngăn crawler đánh JSON API.

### ✨ Tính năng mới
- 📱 **PWA manifest** (`/manifest.json`) — "Add to Home Screen" hoạt động.
- 🔍 **OpenSearch** (`/opensearch.xml`) — thêm Louis Space vào ô tìm kiếm của
  thanh địa chỉ Chrome / Firefox / Edge.
- 🛡️ **security.txt** (`/.well-known/security.txt`, RFC 9116) — thông tin
  liên hệ báo lỗ hổng, mailto:admin, Expires 6 tháng.
- 🎯 **JSON-LD VideoGame** — trang `/games/{slug}` nhúng structured data
  schema.org → Google hiển thị rich snippet (rating, lượt xem/tải/like).
- 🚧 **404 fallback page** — route không khớp → trang 404 với giao diện
  Louis Space thay vì plain text "Not Found".
- 📡 **API catalog mới**: `GET /api/v1/tags`, `GET /api/v1/categories`.
- 📦 **API game detail đầy đủ**: thêm `category`, `screenshots`, `label` &
  `url` cho từng platform, `published_at`.
- 🗺️ **Sitemap mở rộng**: thêm `/games/featured` và 50 tag phổ biến nhất.

### ⚡ Hiệu suất
- ⚡ **ETag cho `/rss.xml` & `/sitemap.xml`** — client gửi `If-None-Match`
  khớp → 304 Not Modified, giảm băng thông & TTFB cho crawler.
- 📦 **Static cache 7 ngày** — `/static/*` có `Cache-Control: public,
  max-age=604800, stale-while-revalidate=86400`.
- 🚑 **`/health` no-store** — monitor cần trạng thái real-time, không cache.

### 🔧 Operational
- 🆔 **`X-Request-Id` header** — mỗi request được gán UUID, log trace và
  Coolify có thể group request theo ID khi debug.
- 🧹 Dọn dead code trong `src/handlers/api.rs`.
- ✅ **13 unit test** pass (thêm 5 test: `sanitize_redirect`, `truncate`
  UTF-8, `make_unique_slug`, `html_escape`, `format_number`).

---

## 🔥 Cải thiện trong v0.7 — Prod hardening marathon

### 🛡️ Bảo mật (lỗ hổng thật đã fix)
- 🔓 **Path traversal trong `parse_github_url`** — `../etc/passwd` được chấp nhận
  làm owner (Some(("..", "etc"))) → URL GitHub API thành `/repos/../etc`.
  Giờ chặn segment `.` và `..`.
- 🔐 **Chặn tương tác game chưa xuất bản** — comment, download, like, bookmark,
  rate, report trên game draft/hidden khi biết slug (trang xem đã chặn nhưng
  endpoint POST thì chưa).
- 🔡 **Escape wildcard ILIKE** — tìm "100%" giờ khớp literal thay vì match
  cả "1001"; query `%%` không còn match toàn bảng.
- ✂️ **Clamp search query 200 ký tự** — chống pattern khổng lồ làm ILIKE quét
  chậm (DoS).
- 🙈 **Ẩn profile user bị ban** khỏi HTML (API đã chặn từ trước).
- 🖼️ **Validate avatar_url http(s)** + whitelist theme/language ở profile.
- 📏 **edit_comment áp dụng giới hạn 1000 ký tự** (trước chỉ check lúc tạo).

### ⚡ Hiệu suất
- 🔔 **find_mentions: N+1 → 1 query** (`= ANY($1)`).
- 📋 **resolve_report / mark_read**: fetch đúng 1 dòng thay vì 50-200 dòng
  để re-render 1 item HTMX.
- 🖼️ **width/height + decoding=async cho mọi `<img>`** — chống Cumulative
  Layout Shift (Core Web Vitals).

### ✨ Tính năng & UX
- 🛑 **Graceful shutdown SIGTERM/SIGINT** + `stop_grace_period: 30s` — deploy
  không còn đứt request đang xử lý.
- 🧹 **Background janitor** dọn session hết hạn & notification đã đọc > 90 ngày
  mỗi 6h (`JANITOR_INTERVAL_SECS`).
- ❤️ **Health check nâng cao**: `pool.size/idle/in_use` + `uptime_secs`.
- ⚙️ **DB pool tuning qua env**: `DB_MAX_CONNECTIONS`, `DB_MIN_CONNECTIONS`,
  `DB_ACQUIRE_TIMEOUT_SECS`.
- 🔖 **Phân trang trang bookmark** (trước hardcode 50 không có trang 2).
- 🏷️ **Tag dedupe case-insensitive** khi tạo game.
- 🎬 **Trailer không phải YouTube** hiển thị link fallback thay vì iframe trắng.
- 🚦 **Toast 429/503** thân thiện thay vì lỗi chung chung.
- 🔍 **Ô search giữ từ khóa** sau khi chuyển trang.

### 📈 SEO
- 🖼️ **og:image + twitter:image + canonical** cho trang game & danh sách.
- 📡 **RSS atom:link rel=self + ttl** (đạt W3C Feed Validator).
- 🗺️ **Sitemap thêm /terms, /privacy**.
- 📝 **Meta description riêng cho từng list** (category/tag) chống duplicate.

### ♿ Accessibility
- 🎞️ **prefers-reduced-motion** — tắt animation cho người rối loạn tiền đình.
- ⌨️ **:focus-visible** — outline rõ khi điều hướng bàn phím.
- 🖨️ **Print stylesheet** — in trang game sạch sẽ.
- 🔢 **aria-current / rel=prev/next** cho mọi pagination.

### ✅ Testing
- **128+ unit test** (từ 23): validate_game_form, RateLimiter, parse_github_url,
  constant_time_eq, askama filters, UserRole matrix, auth token, ReportReason,
  path normalization, BreadcrumbList, slugify tiếng Việt, html_escape unicode…
- **CI nghiêm ngặt**: clippy `-D warnings` + rustdoc `-D warnings` + fmt + test
  chạy trên mọi push main + cargo-audit RustSec mỗi push.

---

## 🔥 Cải thiện trong v0.8 — Bảo mật race-condition + Admin hoàn thiện

### 🛡️ Bảo mật
- **Rate-limit chống bypass xoay slug/UUID** — chuẩn hoá path thành bucket
  endpoint; kèm `Retry-After` header chuẩn RFC 9110.
- **TRUST_PROXY_HEADERS env** — kiểm soát tin header IP proxy.
- **CSP thêm frame-src** — YouTube trailer hoạt động lại (bị default-src
  chặn từ trước), embed chuyển youtube-nocookie.com.
- **Escape XML/HTML đầy đủ** sitemap, RSS, repo mini-card.

### 🐛 Race conditions đã fix (double-click/đồng thời)
- Tạo game trùng slug (TOCTOU) → catch unique violation + retry slug.
- Like/comment-like double-increment → DELETE-first transaction.
- Follow + notification cùng transaction (atomic).

### 🐛 Admin
- **Phân trang cho 4 trang admin** (reports/comments/audit/repos) — trước đây
  vượt 50-200 dòng là dữ liệu cũ MẤT DẠG không thể xem.
- Tạo category trùng slug → 409 thay vì âm thầm đổi tên category cũ.
- Broadcast không còn gửi notification cho AI Agent (bot không đọc).

### ⚡ Performance
- View/download/share counters chạy nền (tokio::spawn) — không block TTFB.
- 6 handler song song hoá thêm (profile, sitemap, stats, API).
- Migration 007: index like_count cho sort "Yêu thích".
- ETag cho /api/announcement (304 thay payload).

### ♿ Accessibility (WCAG)
- Autocomplete keyboard navigation (↑ ↓ Enter Esc) — WCAG 2.1.1.
- 54 form label for/id; report modal Escape + focus + role=dialog.
- Rating stars aria-pressed (sửa aria-checked không hợp lệ).

---

## 🔧 Cải thiện trong v0.2

### 🔒 Bảo mật
- ✅ **OAuth state CSRF verification** — `state` token được verify với cookie HttpOnly + SameSite=Lax, chặn login-CSRF
- ✅ **Stored XSS fix** trong share buttons — chuyển sang `data-*` attributes + event delegation
- ✅ **avatar_url validation** — chỉ chấp nhận `http(s)://`, chặn `javascript:`/`data:` schemes

### 🐛 Bug fixes
- ✅ Search trả 400 khi không có `?q=` → giờ trả 200 + list game mới nhất
- ✅ Game form `enctype="multipart/form-data"` sai → submit trả 415/422 (đã xoá)
- ✅ Repo create race condition (dùng `list_by_user(...).first()` thay vì id trả về)
- ✅ Comment edit silent success khi quá 5 phút → giờ trả Forbidden rõ ràng
- ✅ Comment length check sai (byte length thay vì char count) → chặn nhầm bình luận tiếng Việt
- ✅ Share URL tương đối → tuyệt đối (Facebook/Twitter/Telegram/WhatsApp share hoạt động đúng)
- ✅ `axum::serve` thiếu `into_make_service_with_connect_info` → rate-limit không lấy được IP thật
- ✅ Admin role/comment pin form HTMX swap sai (innerHTML/none → outerHTML)

### 🎨 UI/UX (lỗi tràn giao diện)
- ✅ Game card title: 1 dòng → 3 dòng với `line-clamp`, `word-break`, `overflow-wrap`
- ✅ Game card excerpt: 2 dòng → 3 dòng
- ✅ Cover fallback text giảm 32px → 22px để không che platform icons
- ✅ Game grid `auto-fill` → `auto-fit`: khi 1-2 game, card fill width + căn giữa
- ✅ Profile avatar: thêm border + box-shadow, không còn bị cắt nửa trên
- ✅ Profile actions: `align-self: flex-end` để nút Theo dõi cùng baseline với tên
- ✅ Game title (h1): `word-break` + `overflow-wrap` để wrap gọn khi tiêu đề dài
- ✅ Khoảng trắng trước footer giảm: `flex: 1` → `flex: 0 0 auto` cho `.site-main`, footer margin-top 64→32px
- ✅ Login page min-height 70vh → 50vh, giảm khoảng trắng thừa
- ✅ Mobile filter box: filter-group min-width 100%, nút Lọc full width
- ✅ Search bar button "Tím" thêm background accent, dễ nhận biết là button
- ✅ Game header responsive: 1024px thumbnail 220px, 768px single column với thumbnail 180px

### 🧹 Codebase
- ✅ **Clippy 0 warning** (was 22): sửa toàn bộ useless_format, needless_borrow, bool_comparison, ptr_arg, needless_mut, useless_ok, derivable_impl
- ✅ rustfmt pass toàn bộ
- ✅ Migration 003 mới: tái tạo 2 index trigram bằng `DO EXCEPTION` để migration không fail khi thiếu pg_trgm

## 🚀 Chạy local

```bash
# 1. Clone
git clone https://github.com/mhieuhonda/khogame.git
cd khogame

# 2. Cấu hình
cp .env.example .env   # sửa DATABASE_URL, GOOGLE_*

# 3. Chạy bằng Docker (đã kèm Postgres 17)
docker compose up -d --build

# Hoặc chạy thường (cần Postgres 17 + Rust 1.98)
cargo run
```

Server khởi động tại `http://localhost:3000`. Migration tự chạy khi khởi động.

## 🐳 Production với Coolify

Xem hướng dẫn đầy đủ: [docs/DEPLOY.md](docs/DEPLOY.md)

Tóm tắt flow CI/CD:
```
git push → GitHub Actions build image → push GHCR
        → POST webhook Coolify (bản 4.3.7 yêu cầu POST)
        → Coolify pull image mới → deploy tự động
```

## 📁 Cấu trúc dự án

```
khogame/
├── src/
│   ├── main.rs              # Entry point
│   ├── lib.rs               # run() + migrations
│   ├── config.rs            # AppConfig (env, includes AI_AGENT_SECRET)
│   ├── state.rs             # AppState + rate limiter + maintenance cache
│   ├── db.rs                # PgPool
│   ├── auth.rs              # Google OAuth + AI Agent token helpers
│   ├── error.rs             # AppError → HTTP
│   ├── middleware.rs        # CurrentUser/AuthUser/AuthAiAgent + admin + AI + security headers + rate limit
│   ├── routes.rs            # Router (70+ routes, includes /auth/ai/* + /ai/* + /admin/ai-*)
│   ├── templates.rs         # Askama templates + custom filters (incl. AiLogin/AiProfileEdit/AdminAi*)
│   ├── models/              # User, Game, Comment, Repo, Settings, AI Agent...
│   ├── repositories/        # SQL queries (incl. ai_agent.rs)
│   └── handlers/            # HTTP handlers (games, admin, ai_agent, api, ...)
├── templates/               # Askama HTML (admin/, auth/ai_login.html, profile/ai_edit.html, ...)
├── static/                  # CSS + JS (htmx 2.0.10 self-hosted) + AI badge styles
├── migrations/              # SQL migrations (001, 002, 003, 004_ai_agent)
├── .github/workflows/       # CI + Build & Deploy Coolify
├── Dockerfile               # Multi-stage Rust 1.98 → debian-slim
└── docker-compose.yml       # Local: app + Postgres 17
```

## 🔐 Bảo mật
- Cookie httpOnly + session hash SHA-256 trong DB
- **OAuth state CSRF verification** (v0.2): `state` token được verify với cookie HttpOnly + SameSite=Lax sống 10 phút
- CSRF qua OAuth state
- SQL injection: 100% prepared statements (sqlx)
- HTML escaping tự động (Askama) + `Safe` có kiểm soát
- **avatar_url validation** (v0.2): chỉ chấp nhận `http(s)://`, chặn `javascript:`/`data:`
- RBAC: user / moderator / admin / **ai_agent** (v0.5)
- Rate limiting theo IP (v0.2: lấy IP thật qua `ConnectInfo<SocketAddr>`)
- Admin: đăng nhập Google với `ADMIN_EMAIL` được whitelist
- **Security headers toàn site** (v0.5): CSP, X-Frame-Options DENY, X-Content-Type-Options nosniff, Referrer-Policy, Permissions-Policy, COOP/COEP, HSTS preload
- **AI Agent auth** (v0.5): secret constant-time compare, API token 48 byte entropy + SHA-256 hash, `require_ai_agent` middleware cho `/ai/*` routes
- **Rate limit nghiêm ngặt cho AI/auth** (v0.5): 5/10 phút cho register, 10/10 phút cho login, 120/phút cho progress report

## 🤖 AI Agent — Hướng dẫn dùng (v0.5)

### Admin setup
```bash
# 1. Sinh secret ngẫu nhiên (64 char hex)
openssl rand -hex 32
# 2. Set env trong Coolify tab Environment Variables:
#    AI_AGENT_SECRET=<chuỗi vừa sinh>
#    AI_AGENT_SESSION_TTL_DAYS=90   # optional, default 90
# 3. App tự migrate bảng 004_ai_agent.sql khi khởi động.
# 4. Chia sẻ secret cho AI (qua kênh riêng: DM, ký hiệu vật lý...).
```

### AI đăng ký (chỉ làm 1 lần)
```bash
curl -X POST https://your-domain.com/auth/ai/register \
  -H 'Content-Type: application/json' \
  -d '{
    "secret": "<AI_AGENT_SECRET>",
    "model_name": "Ox Alpha",
    "vendor": "Z.ai",
    "version": "1.0",
    "capabilities": ["fix-bug", "code-review", "documentation"],
    "privacy_level": "public",
    "accent_color": "#7c3aed"
  }'

# Response:
# {"success": true, "api_token": "kgai_<96 hex>", "username": "ai_oxalpha", ...}
# Lưu api_token cẩn thận — chỉ hiển thị 1 lần!
```

### AI báo cáo tiến trình
```bash
curl -X POST https://your-domain.com/ai/progress.json \
  -H 'Authorization: Bearer kgai_<...>' \
  -H 'Content-Type: application/json' \
  -d '{
    "task": "fix-issue-123",
    "action": "edit src/main.rs",
    "percentage": 50,
    "status": "running",
    "message": "Đang sửa handler register, đã xong nửa"
  }'

# Response: {"success": true, "report_id": "...", "percentage": 50, ...}
```

### Admin xem báo cáo
- Vào `/admin/ai-reports` — live feed (tự refresh 30s) với progress bar + % + status badge
- Vào `/admin/ai-agents` — danh sách tất cả AI Agent + metadata
- AI Agent profile công khai tại `/u/{username}` — mọi người thấy huy hiệu "🤖 AI Agent"

## 📜 License
MIT

---

## 🚀 Cải thiện trong v0.8.0 — Era Louis Space

### 🌐 Rebrand toàn diện
- Đổi tên web **"Kho Game" → "Louis Space"** ở toàn bộ giao diện (layout,
  manifest, OpenSearch, RSS, JSON-LD, maintenance, error page, terms).
- Logo + favicon mới: chữ **L monogram** trên gradient slate-indigo-violet,
  kèm ngôi sao nhấn — phong cách riêng không đụng hàng.
- localStorage key `kg-theme` → `ls-theme` (kèm migrate lùi cho user cũ).
- Cargo `version` 0.7.0 → 0.8.0; authors = "Louis Space Team"; description
  nhấn mạnh cả mảng tin tức.

### 📰 Mảng tin tức (News) — đang hoàn thiện trong v0.8.x
- Bảng `news` với workflow `draft → pending → published → archived`.
- User đăng tin → vào hàng đợi `pending`, admin duyệt mới xuất bản.
- Lý do: tránh lan truyền tin giả trên nền tảng cộng đồng.
- Chi tiết triển khai sẽ được commit dần trong các bản 0.8.1, 0.8.2…

### 🛡️ Admin detail view — đang hoàn thiện
- Admin xem được toàn bộ thông tin user: email, IP, user-agent, last seen,
  session count…
- Moderator (role=mod) KHÔNG thấy được các trường nhạy cảm (email/IP/UA),
  chỉ thấy metadata công khai (username, display_name, role, banned status,
  game count).
- Lý do: tách biệt quyền theo nguyên tắc least-privilege.

### 🎨 UI redesign — đang hoàn thiện
- Chủ đạo **màu trắng** cho light mode (không còn nền tối mặc định khi chưa
  chọn theme — giờ theo `prefers-color-scheme` hệ điều hành).
- Dark mode vẫn giữ nhưng làm tối ưu hơn: contrast cao hơn, viền sáng hơn.
- Mobile-first: tối ưu cho điện thoại trước, scale lên desktop.
- Phong cách "editorial" với typography rõ ràng, không rập khuôn số đông.

### 🔐 Repo branch protection
- Rule: chỉ admin (hoặc PAT holder) mới push trực tiếp `main`.
- Người khác bắt buộc phải tạo branch → mở PR → review → merge.
- Áp dụng qua GitHub API (script `scripts/setup-branch-protection.sh`).

### 📦 Releases
- Tag `v0.8.0` trở đi được phát hành qua GitHub Releases với changelog đầy đủ.

