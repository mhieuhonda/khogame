# Changelog

Mọi thay đổi đáng chú ý của dự án **Kho Game** được ghi lại tại đây.
Định dạng dựa trên [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
tuân thủ [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
