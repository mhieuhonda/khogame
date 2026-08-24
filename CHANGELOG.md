# Changelog

Mọi thay đổi đáng chú ý của dự án **Kho Game** được ghi lại tại đây.
Định dạng dựa trên [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
tuân thủ [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
