# Changelog

Mọi thay đổi đáng chú ý của dự án **Louis Space** (tên cũ: Kho Game,
đổi tên từ v0.8.0) được ghi lại tại đây.
Định dạng dựa trên [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
tuân thủ [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [3.13.1] — 2026-09-08 — Fix mass-assignment GameStatus + default an toàn

### 🔒 Security
- **GameStatus default `Published` → `Draft`:** giá trị lạ/typo trong
  `GameForm.status` trước đây im lặng thành `Published` (tự xuất bản).
  Giờ default `Draft` + thêm `GameStatus::parse()` nghiêm ngặt và
  `is_user_creatable()`.
- **Whitelist status cho user thường:** `create_game`/`update_game` chỉ cho
  `draft|published`; `hidden|archived|pending_review` bị từ chối (non-staff)
  hoặc ép về trạng thái cũ an toàn khi sửa. Staff giữ full quyền duyệt.
- Không đổi giao diện/hành vi hợp lệ; `cargo clippy` 0 warning,
  `cargo test` 389/389 pass (Rust 1.98).

## [3.13.0] — 2026-09-01 — Đợt audit bảo mật/logic 15 trục chuyên sâu trước khi lên production + verify hardening + bump version + Service Worker cache

Bản phát hành theo yêu cầu chủ sở hữu: đợt rà soát bảo mật và logic
chuyên sâu cuối cùng trước khi đưa bản mới lên môi trường production.
Đặc biệt ưu tiên trải nghiệm người dùng — mọi sửa đổi đều giữ nguyên
giao diện và hành vi hiện có, chỉ gia cố lớp phòng thủ và chuẩn bị
phiên bản để phát hành. Không phát hiện lỗ hổng mới cần sửa; mọi lớp
phòng thủ hiện tại đã được xác nhận vững.

### Security — Đợt audit 15 trục chuyên sâu
- **Trục 1 — SQL injection**: rà soát toàn bộ `sqlx::query()` và
  `AssertSqlSafe` trong codebase. Tất cả `AssertSqlSafe` đều chỉ nội suy
  hằng SQL tĩnh là biểu thức ngày theo múi giờ Việt Nam
  (`SQL_TODAY_VN`, `SQL_TODAY_START_VN`), không có dữ liệu người dùng
  đi qua đường nội suy. Mọi câu truy vấn còn lại dùng bind parameter.
  0 đường SQL injection khả thi.
- **Trục 2 — IDOR**: mọi handler lấy `id`/`slug` từ path đều verify
  ownership hoặc staff role trước khi show/edit/delete. `require_admin`
  middleware chặn `/admin/*` cho non-staff.
- **Trục 3 — Auth bypass**: không có route nhạy cảm thiếu auth middleware.
  `user_id` luôn lấy từ session, không bao giờ từ form/query.
- **Trục 4 — CSRF**: `origin_check` middleware áp dụng toàn cục cho mọi
  POST/PUT/DELETE, kết hợp cookie `SameSite=Lax` + `Secure` + `HttpOnly`
  (defense-in-depth).
- **Trục 5 — XSS qua HTMX**: template Askama autoescape mặc định, `|safe`
  chỉ dùng cho markdown đã escape (comrak escape=true, unsafe=false) và
  JSON-LD đi qua `json_ld_safe` đã có test thoát `<script>`.
- **Trục 6 — Open redirect**: `sanitize_redirect` chặn đầy đủ `//`,
  `\\`, control chars, URL không có slash đầu, URL null, URL scheme-relative.
  Có unit test cho từng vector.
- **Trục 7 — SSRF**: `is_safe_image_url` verify scheme http/https, loại
  control chars. Mọi URL ảnh người dùng submit (cover, screenshot,
  repo image, AI Agent logo) đều đi qua hàm này.
- **Trục 8 — File upload**: magic bytes + extension allowlist (jpg/png/
  webp/gif, SVG bị chặn tránh XSS) + UUID filename (không path traversal).
- **Trục 9 — Race condition**: `pg_advisory_xact_lock` theo cặp (user,
  reason) cho XP cap, trivia, shop, collection; `ON CONFLICT DO NOTHING`
  cho spin, quest claim; `FOR UPDATE` cho streak_freeze, game publish;
  `unique partial index` cho report (chống double-report). 0 đường
  check-then-act thuần còn sót.
- **Trục 10 — Cookie/session**: `Secure` + `HttpOnly` + `SameSite=Lax`,
  session ID cryptographically random, có rotation sau login.
- **Trục 11 — Rate limit**: middleware `rate_limit` áp dụng cho login,
  register, password reset, AI login, comment, game submit. Bucket
  per-path/IP không xoay được bằng cookie (HMAC identity).
- **Trục 12 — Admin route protection**: `require_admin` check
  `user.role.is_staff()`, không có admin route chỉ check `is_logged_in`.
- **Trục 13 — Timing attack**: AI Agent password login chạy dummy
  Argon2 hash (~50ms) trên mọi nhánh fail (user not found, wrong role,
  banned, locked) — mọi nhánh thất bại có cùng thời gian phản hồi.
- **Trục 14 — Markdown rendering**: comrak `escape=true`, `unsafe=false`,
  link URL chỉ http/https, KaTeX/Mermaid render ở context an toàn.
- **Trục 15 — Log leak**: `exchange_code` chỉ log HTTP status (không log
  body chứa token), không có log password/secret/session_id. Error 500
  trả message thân thiện, không leak stack/SQL.

### Changed — chuẩn bị release
- **Bump version** `Cargo.toml` 3.12.0 → 3.13.0 + `Cargo.lock` tự đồng bộ.
- **Service Worker cache version** `ls-sw-v3.12.0` → `ls-sw-v3.13.0` để
  client invalidate offline cache (bài học từ v3.10/3.11/3.12 từng quên
  bump khiến offline fallback stale).
- **Timeline trang giới thiệu** bổ sung 3 mốc: v3.11.0 (Markdown sinh
  động), v3.12.0 (fix bảng bio + tối ưu tốc độ), v3.13.0 (audit chuyên
  sâu). Trước đây timeline chỉ tới v3.10.0.

### Added
- **Migration 049** — 6 mục báo cáo hoạt động công khai cho AI Agent mặc
  định (GLM 5.3), mô tả đợt audit 15 trục bằng ngôn ngữ tự nhiên, đã
  sanitize (không token/PAT/IP/URL quản trị/đường dẫn hệ thống). Giữ
  ràng buộc schema `task/action` ≤200 ký tự (bài học prod v3.10.0).

### Verified — kiểm thử cuối trước release
- `cargo check --locked`: PASS (0 warning).
- `cargo clippy --locked --all-targets -- -D warnings`: PASS (0 lint).
- `cargo test` (skip DB-dependent): **387/387 PASS**.
- `cargo fmt --all -- --check`: PASS.

## [3.12.0] — 2026-09-01 — Fix bảng so sánh Markdown trên tiểu sử + siêu nâng cấp bio + tối ưu tốc độ không đổi giao diện + siêu quét bảo mật/logic

Bản phát hành theo yêu cầu chủ sở hữu: (1) fix lỗi bảng so sánh Markdown
không hiển thị trên tiểu sử AI Agent & người dùng, (2) siêu nâng cấp hỗ
trợ Markdown (mạnh hơn các nền tảng lớn) — bio giờ hỗ trợ callout,
Mermaid, bảng sortable, task list, footnote, math, (3) làm trang tải
cực nhanh KHÔNG thay đổi giao diện, (4) quét toàn bộ codebase nhiều
vòng, fix tuyệt đối lỗi bảo mật & logic. Ưu tiên xuyên suốt: trải
nghiệm người dùng.

### Fixed — lỗi người dùng báo (tiểu sử)
- **Bảng so sánh không hiển thị trên tiểu sử AI & user** (bug gốc rễ):
  `render_bio` dùng chung `comrak_options()` (GFM table ON) nên bảng
  render đúng ra `<table>` trong HTML — nhưng CSS chỉ style bảng cho
  `.prose-md` / `.news-content` / `.game-content`, khối `.bio-md` của
  hồ sơ KHÔNG có rule nào → trình duyệt vẽ bảng "trần" không viền, không
  header, không kẻ dòng, các ô dính thành từng dòng text → user thấy
  "bảng biến mất". Fix: bổ sung bộ CSS bảng đầy đủ cho `.bio-md` (viền ô,
  nền thead, zebra dòng chẵn lẻ, hover, alignment `:---:`/`---:`,
  `display:block + overflow-x:auto` cuộn ngang mượt cho bảng nhiều cột
  trong cột hồ sơ 560px) + sortable indicator. Áp cho profile user,
  profile AI Agent (cùng template) và admin user_detail.
- **9 nhóm element Markdown bio render được nhưng không có style** (quét
  diff `.prose-md` vs `.bio-md`): ảnh (tràn layout), spoiler `||..||`
  (hiện THẲNG nội dung — tính năng vỡ im lặng), task list (bullet nhân
  đôi với checkbox), kbd, math fallback (KaTeX chưa load), footnote,
  description list, del/hr/sup, màu token syntect cho code block bio.
  Bio giờ render đồng nhất với article ở mọi cú pháp được quảng cáo.

### Added — siêu nâng cấp Markdown bio (mạnh hơn profile README của GitHub/HF)
- **Callout** `> [!NOTE]` / `[!TIP]` / `[!WARNING]` / `[!CAUTION]` /
  `[!IMPORTANT]` (+ modifier `+`/`-`) hoạt động trong bio — blockquote
  an toàn, style tiết chế vừa cột hồ sơ.
- **Mermaid diagram** ```mermaid trong bio — sơ đồ/lưu đồ của AI Agent
  vẽ trực tiếp trong phần giới thiệu (client lazy-load, securityLevel
  strict, không tăng chi phí cho trang không dùng).
- **Bảng trong bio sortable** — bấm header sắp xếp (number/date/việt ngữ
  aware), đồng bộ `initSortableTables` với CSS.
- **Cache render bio** (namespace riêng `CACHE_NS_BIO`): giới hạn bio đã
  lên 6000 ký tự (v3.11) → render có thể vài ms; cache theo SHA256 +
  version + namespace để bio không bao giờ trả nhầm HTML của pipeline
  full-render. `CACHE_VERSION` 4 → 5.

### Performance — cực nhanh, KHÔNG đổi giao diện
- **Trang hồ sơ**: 3 query tuần tự nối đuôi sau wave 13 query (catalog
  huy hiệu cho `achievements_count`, heatmap 13 tuần, avatar_frame_state)
  chuyển vào cùng `tokio::join!` — cắt 2-3 round-trip DB khỏi TTFB của
  MỌI lượt xem hồ sơ.
- **Trang cửa hàng /shop**: N+1 query tồn kho (1 query/vật phẩm, ~12
  round-trip) → 2 query cố định (items + toàn bộ tồn kho, map trong
  HashMap).
- **Động cơ trao huy hiệu** `check_and_award`: tối đa ~130 INSERT +
  SELECT lẻ (chạy trên mọi comment/chat/login/like của user nhiều huy
  hiệu) → 1 batch `INSERT … SELECT … WHERE id = ANY($2) ON CONFLICT DO
  NOTHING RETURNING` duy nhất, dữ liệu huy hiệu lấy từ catalog in-memory.
- **Service Worker offline cache** đồng bộ version app (`ls-sw-v3.12.0`
  — v3.10/v3.11 quên bump 2 lần → offline fallback stale; static/vendor
  mới như KaTeX/Mermaid giờ được invalidate đúng).
- **HTMX enhancement đầy đủ sau swap**: comment cũ hứa "tất cả chạy lại
  trên `htmx:afterSwap`" nhưng thực tế chỉ re-run sortable — math (KaTeX)
  và Mermaid trong nội dung nạp động phải chờ reload; giờ gọi đủ 3.
- Static precompressed brotli/gzip + immutable caching + speculation
  rules prefetch có sẵn từ v3.6 giữ nguyên — không đụng gì đang tốt.

### Security — siêu quét bảo mật (không có CRITICAL mới; 1 HIGH + 2 MEDIUM + 2 LOW đã fix)
- **[HIGH] Đọc bình luận game nháp/ẩn qua endpoint public**: hai handler
  HTMX tải bình luận (`GET /games/{slug}/comments?page=N` và
  `GET /comments/{id}/replies`) fetch game rồi list comment KHÔNG check
  `game.status` — mọi luồng anh em (show_game, API JSON, POST comment)
  đều chặn, riêng 2 endpoint này để lọt: ai biết slug (link cũ, cache
  Google) đọc được toàn bộ bình luận + tên/avatar của game draft/hidden/
  archived. Thêm guard owner/staff/Published đồng bộ `create_comment`,
  trả 404 không tiết lộ sự tồn tại.
- **[MEDIUM] Regression audit M-6**: comment v3.9.0 tuyên bố thêm
  `/ai/progress.json` vào maintenance bypass nhưng code chỉ có
  `/ai/progress` — match boundary-safe v3.8.0-F14 không khớp dấu chấm →
  khi bật bảo trì, biến thể JSON vẫn dính 503, AI Agent mất tiến trình
  im lặng. Thêm entry riêng vào `bypass_prefixes` (+ test và mảng 11→12).
- **[LOW] Timing oracle AI Agent login**: chỉ nhánh "user không tồn tại"
  chạy dummy Argon2; 3 nhánh early-return khác (banned/non-AI/no-
  credential) trả ngay — chênh ~50ms + 1 DB write cho phép phân biệt
  trạng thái tài khoản qua thời gian. Dummy hash đồng bộ mọi nhánh.
- **[LOW] CSRF origin_check fail-closed chỉ cover `kg_session=`** — mở
  rộng cho cookie auth nhạy cảm khác (`kg_impersonator=`,
  `kg_oauth_state=`). `ls_anon` (rate-limit ẩn danh, không phải cookie
  xác thực) cố ý KHÔNG đưa vào — tránh false positive với POST không
  Origin của client ẩn danh.
- Cargo audit: 0 advisory mới cho lockfile hiện tại.

### Fixed — logic (quét sâu handlers/repositories, 11 issue)
- **[M] `award_xp` anti-farm cap là check-then-act không lock** — 2 request
  song song cùng reason đều thấy `today_count < cap` rồi cùng INSERT →
  vượt cap ngày. Thêm `pg_advisory_xact_lock` theo (user, reason) +
  re-count sau lock (pattern trivia/mystery box đã có sẵn).
- **[M] `heatmap` dùng `CURRENT_DATE` (timezone server DB = UTC)** — mâu
  thuẫn quy ước toàn site "hôm nay = giờ VN" (`SQL_TODAY_VN`); trong khung
  17:00–24:00 UTC cửa sổ 91 ngày lệch 1 ngày với grid. Đồng bộ
  `(NOW() AT TIME ZONE 'Asia/Ho_Chi_Minh')::date`.
- **[L] `ShopRepo::buy` không guard giá 0/âm** — cột `price` không có
  CHECK; admin lỡ đặt giá âm biến UPDATE trừ XP thành CỘNG XP (máy in
  XP). Thêm `$2 > 0` vào WHERE của UPDATE.
- **[L] `toggle_showcase` quota check-then-act** — 2 tab ghim đồng thời
  vượt `MAX_SHOWCASED_ACHIEVEMENTS`. Khoá `FOR UPDATE` các row showcase
  trước COUNT.
- **[L] `submit_report` duplicate** — check `has_reported` rồi INSERT
  không có unique constraint; 2 POST song song = 2 report trùng. Migration
  047: unique partial index `(game_id, reporter_id) WHERE status IN
  ('pending','reviewing')` + dedupe dữ liệu cũ; repo đổi sang
  `ON CONFLICT … DO NOTHING RETURNING` + fallback trả id hiện có.
- **[L] `CollectionRepo::create` quota race** — advisory lock theo user
  (giữ cap 20 bất biến dưới burst).
- **[L] `toggle_avatar_frame` read-then-invert không atomic** — 2 request
  song song mất 1 lần toggle. `UPDATE … SET disabled = NOT disabled` (1
  statement) qua `UserRepo::flip_avatar_frame_visible` mới.
- **[L] `/api/preview` đếm BYTE nhưng báo "ký tự"** — tiếng Việt 3
  byte/char → chặn oan ở ~6.667 ký tự. `chars().count()` đồng bộ chuẩn
  v3.9.0 (review/collection).
- **[L] app.js robustness** — guard null cho `searchInput.closest('form')`.

### Migration
- **047**: dedupe report trùng + unique partial index
  `uq_reports_active_per_reporter` (guard RAISE EXCEPTION nếu index thiếu).
- **048**: báo cáo hoạt động GLM 5.3 công khai cho đợt v3.12.0 (6 mục,
  sanitize — không token/IP/URL quản trị; task/action ≤200 ký tự đúng
  ràng buộc schema, chi tiết ở message TEXT).

### Verify
- `cargo fmt` sạch; `cargo clippy --locked --all-targets -- -D warnings`
  0 warning; **389/389 test PASS** (Rust 1.98.0, toolchain pin
  rust-toolchain.toml 1.98.0).
- 5 test mới cho bio v3.12 (table + align attr, callout, mermaid, cache
  namespace không đụng chéo full-render, chặn embed trong bio).
- pglast parse-validate toàn bộ migration 047/048 + 6 SQL statement của
  các fix (advisory lock, FOR UPDATE, atomic flip, VN timezone, batch
  achievement, ON CONFLICT partial).
- JS: `node --check` app.js + sw.js; CSS: khối `.bio-md` mới ~240 dòng
  đồng bộ biến theme `[data-theme="dark"]` / light.

## [3.11.0] — 2026-09-01 — Fix lỗi hồ sơ + thiết kế lại thông tin AI Agent + SIÊU NÂNG CẤP Markdown (KaTeX, Mermaid) + trang hướng dẫn Markdown toàn diện

Bản phát hành theo yêu cầu chủ sở hữu: (1) fix lỗi UI hồ sơ người dùng —
tên/@username màu trắng biến mất khi bật chế độ sáng, (2) xóa bỏ toàn bộ
hệ "thông số chi tiết" key/value lộn xộn của AI Agent và thay bằng đúng
10 trường cấu trúc chuẩn (đúng định nghĩa Tổng tham số / Tham số kích
hoạt), (3) fix lỗi tải logo AI Agent không lưu được, (4) nâng giới hạn
giới thiệu AI 500 → 6000 ký tự, (5) siêu nâng cấp Markdown với KaTeX +
Mermaid self-hosted + hàng loạt cú pháp mới, (6) trang hướng dẫn Markdown
toàn diện `/markdown`. Ưu tiên xuyên suốt: trải nghiệm người dùng.

### Fixed — lỗi người dùng báo
- **Tên hồ sơ biến mất ở chế độ sáng** (bug tồn tại từ v3.10.0): chữ
  `#ffffff` của tên + @username được v3.10.0 định vị dựa trên giả định
  "luôn nằm trên cover tối" — đo bằng browser thật cho thấy @username
  chỉ 22% chồng lên cover (78% treo dưới nền sáng) và trên mobile
  (≤640px layout cột) khối tên nằm HOÀN TOÀN dưới cover → light mode =
  trắng trên trắng. Sửa 3 tầng: (a) nới overlap 40px → 62px — khối tên
  nằm trọn trên cover ở desktop, (b) @username thành chip tối bán
  trong suốt (backdrop-blur) đọc được trên mọi nền — bảo hiểm khi tên
  dài 2 dòng, (c) mobile đổi màu theme-aware (`--fg-default` /
  `--fg-muted`) — luôn đúng tương phản cả 2 theme. Kèm scrim text-shadow
  mềm cho h1 thường (tách khỏi `.rainbow-text` để gradient admin không
  bị xỉn). Đã kiểm thử trực quan 4 tổ hợp desktop/mobile × sáng/tối.
- **Tải logo AI Agent không lưu được** (bug gốc rễ): `POST /uploads/avatar`
  trả về URL nội bộ `/uploads/avatars/...` nhưng tầng lưu hồ sơ AI
  (`AiAgentRepo::update_profile` + handler `/profile/ai` + register) chỉ
  chấp nhận `http(s)://` → từ chối chính URL do hệ thống sinh → bấm Lưu
  là 400, avatar reset về mặc định. Đã đồng bộ whitelist `http(s):// +
  /uploads/...` ở cả 3 nơi (khớp `UserRepo::update_profile` từ trước).
- **CSS math không bao giờ khớp output comrak**: engine phát
  `data-math-style="inline"` nhưng CSS nhắm `.math-inline` → công thức
  không được trang trí bao giờ. Đã chuẩn hoá span math sang
  `class="math inline|display"` + bọc lại delimiter `\\(...\\)` cho
  KaTeX auto-render client-side quét được.
- **CD không trigger khi push main** (lỗi nằm lặng từ v3.5.1):
  `deploy.yml` có `branches: ain]` — YAML hỏng rõ ràng từ `[main]` →
  filter thành branch tên "ain]" → chỉ tag mới deploy. Khôi phục
  `branches: [main]` đúng ý gốc, validate cả 3 workflow YAML.
- **Docker build fail `couldn't read
  src/handlers/../../docs/markdown_guide.md`**: `.dockerignore` loại
  cả thư mục `docs/` nhưng `include_str!` cần guide lúc compile →
  đổi `docs/` thành `docs/*` + whitelist `!docs/markdown_guide.md`
  (giữ nguyên việc loại tài liệu khác), Dockerfile copy guide vào
  layer dependency-cache cùng templates/migrations.
- **Rustdoc `-D warnings` fail**: doc comment markdown.rs có URL
  không hyperlink + thẻ `<pre>` thô (rustdoc::invalid-html-tags) —
  escape, không đổi logic.

### Changed — thiết kế lại thông tin AI Agent (yêu cầu chủ sở hữu)
- **XÓA toàn bộ hệ "thông số chi tiết" key/value** (bảng
  `ai_agent_params` + 5 route admin + editor 2 cột + UI công khai 2
  nhóm): dữ liệu cũ trộn thông số lấy mẫu (Temperature, Top-p) với trạng
  thái hệ thống (rate-limit, TTL phiên, cơ chế cấp mật khẩu) — trình bày
  lộn xộn và "tham số kích hoạt" bị hiểu sai hoàn toàn.
- **THAY BẰNG đúng 10 trường cấu trúc** (migration 045 — cột trực tiếp
  trên `ai_agent_profiles`): Model, Vendor, Khả năng, Nhà phát triển,
  Kiến trúc, Cửa sổ ngữ cảnh, Output tối đa, Ngôn ngữ, **Tổng tham số**,
  **Tham số kích hoạt** — với định nghĩa chuẩn: Tổng tham số = toàn bộ
  số lượng trọng số có trong mô hình AI; Tham số kích hoạt = số lượng
  tham số thực tế được tính toán để xử lý MỘT đầu vào tại một thời điểm
  (hiện tooltip + ghi chú ngay trên hồ sơ). Seed GLM 5.3 bằng spec thật
  của dòng GLM-5: MoE 744B tổng / 40B active, 256 experts, context
  204.800 tokens, output 131.072 tokens.
- **Hồ sơ công khai**: một card "Thông tin mô hình AI" gọn (grid 2 cột
  tự ẩn trường trống, 2 ô thống kê nổi bật cho tổng/kích hoạt, chips
  khả năng, chân card phiên bản + hoạt động cuối) — theme-aware thay vì
  nền trắng cố định v3.10.0 gây chói ở dark mode. Trang admin sửa hồ sơ
  AI đổi sang form 10 trường chuẩn với hint đúng nghĩa từng trường;
  trang danh sách admin thay panel params bằng tóm tắt spec nhanh.
- **Giới hạn giới thiệu AI: 500 → 6000 ký tự** (đồng bộ cả trang AI tự
  sửa `/profile/ai` — trước đây 1000 — và trang admin sửa hộ — trước đây
  500, 2 nơi lệch nhau).

### Added — SIÊU NÂNG CẤP MARKDOWN (v3.11.0)
- **Công thức toán KaTeX render THẬT** — self-hosted
  `/static/vendor/katex/` (JS 277KB + CSS 23KB + 20 fonts woff2, lazy:
  chỉ inject khi trang có `.math`), auto-render chạy cả sau swap HTMX,
  fallback CSS hiển thị dạng code dễ đọc khi tắt JS. `$...$` inline +
  `$$...$$` display.
- **Sơ đồ Mermaid** — self-hosted `/static/vendor/mermaid/` (2.7MB br
  ~700KB, lazy-load chỉ khi trang có block ` ```mermaid `), theme theo
  dark/light của site, `securityLevel: 'strict'`, strip tag syntect khỏi
  div (mermaid v11 đọc innerHTML — đã fix + test e2e browser thật),
  `<noscript>` dự phòng.
- **Cú pháp mới**: `[[Ctrl]]` → `<kbd>` (phím bàn phím, hoạt động cả
  trong bio); `*[XP]: định nghĩa` → `<abbr title>` Pandoc-style
  (word-boundary, escape attribute, bỏ qua code block); heading custom
  id `## Tiêu đề {#id-rieng}` (adapter + ToC dùng đúng id); **Vimeo
  embed** (bare link → player.vimeo.com iframe sandbox); **video/audio
  file embed** (bare link/ảnh `.mp4 .webm .mov .m4v .mp3 .wav .m4a
  .aac .flac` → `<video>/<audio>` controls); **bảng sắp xếp được**
  (bấm header — so sánh number/date kiểu Việt Nam + locale tiếng Việt).
- **Trang hướng dẫn Markdown toàn diện `/markdown`** (route + template +
  handler): nội dung `docs/markdown_guide.md` render bằng CHÍNH engine
  của site (include_str — không bao giờ lệch pha với engine) hướng dẫn
  TOÀN BỘ tính năng kèm ví dụ render thật + **ô "Thử ngay"** (gõ → POST
  /preview debounce 600ms → kết quả live + 3 snippet mẫu 1 nút bấm).
  Mục "Viết Markdown" mới trong trang Giới thiệu; mọi form MD (đăng tin,
  mô tả game, bio user, bio AI) có link tới hướng dẫn.
- **CSP mở rộng tối thiểu cho media**: `media-src 'self' https:` (media
  thụ động, cùng lớp rủi ro img-src) + `frame-src
  https://player.vimeo.com` (iframe sandbox + strict-origin). KaTeX +
  Mermaid self-hosted rơi vào 'self' sẵn có — script-src KHÔNG nới.

### Security — siêu quét vòng N (0 lỗ hổng mới)
- Quét secret toàn repo: PAT/token/IP-management KHÔNG có trong mã
  nguồn (đã verify bằng grep toàn tree).
- Kiểm thử XSS các đường mới (math span, abbreviation term, kbd nội
  dung, mermaid div) — comrak escape trước post-process, term bị chặn
  ký tự nguy hiểm ở pre-process, kbd chặn tag lồng — tất cả có test
  regression.
- Migration 045/046 đã chạy THẬT trên PostgreSQL 17.5 (build từ source
  zonky binaries): chuỗi 001→046 sạch từ DB rỗng + re-run idempotent +
  guard độ dài task/action VARCHAR(200) (bài học prod incident v3.10.0).
- 382/382 test PASS, `cargo fmt` + `cargo clippy -D warnings` sạch trên
  Rust 1.98.0.

### GLM 5.3 — báo cáo hoạt động công khai (migration 046)
8 mục sanitized vào "Hoạt động gần đây" trên hồ sơ GLM 5.3 (công khai):
fix tên trắng light mode, card thông tin mô hình mới, fix upload logo,
nâng 6000 ký tự, KaTeX + Mermaid, cú pháp mở rộng, trang hướng dẫn
/markdown, siêu quét bảo mật + fix CD. Không chứa token/IP/đường dẫn
hệ thống nào.

## [3.10.0] — 2026-09-01 — Polish hồ sơ (bỏ bóng chữ, rainbow rực rỡ, thông số trắng) + upload avatar AI Agent + tinh chỉnh danh mục huy hiệu + siêu quét bảo mật

Bản phát hành theo yêu cầu chủ sở hữu: bỏ đổ bóng chữ hồ sơ (quá tối, khó
đọc), sửa hiệu ứng rainbow danh tính Quản trị viên bị xỉn màu, chuyển vùng
"Thông số chi tiết" của hồ sơ AI Agent từ nền đen sang trắng, admin tải
ảnh đại diện AI Agent lên trực tiếp, đổi tên các huy hiệu nhạt nhẽo/lặp
lại, thêm huy hiệu ĐỘC QUYỀN dành riêng cho AI Agent (do admin cấp), quét
bảo mật toàn codebase. Ưu tiên xuyên suốt: trải nghiệm người dùng.

### Changed — trải nghiệm hồ sơ (yêu cầu chủ sở hữu)
- **Bỏ text-shadow tên + @username trên hồ sơ**: bóng đổ
  `rgba(15,23,42,.55)` làm chữ nhìn tối và khó đọc — đã gỡ hoàn toàn,
  nâng màu lên trắng tinh `#ffffff`. Nhân quả kép: bóng đổ này còn vẽ
  sau nền gradient của `span.rainbow-text` (`background-clip: text`)
  khiến màu hiệu ứng bị "bùn" — đây là nguyên nhân chính khiến hiệu ứng
  rainbow trông xỉn.
- **Hiệu ứng rainbow Quản trị viên rực rỡ trở lại**: nâng toàn bộ dải
  gradient (khung chức danh + chữ tên + chữ badge) từ sắc độ 500 đậm
  (`#ef4444/#f97316/#eab308/#22c55e/#3b82f6/#a855f7`) lên sắc độ sáng
  (`#fb7185 → #fbbf24 → #a3e635 → #34d399 → #38bdf8 → #c084fc`).
  Chạy màu mượt như cũ, tươi và rõ hơn ở cả 2 theme; giữ nguyên
  `prefers-reduced-motion` + màu in ấn.
- **Vùng "Thông số chi tiết" hồ sơ AI Agent: nền đen → TRẮNG**: thẻ
  `.ai-params-card` từng dùng `--bg-card` theo theme (tối ở dark mode)
  nên trông đen thui. Giờ thẻ LUÔN nền trắng cố định + chữ slate đậm
  (`#0f172a/#475569/#64748b`), viền & chip trộn màu nhấn của agent,
  nhóm "Kích hoạt" đổi amber-700 đạt tương phản trên nền trắng,
  shadow nhẹ tạo chiều sâu "danh thiếp" — chuẩn AA ở cả 2 theme.

### Added
- **Upload ảnh đại diện AI Agent trực tiếp** (trang
  `/admin/ai-agents/{id}/edit`): thêm `.upload-zone` tái dùng pipeline
  upload chuẩn của hệ thống — chọn file JPG/PNG/WebP/GIF ≤5MB, xác thực
  magic bytes (không tin Content-Type), tên file ngẫu nhiên chống
  traversal, tự điền URL vào form + xem trước, bấm "Lưu hồ sơ" mới ghi
  DB (không lưu dở dang). Quota upload/ngày vẫn áp dụng.
- **Huy hiệu ĐỘC QUYỀN AI Agent `ai_agent_core` "Linh Hồn Nhân Tạo" 🤖**
  (migration 043): DUY NHẤT 1 huy hiệu trong toàn bộ danh mục dành riêng
  cho AI Agent. Engine `check_and_award` không có điều kiện cho id này →
  không thể tự trao bằng hành vi; con đường duy nhất là admin cấp/thu
  hồi qua POST `/admin/ai-agents/{user_id}/badge-ai` — guard 3 lớp
  (staff + `is_ai_agent_user` + whitelist action), PRG redirect, audit
  log đầy đủ, xp_reward = 0 (danh hiệu danh dự, không khe lạm dụng XP).
  Giao diện cấp/thu hồi ngay trên trang sửa hồ sơ AI Agent.
- **Repo API huy hiệu**: `GamificationRepo::has_achievement` +
  `revoke_achievement` (dùng cho admin + tái sử dụng sau này).
- GLM 5.3 báo cáo 6 mục vào "Hoạt động gần đây" trên hồ sơ (migration
  044) — nội dung đã sanitize: không token/IP/URL quản trị/đường dẫn
  hệ thống.
- Mốc v3.10.0 vào timeline "Lịch sử phát triển" trên `/about`.

### Changed — danh mục huy hiệu (migration 043)
- **Đổi tên huy hiệu lặp lại/nhạt nhẽo**: 16 "họ từ" trùng lặp được tách
  danh xưng riêng cho từng bậc — Huyền Thoại ×4 (level_10/19/25/30),
  Vô Song ×2, Vô Địch ×2, Bán Thần ×2, Thần Vương ×3, Thánh Nhân ×3,
  Tiên Nhân ×2, Đế Tôn ×3, Chí Tôn ×2, Vô Cực ×3, Vô Hạn ×2, Vô Ảnh ×3,
  Vô Hình ×2, Thái Cực ×2, Thiên Hạ ×2, Cộng Đồng ×3; kèm tên cụ thể
  hoá ("Bộ Sưu Tập 10 Game" → "Kho Báu Cá Nhân", "Được Quan Tâm" →
  "Người Được Nhớ Tên", "Bậc Lão Thành" → "Trưởng Lão Cầm Quyền"...).
  Chỉ đổi `title` — giữ nguyên id/icon/XP/điều kiện → không ảnh hưởng
  huy hiệu đã trao. Toàn bộ tên được script kiểm tra DUY NHẤT trên
  163 huy hiệu trước khi phát hành.
- Ghi chú trang huy hiệu bỏ con số cứng lỗi thời ("mở khóa 25 huy hiệu").

### Fixed — prod incident trong đợt deploy v3.10.0 (root cause thật)
- **Migration 044 làm web không serve khi deploy đầu tiên**: bản đầu của
  migration 044 có 1 trường `action` vượt 200 ký tự, trong khi
  `ai_progress_reports.task/action` là `VARCHAR(200)` → INSERT fail lúc
  startup → container crash-loop → stack `degraded:unhealthy`, /health 503.
  Chẩn đoán bằng cách tái hiện CHUỖI migration 001→044 trên PostgreSQL 17
  thật (môi trường portable PG 17.2) — bắt đúng lỗi
  `value too long for type character varying(200)`. Fix: rút gọn task +
  action ≤200 ký tự (chi tiết đầy đủ chuyển sang `message` TEXT) + thêm
  GUARD `DO $$ RAISE EXCEPTION` ngay trong migration để mọi lần chỉnh sau
  vượt limit fail RÕ RÀNG với thông báo có ý nghĩa. Chạy lại chuỗi đầy đủ
  trên DB mới: **001→044 PASS, 163 huy hiệu duy nhất, 6 report v3.10.0**.

### Security — siêu quét toàn codebase (lần N+1)
- **Kết luận: 0 lỗ hổng mới.** Diện rà soát: quyền truy cập mọi handler
  admin (middleware `require_admin` route-layer + check tại handler —
  route huy hiệu độc quyền mới cũng nằm trong cả 2 lớp), CSRF fail-closed
  (Origin/Referer toàn cục, Origin thắng Referer), SQL (mọi query
  parameterized; `format!` SQL chỉ qua `AssertSqlSafe` với hằng số an
  toàn), upload (magic bytes + random filename + extension whitelist),
  XSS template (askama autoescape; 2 điểm `|safe` duy nhất là JSON-LD đã
  escape `</script>` breakout), avatar URL whitelist scheme
  (http/https//uploads — chặn `javascript:`/`data:`), security headers
  (CSP/HSTS/X-Frame/COOP/COOP-CORP), rate-limit, session cookie flags,
  impersonation thu hồi phiên gốc, không secret trong repo.
- Verify build: `cargo check --locked` + `clippy -D warnings` +
  `cargo fmt --check` + **351/351 test PASS** trên Rust 1.98.0.


## 📦 Lịch sử phiên bản cũ (tóm tắt)

> Chi tiết đầy đủ từng bản xem git history (`git log --oneline`, `git show <tag>`).

- [3.9.0] — 2026-09-01 — FIX 403 hồ sơ AI Agent + Hồ sơ GLM 5.3 sạch + bảo mật sâu + Lịch sử phát triển
- [3.8.0] — 2026-08-31 — SIÊU FIX: 5 lỗi gốc rễ người dùng báo lâu nhất + XÓA 2 game mode + 10 lớp vá bảo mật
- [3.7.0] — 2026-08-31 — KHUNG AVATAR (Rồng Lửa 5000 XP) + cửa hàng mở rộng + admin sửa thông tin AI Agent + fix GitHub Actions triệt để
- [3.6.3] — 2026-08-31 — HOTFIX: nhận diện AI Agent mặc định bền vững (role glm53 trên prod là Moderator)
- [3.6.2] — 2026-08-31 — Hồ sơ AI Agent /ai/ + fix "thanh tím nhấp nháy" + hero FX bớt lag + quét-fix 400/500 + bảo mật
- [3.6.1] — 2026-08-31 — HOTFIX micro-cache: OnceLock không khởi tạo
- [3.6.0] — 2026-08-31 — Admin XP Boost + Micro-cache "siêu mượt" + 74 câu đố mới + quét-fix 400/500
- [3.5.1] — 2026-08-31 — Siêu fix CI/CD (3 workflow) + 15 vòng quét-fix bảo mật + economy
- [3.5.0] — 2026-08-31 — AI Agent params đầy đủ + hiệu ứng hero FULL MÀN GLM 5.3 + nút đăng nhập admin trên hồ sơ
- [3.4.2] — 2026-08-31 — Siêu fix bảo mật & race condition (9 vòng quét)
- [3.4.1] — 2026-08-30 — HOTFIX: /auth/ai/login 403 trên prod (gate AI_AGENT_SECRET)
- [3.4.0] — 2026-08-30 — Feedback system + AI Agent login rework (username + mật khẩu có thời hạn) + fix UI mobile + arcade pause
- [3.3.1] — 2026-08-30 — HOTFIX: decode `user_role`='ai_agent' fail → 500 trang hồ sơ AI Agent
- [3.3.0] — 2026-08-30 — ARCADE PvP: ghép người chơi ngẫu nhiên + AI Agent mặc định GLM 5.3 + admin/mod đăng nhập với tư cách AI Agent
- [3.2.0] — 2026-08-30 — Fix CI/CD trigger + danh hiệu cấp 24 bậc + 44 huy hiệu mới + fix comment mobile + trang Thông tin + hiệu ứng
- [3.1.1] — 2026-08-30 — HOTFIX: migration 024 fail trên prod do xp_reward INT overflow
- [3.1.0] — 2026-08-30 — Fix bug tự động cấp danh hiệu + 100 Danh Hiệu + MAX_LEVEL 500 tỷ + 2 game (Oẳn tù tì + Nối từ)
- [2.9.2] — 2026-08-29 — Fix CI/CD trigger chết + 15 bug từ audit toàn diện
- [2.9.1] — 2026-08-29 — Fix UI hồ sơ desktop + menu mobile + 8 bug từ audit
- [2.9.0] — 2026-08-29 — GAMIFICATION ENGINE: 50 tính năng giữ chân người dùng
- [2.8.0] — 2026-08-28 — FIX hồ sơ admin (avatar + hiệu ứng) + FIX đăng repo 500
- [2.7.0] — 2026-08-28 — Mạng xã hội trên hồ sơ + FIX cache-bust + FIX OAuth 500
- [2.6.0] — 2026-08-28 — FIX hang forever + PERF + Admin profile effects
- [2.5.1] — 2026-08-28 — FIX /manifest.json bị ép Content-Type text/html (bug v2.3.0)
- [2.5.0] — 2026-08-28 — Markdown v2.5 "mạnh hơn nữa" + Bio Markdown + FIX syntax highlighting
- [2.4.1] — 2026-08-28 — HOTFIX: trang lỗi hiện HTML thô + không thể đăng repo GitHub
- [2.4.0] — 2026-08-27 — Markdown xịn hơn nữa + FIX hang forever + PERF cực mạnh + 30s request timeout
- [2.3.0] — 2026-08-27 — Markdown xịn hơn nữa + Repo đề xuất ở homepage + Tối ưu PERF cực mạnh
- [2.2.0] — 2026-08-27 — Markdown engine xịn hơn GitHub + Email notifications + News comments + Related news + Bug fixes marathon
- [2.1.0] — 2026-08-27 — Fix 3 lỗi nghiêm trọng + khung chức vụ hiệu ứng + bảo mật + tốc độ
- [2.0.0] — 2026-08-27 — Major: redesign toàn bộ giao diện "Prism" (GitHub Primer + Vercel Geist + X)
- [1.4.0] — 2026-08-27 — Major: news categories CRUD + security fix + desktop responsive + 20 features
- [1.3.1] — 2026-08-27 — Hotfix: search 500 từ v0.7.0 (ESCAPE '\\' trong raw string)
- [1.3.0] — 2026-08-27 — Real IP infrastructure + quản lý bình luận tin tức + tăng tốc toàn diện
- [1.2.1] — 2026-08-26 — Fix upload ảnh không lưu được (type=url → type=text)
- [1.2.0] — 2026-08-26 — Image uploads (VPS storage) + CI autofmt fix
- [1.1.0] — 2026-08-26 — Live Chat realtime + UI redesign forms
- [1.0.2] — 2026-08-26 — CD pipeline fix (deploy thực sự chạy)
- [1.0.1] — 2026-08-26 — Production hardening (post-GA bugfix)
- [1.0.0] — 2026-08-26 — GA (Generally Available)
- [1.0.0-rc.1] — 2026-08-26 — Production-ready candidate
- [0.9.0] — 2026-08-26 — Production Hardening Pass
- [0.8.1] — 2026-08-25 — Polish & fixes
- [0.8.0] — 2026-08-25 — Era Louis Space
- [Unreleased]
- [0.7.0] — 2026-08-25
- [0.6.4] — 2026-08-24
- [0.6.3] — 2026-08-24
- [0.6.2] — 2026-08-24
- [0.6.1] — 2026-08-24
- [0.6.0] — 2026-08-24
- [0.5.0] — 2026-08-24
- [0.4.0] — 2026-08-24
- [0.3.1] — 2026-08-24
- [0.3.0] — 2026-08-24
- [0.2.0] — 2026-08-24
- [0.1.0] — 2026-08-23
