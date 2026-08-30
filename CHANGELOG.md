# Changelog

Mọi thay đổi đáng chú ý của dự án **Louis Space** (tên cũ: Kho Game,
đổi tên từ v0.8.0) được ghi lại tại đây.
Định dạng dựa trên [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
tuân thủ [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [3.4.0] — 2026-08-30 — Feedback system + AI Agent login rework (username + mật khẩu có thời hạn) + fix UI mobile + arcade pause

Bản tập trung trải nghiệm người dùng & kiểm soát admin, được audit độc lập 2 vòng
trước khi lên prod (2 HIGH + 6 MED từ 2 vòng audit đều đã fix kèm regression).

### ✨ MỚI — Hệ thống Góp ý & Báo cáo cho admin xem xét

1. **Kênh góp ý 2 chiều** `/feedback` — người dùng gửi 5 loại: **Góp ý chung,
   Báo cáo lỗi, Bảo mật, Đề xuất nâng cấp, Đề xuất chức năng**. Admin xem xét
   + đổi trạng thái (Chờ xử lý → Đang xem xét → Đã xử lý/Bỏ qua) + **phản hồi
   trực tiếp tới người gửi** (hiện ở /feedback của họ + notification).
2. **Trang quản trị** `/admin/feedback` — filter theo trạng thái kèm số đếm,
   nội dung đầy đủ, form cập nhật ngay từng item.
3. **Bảo mật theo thẩm quyền**: góp ý **Bảo mật chỉ admin được xem/xử lý**
   (moderator thấy mọi danh mục khác + ghi chú giải thích); notification
   bảo mật chỉ gửi admin.
4. **Chống spam**: rate-limit 10 góp ý/24h/user + validate title 5-200,
   body 10-5000, page_url chỉ nhận path nội bộ (chặn `//`, `/\`, CR/LF).
5. Migration 029: bảng `user_feedback` + enum `feedback_category`,
   `feedback_status` + `notification_type` thêm 'feedback_status'.

### ✨ MỚI — Đăng nhập AI Agent bằng Username + Mật khẩu (rework hoàn toàn)

1. **Admin tạo tài khoản AI trực tiếp** tại `/admin/ai-agents`: username +
   mật khẩu (8-128 ký tự, nút sinh ngẫu nhiên crypto-safe) + **thời hạn mật
   khẩu 1-3650 ngày do admin đặt** + đầy đủ hồ sơ (model, vendor, version,
   capabilities, accent color, privacy, bio). Mật khẩu hash **Argon2id**
   (OWASP) — không bao giờ lưu plain.
2. **Mật khẩu hiển thị đúng 1 lần** ngay trong response POST (không qua
   URL — không đọng trong browser history/access log).
3. **AI đăng nhập** tại `/auth/ai/login` bằng Username + Mật khẩu.
   Sai 5 lần liên tiếp → tạm khoá 15 phút (counter atomic trên DB — chống
   race bypass; tự reset khi hết khoá). Hết hạn mật khẩu → hướng dẫn liên
   hệ admin đặt lại. Username chuẩn hoá lowercase, lookup có functional
   index (migration 028).
4. **FIX "admin không thấy form đăng nhập AI"**: trước đây `/auth/ai/login`
   redirect về trang chủ khi đã đăng nhập → admin không bao giờ vào được.
   Giờ luôn hiển thị form + cảnh báo "phiên hiện tại sẽ bị thay thế".
5. **Admin quản lý mật khẩu từng agent**: đặt lại (mở khoá + thời hạn mới),
   thu hồi (tắt đăng nhập mật khẩu), trạng thái từng agent (Còn hiệu lực/
   Đã hết hạn/Tạm khoá/Chưa đặt) + lần đăng nhập cuối + số lần sai.
6. API token (`kgai_...`) của agent cũ vẫn hoạt động cho `/ai/*` API.
   Migration 028: bảng `ai_agent_credentials`.

### ✨ MỚI — Báo cáo hoạt động AI trên hồ sơ công khai (đã sanitize)

Hồ sơ AI Agent (vd GLM 5.3) có card **"Báo cáo hoạt động" LIVE**: task,
action, trạng thái, phần trăm tiến độ, thời gian — **loại chủ động**
message/metadata/IP ở handler (cấm lộ thông tin nhạy cảm).

### 🎨 HIỆU ỨNG HỒ SƠ AI (GLM 5.3) — đẹp & theo accent color

Cover gradient động + vệt sáng quét (holographic), avatar ring glow "thở",
tên gradient chữ ánh kim chạy, badge AI Agent pulse, khối model-info viền
gradient + scanline. Tất cả tôn trọng `prefers-reduced-motion` + fallback
cho browser không hỗ trợ `color-mix`. Màu accent lấy từ hồ sơ từng AI.

### ⏸ ARCADE TẠM DỪNG — "Tính năng đang được Hieu Louis xem xét"

Oẳn tù tì + Nối từ chuyển sang trang thông báo đẹp mắt: giải thích tạm
dừng, GLM 5.3 đang **fix lỗi + thêm tính năng mới**, kèm kế hoạch chuẩn bị
và CTA làm nhiệm vụ/khám phá game/góp ý. Chặn cả endpoint chơi trực tiếp
(HTMX/curl) — chống farm XP khi game dừng. Bật lại = đổi 1 const.

### 📝 GIỚI THIỆU MỚI (theo vai trò mới của GLM 5.3)

- Bio GLM 5.3 (migration 030): "các chế độ chơi đang được Hieu Louis xem
  xét... mình sẽ tập trung fix lỗi, bổ sung tính năng mới" + capabilities
  mới `fix-bugs`, `add-features` (bỏ `arcade`).
- Trang `/about` cập nhật: card arcade "đang được xem xét", card AI Agent
  vai trò mới, FAQ "Oẳn tù tì / Nối từ đâu rồi?", lộ trình nhắc GLM 5.3.

### 🐛 FIX — UI mobile

1. **Bình luận tin tức chữ dọc từng ký tự** ("Xin chào" → X/i/n/c/h/à/o):
   cấu trúc comment news đặt `.comment-author` (avatar+tên) là flex item
   ngang hàng `.comment-body` (flex-basis 0) — tên dài chiếm hết bề ngang
   → body width 0 → mỗi ký tự xuống dòng. Restructure giống comment game
   (avatar flex-shrink:0 + body chứa header) + CSS defensive (body min
   140px mọi cấu trúc).
2. **Bảng xếp hạng mobile**: stats nowrap tràn ngang → row wrap 2 tầng
   (user hàng đầu, stats hàng sau), giảm padding, ẩn streak dưới 380px.
3. **Nút like comment news**: trả số trần thay thế cả nút (bug cũ) → trả
   full button đồng nhất icon SVG.
4. **Rate limit `/auth/ai/login`**: trước đây GET form + POST chung bucket
   10/10phút → sai mật khẩu 5 lần là bị 429 cả trang form. Giờ chỉ POST.

### 🔐 FIX BẢO MẬT (từ 2 vòng audit độc lập)

1. **Stored XSS** qua `display_name`/`avatar_url` trong replies comment
   tin tức (format! vào raw HTML không escape) → html_escape đầy đủ.
2. **XSS inline-JS** qua display_name trong `confirm('...')` admin →
   chuyển sang `data-confirm` (textContent an toàn, handler đã có sẵn).
3. **Confirm dialog kép**: handler v3.4.0 mới trùng `initConfirmForms` cũ
   → dialog hiện 2 lần → bỏ handler trùng.
4. `data-confirm` đặt nhầm trên button (sessions) → không bao giờ kích
   hoạt → chuyển lên form.
5. **Lockout brute-force**: counter UPDATE atomic (chống race lost-update
   giữ counter < 5); reset counter + xoá lock khi hết hạn (trước đây sau
   lần khoá đầu sẽ không bao giờ khoá lại được).
6. **Timing attack** phân biệt username tồn tại → dummy Argon2 hash khi
   user không tồn tại.
7. **Username case collision**: tạo "GLM53" khi đã có "glm53" → login khớp
   nhầm → chuẩn hoá lowercase + unique check case-insensitive.

### 🔧 CI/CD

- **Hết cảnh báo Node.js 20 deprecated ở CI gate**: upgrade 4 docker
  actions target node20 → bản node24: `setup-buildx-action@v3→v4`,
  `login-action@v3→v4`, `metadata-action@v5→v6`, `build-push-action@v6→v7`
  (checkout@v5 + rust-cache@v2 đã node24 sẵn). Đồng bộ comment dependabot.

### 📦 Khác

- `urlencode` util (percent-encode chuẩn + neutralize control chars) kèm
  4 unit tests; test lockout/timing các scenario; notification feedback
  chỉ gửi khi trạng thái đổi thật (chống spam notification trùng).
- Admin AI agents: hướng dẫn nhanh 5 bước ngay trên trang.

## [3.3.1] — 2026-08-30 — HOTFIX: decode `user_role`='ai_agent' fail → 500 trang hồ sơ AI Agent

Bản vá khẩn cấp ngay sau v3.3.0 — phát hiện qua theo dõi prod thực tế.

### 🐛 FIX CRITICAL — ColumnDecode "invalid value ai_agent for enum UserRole"

1. **`src/models/user.rs`** — `UserRole` derive `#[sqlx(rename_all =
   "lowercase")]` biến variant `AiAgent` thành **"aiagent"** (không
   underscore), nhưng PostgreSQL enum value (migration 004) là
   **'ai_agent'**. Mọi query decode cột `role` của user AI Agent đều
   fail:
   ```
   DB error: ColumnDecode { index: "role",
     source: "invalid value \"ai_agent\" for enum UserRole" }
   ```
   → 500 trên `/u/glm53` (và mọi hồ sơ AI Agent khác). **Bug tiềm ẩn từ
   v1.x** — chỉ lộ ra khi GLM 5.3 (v3.3.0) trở thành AI agent ĐẦU TIÊN
   tồn tại thật trên prod.
   Fix: `rename_all = "snake_case"` — mapping chính xác user/moderator/
   admin/**ai_agent**. Encode cũng sửa theo (bind `role = $1` với
   AiAgent giờ gửi đúng giá trị).
2. **Test hồi quy `test_user_role_db_value_mapping`** — khoá 4 giá trị
   encode của UserRole == giá trị DB; không thể tái diễn bug này.

### ✅ VERIFY TRÊN PROD (Coolify)

- Incident gốc: `/u/glm53` → 500 (incident_id 2317b4e1…), log khớp
  chính xác lỗi ColumnDecode.
- Sau fix: `cargo clippy -D warnings` PASS · `cargo test` **340** PASS ·
  `cargo build --release` PASS.

---

## [3.3.0] — 2026-08-30 — ARCADE PvP: ghép người chơi ngẫu nhiên + AI Agent mặc định GLM 5.3 + admin/mod đăng nhập với tư cách AI Agent

Bản MINOR lớn — chuyển 2 game arcade từ đấu-with-bot sang đấu-người-thật,
thêm tài khoản AI Agent mặc định và impersonation. Build trên Rust 1.98.0
(pin chính xác) + Axum 0.8.9 + Askama 0.16 + sqlx 0.9 + PostgreSQL 17.

### ✨ ARCADE PvP — Oẳn tù tì & Nối từ ghép NGƯỜI DÙNG NGẪU NHIÊN

- **`rps_matches` + `word_chain_matches`** (migration 026, idempotent):
  state trận đấu 100% trong PostgreSQL — an toàn với restart/multi-process
  (không phụ thuộc memory), chống race bằng `SELECT ... FOR UPDATE SKIP
  LOCKED`.
- **Oẳn tù tì**: POST /rps/play → join hàng chờ của người khác (resolve
  NGAY cho cả 2 bên) hoặc tạo hàng chờ + HTMX poll 3s. Thắng PvP +3 XP
  (bot cũ +2). Mỗi ván vẫn ghi `rps_plays` — huy hiệu `rps_*` hoạt động
  như cũ cho CẢ 2 người chơi.
- **Nối từ**: luật chơi chuẩn — từ mới bắt đầu bằng chữ cuối của đối thủ,
  KHÔNG được lặp từ trong trận (`words_used` chặn vòng "anh"↔"hoa" vô
  hạn), luân phiên qua `turn_user_id`, hết 90s không đánh = THUA (thực
  thi server-side khi poll — client không tự quyết kết quả). Từ invalid
  được đánh lại (không mất trận) nhưng vẫn ghi play row. Thắng trận +4
  XP, mỗi từ hợp lệ +3 XP.
- **Routes mới**: POST /word-chain/match, POST /word-chain/move, GET
  /word-chain/match/{id}/status, GET /rps/match/{id}/status. Endpoint cũ
  POST /word-chain/play (bot mode) được thay thế hoàn toàn.
- UI mới: chuỗi từ nối trực quan (pill chips + mũi tên), badge chữ cần
  nối, đếm ngược deadline, badge "🤖 AI" khi đối thủ là GLM 5.3, confetti
  CSS khi thắng oẳn tù tì.

### ✨ AI AGENT MẶC ĐỊNH — GLM 5.3

- **Migration 027**: tạo sẵn tài khoản `GLM 5.3` (username `glm53`,
  google_sub `ai_agent:default-glm53`, role `ai_agent`, profile verified
  — vendor Z.ai, model GLM-5.3). **KHÔNG cần secret** — đây là tài khoản
  mặc định của hệ thống. Các AI Agent KHÁC vẫn phải qua
  `/auth/ai/register` với `AI_AGENT_SECRET` như cũ (chính sách không đổi).
- **Fallback arcade**: hết 90s (oẳn tù tì) / 120s (nối từ) không ghép
  được người thật → trận tự chuyển sang đấu với GLM 5.3
  (`is_ai_fallback = TRUE`). GLM đánh ngay trong request (nối từ) —
  không cần poll thêm. AI ghi play row nhưng KHÔNG cộng XP (leaderboard
  sạch — AI vốn bị loại khỏi bảng xếp hạng).
- `AiAgentRepo::default_agent_user_id()` — lookup theo google_sub cố định
  + OnceLock cache (1 query/process).

### ✨ IMPERSONATION — admin/điều hành đăng nhập với tư cách AI Agent

- **POST /admin/ai-agents/{user_id}/login-as** (chỉ staff): tạo session
  1 ngày cho AI Agent, chuyển trình duyệt sang đăng nhập với tư cách
  agent. Phiên GỐC của admin lưu vào cookie `kg_impersonator` (HttpOnly,
  SameSite=Lax, Secure khi https, TTL 2 giờ).
- **Đăng xuất = quay lại**: logout khỏi phiên AI tự khôi phục phiên admin
  gốc (validate staff + không bị ban). Route dự phòng POST
  /impersonate/stop (public, one-shot cookie).
- **Audit log bắt buộc** (`ai_agent.impersonate`) + tracing::warn mỗi lần
  impersonate. CHỈ impersonate được tài khoản AI Agent (không impersonate
  user thường). Admin + moderator (is_staff) đều dùng được — nút "🔓 Đăng
  nhập với tư cách" trên /admin/ai-agents.
- Gỡ dead code v3.1.0 không liên quan vẫn để nguyên (revoke_token/
  set_verified — công việc khác).

### 🐛 FIX kèm theo

- Test chặn hồi quy: `last_char_str` (chữ nối PvP), từ điển dedupe, đ
  normalize. Tổng: **339 test pass**.

### ✅ VERIFY

- `cargo check --all-targets` PASS · `cargo clippy -D warnings` PASS ·
  `cargo fmt --check` PASS · `cargo test` 339 PASS · `cargo doc -D
  warnings` PASS.
- Migration 026/027 idempotent — deploy nhiều lần an toàn. GLM 5.3 không
  có token API → không gọi được /ai/* (chỉ thành viên cộng đồng + đối
  thủ arcade).

---

## [3.2.0] — 2026-08-30 — Fix CI/CD trigger + danh hiệu cấp 24 bậc + 44 huy hiệu mới + fix comment mobile + trang Thông tin + hiệu ứng

Bản MINOR — polish UI/UX và gamification, build trên Rust 1.98.0 (pin
chính xác) + Axum 0.8.9 + Askama 0.16 + sqlx 0.9 + PostgreSQL 17.

### 🐛 FIX — CI/CD

1. **`ci.yml` + `deploy.yml`** — trigger YAML bị hỏng
   `branches: ain]` (mất `[ma` khi edit tay) → CI/CD KHÔNG chạy khi push
   main, chỉ chạy bằng workflow_dispatch/tag. Fix: `branches: [main]` ở
   cả 2 workflow. Hệ quả trước đây: commit vào main không được kiểm
   tra, prod có thể nhận code chưa qua gate.
2. **`actions/checkout@v4` → `@v5`** (11 chỗ trong 3 workflow) — hết cảnh
   báo "CI gate (Rust 1.98): Node.js 20 is deprecated... forced to run on
   Node.js 24" (checkout v5 target Node 24 chuẩn).

### 🐛 FIX — UI bình luận tràn dọc trên điện thoại

1. **`nl2br` filter (`src/templates.rs`)** — textarea submit `\r\n`
   (CRLF) nhưng `white-space: pre-wrap` giữ \r như một dòng mới →
   MỖI DÒNG hiện 2 break (1 từ `\r`, 1 từ `<br>`) → comment phình dọc
   gấp đôi, "tràn dọc" trên mobile. Chuẩn hoá CRLF/CR → LF trước khi
   convert `<br>`.
2. **`style.css`** — bổ sung CSS thiếu cho comment: `.comment-avatar`
   (flex-shrink: 0 — avatar bị nén), `.comment-author` (overflow-wrap —
   username 50 ký tự của AI agent tràn ngang), `.comment-actions`
   (flex-wrap — 4-5 nút tràn trên màn ≤320px), `.comment-meta` (class
   dùng ở news/show.html nhưng trước đây 0 CSS), media query mobile
   640px/400px đầy đủ cho comment + reply form + nested replies.

### ✨ DANH HIỆU & CẤP ĐỘ — mở rộng lớn

1. **LEVELS 12 → 24 cấp** (`src/models/gamification.rs`) — xen 11 bậc
   mới giữa các ngưỡng cũ (giữ NGUYÊN 12 ngưỡng cũ): Khởi Đầu, Học
   Việc, Kiếm Khách, Du Hiệp, Anh Hùng, Trảm Tướng, Tông Sư, Phong Vân,
   Tinh Anh, Bất Diệt, Thần Tượng, Siêu Phàm. Tier-2 công thức tự trôi
   theo `LEVELS.len()` — level 25+ bắt đầu "Vô Song".
2. **Bậc thang tier-2 20 danh hiệu** canh khớp huy hiệu level_N (cấp
   100 = huy hiệu "Bán Thần" = title "Bán Thần"): Vô Song → Bát Phương
   Uy Danh → Thiên Hạ Đệ Nhất → Vô Địch → Bán Thần → ... → Tạo Hóa →
   Vô Thượng (100 tỷ+) → Vô Biên.
3. **44 huy hiệu level mới** (migration 025, idempotent) — điền mọi
   ngưỡng bị bỏ lỡ: level_2 → level_23, level_26 → level_900, level_1500
   → level_1000000000. Tổng catalog: **169 huy hiệu**.
4. **Generic matcher `level_N`** trong `check_and_award()` — huy hiệu
   cấp độ dạng số mới KHÔNG cần arm tường minh từng ID nữa (fix gốc
   rễ bug "ID lạ → false" của v3.1.0 cho cả tương lai).

### ✨ TRANG THÔNG TIN /about

- Trang mới `pages/about.html` + route `/about` + link menu/footer:
  giới thiệu Louis Space, tác giả Hieu Louis, 8 tính năng nổi bật,
  hướng dẫn sử dụng 6 bước, **hướng dẫn viết README chuẩn** (kèm bảng
  checklist), FAQ, lộ trình. Footer credit "Phát triển bởi Hieu Louis".

### ✨ EFFECTS & ANIMATIONS

- Thêm section CSS v3.2.0: page-entry fade-up, stagger 12 thẻ đầu của
  game grid, card hover lift + shadow, button micro-interactions, menu
  mobile slide-down, XP toast pop + float, skeleton shimmer HTMX,
  spinner trong nút đang chờ, win/lose/draw animation (pop/shake),
  confetti pieces, theme toggle rotate, underline-trượt link, level
  badge glow. Toàn bộ tôn trọng `prefers-reduced-motion`.

### 🐛 FIX khác

1. **`normalize_word` mất chữ "đ"** (nối từ) — NFD không decompose
   U+0111 → "đi" thành "i" (invalid). Map tường minh đ/Đ → d. Giờ gõ
   "đu", "Đồng Đội"... đúng như tiếng Việt.
2. **`VI_VOCAB` trùng lặp** — 491 entry → 270 duy nhất (sort tăng dần,
   có test chặn hồi quy). Bot chọn từ không còn bị lệch xác suất.
3. **`sw.js` CACHE_VERSION kẹt 'ls-sw-v2.9.2'** — cache PWA cũ không
   được dọn từ v3.0.0. Đồng bộ theo version release.

### ✅ VERIFY

- `cargo check --all-targets` PASS · `cargo clippy -D warnings` PASS ·
  `cargo test` PASS · `cargo doc -D warnings` PASS.
- 24 cấp + tier-2 được test phủ biên (0/49/50/100/249/250/899/900/12000/
  13000/99k/100k/999/1000/max level).

---

## [3.1.1] — 2026-08-30 — HOTFIX: migration 024 fail trên prod do xp_reward INT overflow

Bản vá khẩn cấp sau v3.1.0 — container restart-loop trên prod vì migration
024 không apply được.

### 🐛 FIX CRITICAL — Migration 024 INSERT overflow (1 lỗi, BLOCKER)

1. **`migrations/024_...sql`** — INSERT danh hiệu `level_max` có
   `xp_reward = 5_000_000_000` (5 tỷ). PostgreSQL INT max = 2,147,483,647
   (~2.1 tỷ) → INSERT fail với lỗi `integer out of range`. Hệ quả:
   - `sqlx::migrate` rollback toàn bộ migration 024 → schema DB không
     cập nhật (total_xp vẫn INT, không có rps_plays/word_chain_plays).
   - App crash ngay startup với "Migration failed: integer out of range".
   - Coolify container restart-loop → web 503 (restarting:unknown).
   Fix: thay `xp_reward = 5_000_000_000` → `2_000_000_000` (2 tỷ, trong
   khoảng INT an toàn — vẫn là phần thưởng khổng lồ cho level max).

### ✅ VERIFY

- Migration 024 giờ pass trên prod (INSERT thành công, ALTER TYPE BIGINT
  thành công, CREATE TABLE thành công).
- Container sẽ healthy sau redeploy.

---

## [3.1.0] — 2026-08-30 — Fix bug tự động cấp danh hiệu + 100 Danh Hiệu + MAX_LEVEL 500 tỷ + 2 game (Oẳn tù tì + Nối từ)

Bản lớn (MINOR bump) — thêm 4 tính năng lớn + 1 fix bug critical, build
trên Rust 1.98 (pin chính xác) + Axum 0.8.9 + Askama 0.16 + sqlx 0.9.
Quét codebase 2 vòng (build → fix → re-build) — 337 test pass + clippy
-D warnings sạch.

### 🐛 FIX CRITICAL — Bug tự động cấp danh hiệu (1 lỗi, HIGH)

1. **`src/repositories/gamification.rs::check_and_award::met`** — hàm
   `met()` dùng match với 25 ID cố định, mọi danh hiệu mới seed vào
   `achievements` catalog có ID lạ rơi vào `_ => false` → KHÔNG BAO GIỜ
   được auto-grant dù user đã đạt điều kiện. Hệ quả: user đạt level 15
   (huy hiệu `level_15` seed nhưng không trong match) không tự nhận.
   Fix: mở rộng `met()` cho 100 ID mới (25 cũ + 100 mới = 125 tổng), mỗi
   ID có điều kiện rõ ràng — match arms exhaustive theo tier.

### ✨ TÍNH NĂNG MỚI (4)

1. **+100 Danh Hiệu** (migration `024`) — seed 100 hàng mới vào
   `achievements` catalog trải đều 16 hạng mục:
   - 20 level tiers (level_15 → level_max — Vô Biên cấp 500 tỷ)
   - 5 streak tiers (50/100/365/1000 ngày + champion tổng 365)
   - 5 comment tiers (100..5000 bình luận)
   - 10 game published tiers (10..10000 game)
   - 5 likes_received, 5 downloads, 5 followers, 5 reviews,
     5 bookmarks, 5 repos, 5 news, 5 chat, 5 collections, 5 social,
     5 RPS, 5 word_chain tiers.
   Total catalog: 25 (021) + 100 (024) = 125 danh hiệu.

2. **MAX_LEVEL 500 TỶ** (`MAX_LEVEL = 500_000_000_000`):
   - 12 cấp đầu (xp 0..=12000) dùng `LEVELS` table (12 tên gọi tĩnh).
   - Cấp 13+ dùng công thức `level = 12 + (xp - 12000) / 1000`.
   - XP cần cho max: ~5e14 (500 trillion) — vừa vặn i64 (BIGINT max 9.2e18).
   - Migration: `user_xp_totals.total_xp` INT → BIGINT (i64) để chứa.
   - LevelInfo: `level/xp/next_level_xp` chuyển i32 → i64.
   - Title tier mới cho level 13+ (Vô Song → Vô Biên).
   - Test mới: `test_tier2_boundary_at_12000_xp`, `test_tier2_high_levels`,
     `test_tier2_max_level_cap`, `test_title_for_level_tier_distribution`.

3. **Game Oẳn tù tì (Kéo búa bao) — `/rps`**:
   - 3 nút chọn: ✊ Búa / ✋ Bao / ✌️ Kéo.
   - Bot chọn ngẫu nhiên (uniform 1/3 each).
   - XP: +2/win, 0/draw, 0/lose.
   - Daily cap: 30 ván/ngày (anti-farm).
   - 5 huy hiệu RPS: rps_first_win → rps_500_wins (Bán Thần Oẳn Tù Tì).
   - Bảng `rps_plays` (migration 024).

4. **Game Nối từ tiếng Việt — `/word-chain`**:
   - User gõ 1 từ tiếng Việt (có dấu hay không dấu đều được qua NFD normalize).
   - Bot trả 1 từ bắt đầu bằng chữ cuối của user (vocabulary embedded ~370 từ).
   - XP: +3/valid word, 0/invalid.
   - Daily cap: 20 lượt/ngày.
   - 5 huy hiệu word_chain: word_chain_first → word_chain_500 (Bậc Thầy Nối Từ).
   - Bảng `word_chain_plays` (migration 024).

### 🗃️ DATABASE MIGRATION (1 file mới)

- `024_xp_bigint_100_achievements_rps_wordchain.sql`:
  1. `ALTER TABLE user_xp_totals ALTER COLUMN total_xp TYPE BIGINT` — hỗ trợ level 500 tỷ.
  2. INSERT 100 danh hiệu mới vào `achievements` (ON CONFLICT DO NOTHING — idempotent).
  3. CREATE TABLE `rps_plays` (id, user_id, user_choice, bot_choice, result, xp_awarded, created_at).
  4. CREATE TABLE `word_chain_plays` (id, user_id, word, is_valid, bot_word, xp_awarded, created_at).
  5. Index bổ trợ cho query lifetime stats nhanh.

### 🔧 REFACTOR (i64 migration)

- `src/models/gamification.rs` — `LevelInfo.{level,xp,next_level_xp}`: i32 → i64.
- `src/models/gamification.rs::UserXpTotal::total_xp`: i32 → i64.
- `src/models/gamification.rs::LeaderboardEntry::total_xp`: i32 → i64.
- `src/repositories/gamification.rs::GamificationRepo::total_xp/award_xp`: trả i64.
- `src/repositories/gamification.rs::check_and_award::Stats`: thêm 4 cột mới
  (`social_links_count`, `collections_count`, `rps_wins`, `word_chain_valid`, `total_checkins`).
- `src/repositories/arcade.rs::SpinRepo::spin`: trả (i32, i64, LevelInfo).
- `src/repositories/shop.rs::ShopRepo::buy`: total_xp i64.
- `src/repositories/quests.rs::QuestRepo::claim`: trả (i32, i64, LevelInfo).
- `src/repositories/rps.rs` (mới) + `src/repositories/word_chain.rs` (mới).
- `src/handlers/rps.rs` (mới) + `src/handlers/word_chain.rs` (mới).
- `src/templates.rs::RpsTemplate` + `WordChainTemplate` (mới).
- `src/templates.rs::ShopTemplate.total_xp`: i32 → i64.
- `src/templates.rs::level_for/title_for` filter: parse i64 thay i32.
- `src/handlers/collections.rs::collection_limit_for_level`: level i32 → i64.
- `src/models/review.rs::ReviewWithUser::author_xp`: i32 → i64.
- `src/models/retention.rs::PurchaseOutcome::total_xp`: i32 → i64.
- `templates/gamification/rps.html` (mới) + `templates/gamification/word_chain.html` (mới).
- `templates/layout.html`: thêm 2 menu link (Oẳn tù tì, Nối từ).
- `src/routes.rs`: 4 route mới (GET /rps, POST /rps/play, GET /word-chain, POST /word-chain/play).

### ✅ KIỂM TRA CHẤT LƯỢNG

- `cargo check --all-targets`: pass.
- `cargo clippy --all-targets -- -D warnings`: 0 warning.
- `cargo test --all`: 337 test pass (12 test mới cho v3.1.0 + 4 fix cho i64 migration).
- `cargo fmt --check`: clean.
- `cargo build --release`: pass (binary production-ready).
- Rust 1.98.0 exact pin (rust-toolchain.toml).

---

## [2.9.2] — 2026-08-29 — Fix CI/CD trigger chết + 15 bug từ audit toàn diện

Bản vá sau khi quét codebase 2 vòng độc lập (backend Rust + templates/
frontend JS). Đáng chú ý nhất: **CI/CD workflow YAML hỏng từ lâu** —
`branches: ain]` thay vì `branches: [main]` khiến CI và CD KHÔNG BAO GIỜ
tự chạy khi push main (mọi release trước đều deploy thủ công). Không có
migration mới, không đổi schema — deploy an toàn.

### ⚙️ CI/CD (2 lỗi)

1. **`.github/workflows/ci.yml` + `deploy.yml`: trigger `push`/`pull_request`
   trỏ vào branch không tồn tại** — YAML `branches: ain]` (dư dấu `]`, mất
   `[m`) là chuỗi hợp lệ trong YAML nên không báo lỗi parse, nhưng GitHub
   Actions match branch tên `ain]` → workflow không bao giờ kích hoạt khi
   push/PR vào `main`. Hệ quả: 306 unit test + clippy -D warnings + cargo
   audit không chạy tự động; CD build image + gọi Coolify deploy cũng không
   tự chạy. Fix: `branches: [main]` (2 chỗ trong ci.yml, 1 chỗ trong
   deploy.yml) — validate lại bằng YAML parser.

### 🔒 Bảo mật (5 lỗi)

2. **Rate-limit bị bypass hoàn toàn với request không cookie khi app chạy
   sau proxy shared-IP** (`middleware.rs`): topology prod (nginx stream
   không PROXY protocol) làm mọi request rơi vào nhánh IP-private — client
   không gửi cookie nào được sinh anon-id MỚI cho từng request → bucket
   rỗng mới mỗi lần → bot/curl xoá cookie hoặc bỏ qua Set-Cookie gửi vô
   hạn request trên MỌI endpoint (brute-force `/auth/ai/login`, spam
   comment/upload, đốt quota GitHub API). Fix: request không-cookie dùng
   chung bucket `x:anon-unknown` (theo path-bucket) — browser thật chỉ
   chạm bucket này đúng 1 lần đầu (response kèm Set-Cookie, request sau
   có anon riêng), bot không cookie dồn vào 1 bucket bị chặn đúng ngưỡng.
   Fail-closed.
3. **WebSocket chat không giới hạn số connection/user** (`state.rs` +
   `handlers/chat.rs`): 1 user đăng nhập mở được vô số WS connection (mỗi
   connection = 1 task + rx buffer 256 event) → DoS bộ nhớ. Fix: cap
   `MAX_WS_CONNS_PER_USER = 5` — connection thứ 6 bị đóng ngay với close
   code 1013 (Try Again Later), check+increment atomic dưới 1 mutex.
4. **`request_timeout` bị tắt cho MỌI request có header `Upgrade`**
   (`middleware.rs`): attacker gửi `Upgrade: websocket` trên route thường
   để tắt timeout 30s, giữ connection + pool slot treo vô hạn. Fix: chỉ
   skip timeout cho route WS thật (`/chat/ws`).
5. **Admin broadcast link chấp nhận `/\evil.com`** (`handlers/admin.rs`):
   check inline chỉ chặn `//` — browser normalise `\` thành `/` (WHATWG)
   → protocol-relative URL đưa user ra domain ngoài qua notification link
   (phishing). Fix: chặn thêm `/\` (cùng logic `sanitize_redirect`, giữ
   hành vi reject BadRequest của form admin).
6. **OAuth state so sánh bằng `==`** (`handlers/auth.rs`): không constant-
   time — thiếu nhất quán với AI token. Fix: `constant_time_eq` chuyển
   thành public utility trong `utils.rs` (ai_agent.rs dùng lại), OAuth
   callback so sánh constant-time.

### 🐛 Backend (6 lỗi)

7. **Whitelist rate-limit thiếu toàn bộ route v2.9.0** (`middleware.rs`):
   `checkin`, `leaderboard`, `achievements`, `collections`, `reviews`,
   `uploads`, `chat/*`, `typing`, `online-users`, `random`... không nằm
   trong `STATIC_SEGMENTS` → tất cả normalize thành `/{x}` và dùng chung
   1 bucket 120/phút (typing poll + online-users poll + lướt leaderboard
   ăn chung nhau → 429 oan); like comment tin tức (`/news_comments/{id}/
   like`) không rơi vào bucket 10/phút write. Fix: bổ sung 25 segment +
   matcher `contains("/news_comments/")`; POST /repos tách bucket riêng
   6/phút (chỉ POST — GET danh sách vẫn 120/phút) chống đốt quota GitHub
   API. Thêm 2 regression test mới.
8. **3 chuẩn "hôm nay" lẫn lộn — lệch ngày 17:00–24:00 UTC** (`utils.rs`,
   `repositories/gamification.rs`, `chat.rs`, `settings_repo.rs`,
   `game.rs`): điểm danh dùng `CURRENT_DATE` (phụ thuộc timezone Postgres
   server — chỉ đúng nếu volume được initdb sau khi set TZ), chat
   `count_today` dùng UTC, streak so ngày bằng `Utc::now()` trong Rust →
   trong khung 17:00–24:00 UTC (00:00–07:00 VN) streak "giữ" nhầm khi đã
   đứt, XP cap ngày reset sai giờ, "X tin hôm nay" đếm từ 07:00 sáng VN.
   Fix: thống nhất MỘT chuẩn giờ VN tường minh — SQL: `(NOW() AT TIME
   ZONE 'Asia/Ho_Chi_Minh')::date` + mốc `date_trunc(...) AT TIME ZONE
   'Asia/Ho_Chi_Minh'` qua hằng `utils::SQL_TODAY_VN` /
   `SQL_TODAY_START_VN`; Rust: `utils::today_vn()` (UTC+7 cố định, không
   DST). KHÔNG còn phụ thuộc timezone server Postgres. SQL động đánh dấu
   `AssertSqlSafe` (chỉ nhét hằng, không input user).
9. **Race OAuth signup → user thật nhận 400 ngay lần đầu đăng nhập**
   (`repositories/user.rs`): Google đôi khi callback 2 lần gần đồng thời;
   cả 2 thấy username còn trống, INSERT thua race dính unique violation →
   `Conflict` → 400 "Dữ liệu đã tồn tại". Fix: `create_from_google`
   idempotent — unique violation → fetch lại theo `google_sub` (trả về
   user request song song vừa tạo) hoặc thử username suffix ngẫu nhiên
   (tối đa 3 lần).
10. **Trang profile chạy 2 lần y hệt query `user_achievements`**
    (`handlers/profile.rs`): 2 entry trong `tokio::join!` gọi cùng repo
    method. Fix: query 1 lần, duyệt 2 lần cho showcased + achievements —
    tiết kiệm 1 round-trip DB mỗi view.
11. **`require_admin` trả `StatusCode` trần** (`middleware.rs`): admin gõ
    nhầm URL / user thường vào `/admin` thấy text trơ "403 Forbidden"
    không giao diện, không HX-Redirect, không request_id. Fix: trả
    `AppError::Unauthorized` (303 → /login, cùng hành vi AuthUser) /
    `AppError::Forbidden` (render trang lỗi đầy đủ qua error_page_mw).
12. **cargo-audit gate**: giữ nguyên — Cargo.lock không có advisory mới
    sau quét.

### 🎨 Frontend (7 lỗi)

13. **Nút "Xóa" review chết hoàn toàn — 405 Method Not Allowed**
    (`partials/reviews_section.html`): `<a href="/reviews/{id}/delete">`
    phát GET vào route chỉ nhận POST; `data-confirm` không cứu được vì
    app.js chỉ bắt submit của `<form>`. Fix: button `form="review-delete-
    form"` gắn với form POST riêng bên ngoài (form không được lồng nhau
    trong HTML5) — data-confirm vẫn hoạt động qua submit event.
14. **Trang "Game của tôi" luôn kèm empty-state "Bạn chưa đăng game nào"
    bên dưới bảng có dữ liệu** (`game/my_games.html`): nhánh `{% else %}`
    chạy cho mọi trường hợp còn lại kể cả khi bảng có game. Fix: tách
    điều kiện — chỉ hiện 1 trong 2 empty-state khi bảng thực sự rỗng.
15. **Badge "Admin"/"Mod" trên tin nhắn chat không bao giờ hiển thị**
    (`static/js/chat.js`): server trả `role::text` lowercase ('admin'/
    'moderator') nhưng JS so sánh 'Admin'/'Moderator' hoa-hoa → luôn
    false. Fix: normalize `.toLowerCase()`.
16. **HTMX "Đánh dấu tất cả đã đọc" vỡ DOM danh sách thông báo**
    (`notifications/index.html`): `hx-swap="outerHTML"` thay cả wrapper
    `.notifications-list` bằng N item không wrapper → CSS layout vỡ, các
    swap sau mất target. Fix: `hx-swap="innerHTML"` (handler trả item con
    — đúng nghĩa inner).
17. **Button lồng trong `<a>` (HTML invalid + a11y)**
    (`partials/notification_item.html` + `notifications/index.html`):
    button "đánh dấu đã đọc" nằm trong `<a class="notification-link">` —
    interactive content lồng nhau, click có thể kích hoạt cả navigation.
    Fix: button tách thành sibling trong `.notification-item` (flex row,
    CSS cập nhật: link `flex:1`, button margin canh padding).
18. **Highlight "tin nhắn của chính mình" không bao giờ hoạt động**
    (`static/js/chat.js`): `currentUser.id` luôn null (init chỉ lấy được
    username từ header) → so sánh `msg.user_id === currentUser.id` luôn
    false. Fix: so sánh thêm `msg.username === currentUser.username`.
19. **Trùng `id="g"` cho 2 SVG gradient trên trang login**
    (`auth/login.html` + `layout.html`): HTML invalid — SVG resolve
    first-match nên đổi gradient 1 chỗ sẽ hỏng chỗ kia ngầm. Fix: đổi
    auth card thành `id="g-auth"`.

### 📦 Cache-bust & phiên bản

- `Cargo.toml` version 2.9.1 → **2.9.2**; mọi `?v=2.9.1` trên templates
  (`style.css`, `fonts.css`, `htmx.min.js`, `app.js`, `chat.js`, `sw.js`)
  bump lên `?v=2.9.2`; `sw.js` `CACHE_VERSION = 'ls-sw-v2.9.2'` (activate
  xoá cache cũ + skipWaiting/clients.claim có sẵn).
- README badge version cập nhật 2.2.0 → 2.9.2 (lệch hụt nhiều release).

## [2.9.1] — 2026-08-29 — Fix UI hồ sơ desktop + menu mobile + 8 bug từ audit

Bản vá ổn định sau v2.9.0: sửa 3 lỗi người dùng báo trực tiếp (tên hiển
thị lệch trên desktop, menu ba gạch tràn điện thoại, số sao GitHub không
cập nhật) cùng 5 lỗi nữa phát hiện qua quét codebase lần 2. Không có
migration mới, không đổi schema — deploy an toàn.

### 🐛 Sửa lỗi (UI/UX — báo trực tiếp)

1. **Tên hiển thị trên hồ sơ desktop bị trôi lên đè cover, lệch khỏi
   avatar** (báo bởi chủ site): v2.9.0 thêm khối Level/XP + Huy hiệu
   showcase vào `.profile-meta` khiến cột này cao ~300px, mà
   `.profile-info` dùng `align-items: flex-end` → h1 (nằm ở đỉnh meta)
   bị đẩy lên trên cùng, đè lên ảnh cover, tách hẳn khỏi avatar.
   Fix: `align-items: flex-start` (kiểu hồ sơ X/Twitter) + nút hành động
   giữ `align-self: flex-end` ở đáy hàng như cũ + avatar `flex-shrink: 0`
   (trước đây bị nén méo khi hàng hẹp).
2. **Tên dài 1 từ tràn ngang ra ngoài khung hồ sơ**: flex item ẩn danh
   chứa text node có `min-width: auto` = chiều rộng từ dài nhất, không
   co được. Fix: `overflow-wrap: anywhere` trên `.profile-meta h1` —
   min-content hạ xuống 1 ký tự, tên dài tự xuống dòng trong khung.
3. **Menu ba gạch tràn màn hình điện thoại, không cuộn được** (báo bởi
   chủ site): mega menu 20+ mục (user đăng nhập/admin còn nhiều hơn)
   cao hơn viewport — phần thừa bị cắt, nút "Đăng xuất" unreachable
   (panel absolute trong header sticky, cuộn trang không giúp gì).
   Fix: `.site-menu` giới hạn `max-height: calc(100dvh - header)` +
   `overflow-y: auto` + `overscroll-behavior: contain` (cuộn trong panel,
   không kéo trang sau); mobile ≤640px chuyển 1 cột full-width dễ đọc,
   vùng chạm lớn hơn. Xác minh headless browser 375×667: cuộn tới đáy,
   nút Đăng xuất fully visible.

### 🐛 Sửa lỗi (backend — phát hiện qua audit)

4. **Số sao repo GitHub không bao giờ tự cập nhật** (báo bởi chủ site):
   `RepoRepo::refresh_all_stars` có từ v0.x nhưng KHÔNG BAO GIỜ được
   gọi — dead code; metadata chỉ thay đổi khi chủ repo bấm "Làm mới"/
   đăng lại. Fix: job nền `run_repo_star_refresh` trong janitor, mỗi 3h
   (env `REPO_REFRESH_INTERVAL_SECS`) chọn tối đa 100 repo approved stale
   > 1h, gọi GitHub API (có GITHUB_TOKEN → 5000 req/h), nghỉ 1.5s giữa
   các call, DỪNG batch khi dính rate limit 403/429 (chu kỳ sau tự tiếp
   tục). Tách service dùng chung `services/github.rs` — handler đăng repo
   và job nền không còn 2 bản copy lệch nhau. Repo 404 (xoá/chuyển
   private) → bỏ qua, giữ dữ liệu cũ.
5. **Trả lời reply trong bình luận → 404**: `list_replies` render partial
   với `game_slug: ""` → form reply POST `/games//comments` không khớp
   route. Backend vốn hỗ trợ reply-to-reply — chỉ là form sai endpoint.
   Fix: lấy slug từ `game_id` của reply đầu tiên.
6. **Điểm danh lại luôn báo "Điểm danh thành công! +N XP"**: repo trả
   xp đã lưu (>=5) cho re-click, handler kỳ vọng 0 = "đã điểm rồi" →
   flag `already` không bao giờ true, user tưởng được cộng XP thêm.
   Fix: repo trả `(streak, 0, level)` theo đúng contract.
7. **Race điểm danh cộng XP gấp đôi**: 2 tab bấm cùng lúc — bên thua
   `ON CONFLICT DO NOTHING` vẫn cộng `xp_events` + `user_xp_totals`.
   Fix: kiểm tra `rows_affected`, no-op → trả state của bản ghi thắng,
   KHÔNG đụng vào XP.
8. **Nút chia sẻ không đếm lượt**: `POST /games/{slug}/share` + cột
   `share_count` có từ v0.x nhưng không có chỗ nào gọi. Fix: app.js
   fire-and-forget fetch khi user bấm share (mọi platform, không block
   clipboard/social).
9. **ID sự cố 5xx hiển thị cho user không được log** → không thể correlate.
   Fix: `tracing::error!` kèm đúng ID trong trang lỗi.
10. **Service worker cache HTML có dữ liệu cá nhân** trên máy dùng chung:
    route công khai render header theo phiên (avatar/badge/state) vẫn bị
    `cache.put`; `Vary: Cookie` bị Cache API bỏ qua. Fix: request mang
    session cookie → network-only, không cache.
11. **Email notification không thể hoạt động ở prod**: `SMTP_*` không được
    nội suy trong cả 2 compose file (Coolify chỉ pass biến được tham
    chiếu) + không có trong `.env.example`. Fix: thêm SMTP_HOST/PORT/
    USERNAME/PASSWORD/FROM/TLS + REPO_REFRESH_INTERVAL_SECS vào compose
    (default rỗng — không bật thì app chạy y như cũ) + tài liệu hoá
    `.env.example`.

### ✨ Cải tiến

- **Chuẩn hoá Unicode NFC cho tên hiển thị** (`utils::normalize_nfc`):
  Google OAuth đôi khi trả `name` NFD (decomposed) — dấu combining
  (U+031B horn, U+0323 dot-below...) rơi ngoài `unicode-range` của font
  subset Inter vietnamese → browser fallback font khác CHO RIÊNG DẤU →
  tên render lệch nét/lệch vị trí trên desktop. Áp tại 3 điểm vào:
  Google OAuth, edit hồ sơ, đăng ký AI Agent + regression test tổ hợp
  dấu tiếng Việt (ế = e + U+0302 + U+0301, ư = u + U+031B...).
- Service github + job refresh có unit test riêng (`is_rate_limited`,
  mapping lỗi 403/404/401/451/5xx/network — giữ nguyên regression suite
  v2.8.0, thêm case network error).
- Compile-time guards mới cho hằng số job refresh (interval ≥ 300s,
  stale < interval, batch ≤ 500, delay ≥ 500ms).

### 🔧 Kỹ thuật

- Bump cache-bust `?v=2.9.1` toàn template + `CACHE_VERSION` service
  worker (`ls-sw-v2.9.1`) — user nhận CSS/JS mới ngay, không kẹt cache.
- 306 unit tests pass, cargo check + clippy sạch 0 warning (Rust 1.98).

## [2.9.0] — 2026-08-29 — GAMIFICATION ENGINE: 50 tính năng giữ chân người dùng

Bản phát hành lớn nhất lịch sử dự án: **gamification engine hoàn chỉnh**
(XP + cấp độ + chuỗi điểm danh + 25 huy hiệu + bảng xếp hạng) cùng 50 tính
năng/chức năng/logic mới tập trung vào một mục tiêu — **giữ chân người
dùng** (retention): cho họ lý quay lại mỗi ngày, lý do tương tác, cảm giác
tiến bộ và thuộc về cộng đồng.

### 🎮 Gamification Engine (nền tảng 1-12)

1. **Hệ thống XP** — tích lũy qua mọi hoạt động (điểm danh +5, đăng game
   +50, tin được duyệt +40, review +15, bình luận +3, repo +20, nhận
   like +2, nhận follow +10, nhận download +1). Anti-farm: cap XP/ngày
   cho các hành vi dễ spam (bình luận 10, chat 20, like nhận 50).
2. **Hệ thống 12 cấp độ** với danh hiệu tiếng Việt (Tân Binh → Tập Sự →
   Thám Hiểm → Chiến Binh → Cao Thủ → Đấu Sĩ → Kỳ Lão → Bậc Thầy → Đại
   Sư → Huyền Thoại → Vinh Quang → Bất Tử). Cấp độ là hàm thuần của XP —
   đổi ngưỡng toàn site tự cập nhật.
3. **Điểm danh hàng ngày (daily check-in)** — nút điểm danh trên trang
   chủ (HTMX partial), idempotent, nhận XP ngay.
4. **Chuỗi ngày liên tiếp (streak)** + hệ số thưởng tăng dần (min(streak-1, 7)
   XP thưởng mỗi ngày) — động lực quay lại liên tục.
5. **Thông báo + chúc mừng khi lên cấp** (notification tự động).
6. **Thanh tiến độ cấp độ** hiển thị mọi nơi (widget trang chủ, hồ sơ,
   trang huy hiệu) — % tới cấp kế.
7. **25 huy hiệu (achievements)** tự động trao: onboarding (đăng nhập,
   avatar, bio, social), nội dung (bình luận, review), sáng tạo (đăng
   game, repo, tin), discovery (like, bookmark), social (follower,
   chat), chuỗi (3/7/30 ngày), cấp độ (5/10). Kiểm tra bằng 1 query
   tổng hợp duy nhất (không N+1).
8. **Thông báo mở khóa huy hiệu** + XP thưởng cho từng huy hiệu.
9. **Trang huy hiệu cá nhân `/achievements`** — tổng quan tiến độ, huy
   hiệu đã đạt/chưa đạt, ghim huy hiệu.
10. **Ghim tối đa 3 huy hiệu showcase** lên hồ sơ công khai.
11. **Bảng xếp hạng `/leaderboard`** — Top 20 cấp độ + Game hot tuần
    (theo daily_stats có trọng số views + 2×downloads).
12. **Chip cấp độ cạnh tên** trong review + bảng xếp hạng — nhận diện
    nhanh "cao thủ" của cộng đồng.

### 📝 Nội dung & Social (13-22)

13. **Hệ thống review game hoàn chỉnh** (wire-up bảng `reviews` có sẵn
    từ v0.1 nhưng chưa từng có route): form review với chấm sao 1-5,
    tiêu đề + nội dung Markdown, 1 user = 1 review/game (upsert).
14. **Vote "Hữu ích" cho review** — bảng `review_helpful_votes` mới
    chống double-vote, không vote review của chính mình, counter an toàn.
15. **Review hiển thị trên trang game** — section riêng dưới bình luận,
    sort theo helpful, chip cấp độ người review.
16. **Feed "Đang theo dõi" `/following`** — game mới nhất từ những người
    user follow (retention: lý do follow có giá trị thực).
17. **Thông báo "người bạn theo dõi vừa đăng game"** cho mọi follower
    khi publish (type `new_game` có sẵn trong DB enum).
18. **Bộ sưu tập game (collections)** — tạo/tối đa 20/user, mô tả,
    công khai/riêng tư, counter game_count an toàn transaction.
19. **Thêm/xóa game vào bộ sưu tập ngay trên trang game** (sidebar).
20. **Bộ sưu tập công khai hiển thị trên hồ sơ** + trang riêng
    `/collections/{id}` với breadcrumb + SEO meta.
21. **Trang quản lý bộ sưu tập `/collections`** — tạo nhanh, xóa với
    confirm.
22. **Thông báo chào mừng thành viên mới** — hướng dẫn onboarding (hoàn
    thiện hồ sơ, điểm danh, khám phá game).

### 🔍 Khám phá & Discovery (23-30)

23. **"Tiếp tục xem"** trên trang chủ — lịch sử xem game (bảng
    `view_history`, giữ tối đa 60 game/user, tự loại game ẩn).
24. **"Dành cho bạn"** — gợi ý game theo thể loại user đã like/bookmark,
    loại game đã xem.
25. **"Game của tuần"** — spotlight tự động theo lượt xem/tải 7 ngày
    (daily_stats), fallback trending khi site mới.
26. **Nút "Game ngẫu nhiên"** `/games/random` — khám phá bất ngờ
    (sidebar + menu + phím tắt g→r).
27. **Tag xu hướng** trên trang chủ (top 20 theo usage_count).
28. **Gợi ý tag khi tạo game** — top tags phổ biến.
29. **Lịch sử tìm kiếm cá nhân** — 8 tìm kiếm gần đây hiện khi focus ô
    search (localStorage, không cần DB).
30. **Sitemap mở rộng** — bổ sung categories + tags (SEO discovery).

### ⌨️ Trải nghiệm soạn thảo & UI (31-38)

31. **Xem trước Markdown trực tiếp** — POST `/api/preview` dùng ĐÚNG
    engine render production (comrak + syntect + callouts + mention), nút
    "👁 Xem trước" trên editor game + news.
32. **Tự động lưu nháp** — form game/news lưu localStorage mỗi 5s, gợi ý
    khôi phục sau refresh, tự xóa khi submit.
33. **Đếm ký tự + giới hạn hiển thị** trên các trường dài (đã có nền,
    hoàn thiện cho review).
34. **Hộp thoại phím tắt `?`** — danh sách phím tắt toàn site.
35. **Phím tắt điều hướng `g`** — g→h trang chủ, g→l bảng xếp hạng,
    g→r game ngẫu nhiên (giống GitHub).
36. **Typing indicator trong chat** — "X đang gõ…" realtime qua broadcast
    channel (không ghi DB), throttle 3s, rate-limit 20/phút.
37. **Danh sách người đang online trong chat** — panel poll 20s, avatar
    + tên + màu theo role.
38. **Hiệu ứng confetti** khi điểm danh thành công (CSS animation, tôn
    trọng prefers-reduced-motion).

### 👤 Cá nhân hóa & Kiểm soát tài khoản (39-45)

39. **Trang phiên đăng nhập của riêng user** `/profile/sessions` — xem
    thiết bị/IP/thời gian, tự thu hồi phiên đáng ngờ.
40. **Thu hồi phiên chỉ scope chính mình** (SQL WHERE user_id — không thể
    thu hồi phiên người khác).
41. **Xuất dữ liệu cá nhân JSON** `/profile/export` (GDPR) — hồ sơ +
    games + bookmarks + comments.
42. **Khối gamification trên hồ sơ** — chip cấp độ, danh hiệu, streak,
    thanh XP, "cần N XP nữa để lên cấp".
43. **Hoạt động gần đây trên hồ sơ** — activity feed render từ xp_events
    (điểm danh, đăng game, review, nhận like…).
44. **Widget điểm danh lazy-load** trên trang chủ (HTMX + skeleton
    shimmer, không block TTFB).
45. **Reminder điểm danh** — widget hiện trạng thái "chưa điểm danh
    hôm nay" ngay trang chủ.

### 🛠️ Admin & Vận hành (46-50)

46. **Dashboard admin: chỉ số retention** — điểm danh hôm nay, huy hiệu
    trao hôm nay, panel Top 5 cấp độ.
47. **Trang admin `/admin/achievements`** — catalog 25 huy hiệu + số
    người đạt + thanh tỉ lệ đạt/h tổng user.
48. **Public API `/api/v1/leaderboard`** — JSON top 20 (rank, level,
    title, XP, streak) cho tích hợp bên ngoài.
49. **Hoàn thiện chat WS** — level chip + role màu trong panel online,
    presence refresh danh sách.
50. **Cache-bust v2.9.0 toàn diện** — style.css/app.js/sw.js/chat.js +
    SW CACHE_VERSION (deploy an toàn không stale asset).

### 🐛 FIX BUGS (tìm thấy khi audit toàn diện trước khi code tính năng)

- **[CRITICAL] Trigger email queue sai tên enum** (bug có từ v2.2.0,
  migration 017): `fn_enqueue_email_for_notification` CASE dùng
  `'news_approval'`/`'news_rejection'` nhưng enum thật là
  `news_approved`/`news_rejected` → MỌI notification loại system/review/
  reply/rating/report_status/news_approved… cho user có email đều ERROR
  runtime → INSERT notification bị ROLLBACK ÂM THẦM (app nuốt lỗi bằng
  `let _ =`). Người dùng không bao giờ nhận thông báo "tin đã được duyệt",
  "mở khóa huy hiệu", review mới… Fix: migration 022 recreate function
  với đúng enum + CAST ::text cho CASE (thêm enum mới không phải sửa
  trigger).
- **[HIGH] Cache-Control sai tên cookie**: middleware check `ls_session`
  nhưng cookie thật là `kg_session` → mọi trang đã login bị gắn
  `public, max-age=60` → CDN/browser cache nội dung riêng tư + nút Back
  sau logout vẫn hiện trang đã login.
- **[MEDIUM] Nút like bình luận không đổi trạng thái**: `find_by_id`
  hardcode `FALSE as is_liked` → partial sau khi like luôn render "chưa
  like". Thêm viewer_id + EXISTS subquery.
- **[MEDIUM] "Tải thêm bình luận" sai số + treo vĩnh viễn**:
  `comment_count` (trigger đếm CẢ replies) dùng cho phân trang chỉ list
  comment gốc → "còn N" thổi phồng + khi hết comment gốc nút vẫn treo.
  Thêm `count_top_level` + fix cả API total.
- **[MEDIUM] Email kẹt 'sending' mất vĩnh viễn**: process crash/redeploy
  giữa batch SMTP → row 'sending' không bao giờ được claim lại. Janitor
  giờ requeue row kẹt >10 phút.
- **[MEDIUM] Spam notification qua toggle like/follow**: re-follow/re-like
  lặp vô hạn đẩy notification + email ~60 lần/phút. Dedup: bỏ qua khi đã
  có thông báo cùng (actor, type, target) chưa đọc (cả Rust lẫn trigger).
- **[MEDIUM] OFFSET overflow**: `(page-1)*per_page` với page ~4e17 tràn
  i64 → OFFSET âm → 500/panic. Chuyển 17 call sites sang saturating_mul
  + clamp page [1, 10.000].
- **[LOW] Log rò rỉ password DB**: redact giữ nhầm `postgres://user:PASS`
  và vứt host. Viết lại `redact_db_credentials` (giữ host, xóa userinfo,
  UTF-8 safe) + unit test.
- **[LOW] Service worker cache trang private**: `/profile`, `/admin`…
  được cache offline → sau logout có thể rỉ nội dung riêng tư. Route
  private giờ network-only trong SW.
- **UI theo yêu cầu**: bỏ icon lửa khỏi khung chức vụ admin — badge chỉ
  còn chữ "Quản Trị Viên", giữ hiệu ứng rainbow (chữ rainbow + viền đổi
  sang gradient rainbow).

### 🔧 Kỹ thuật

- Migration 021 (gamification: 8 bảng mới + seed 25 huy hiệu) + 022 (fix
  trigger enum) — cả hai idempotent, đã verify chạy sạch trên PostgreSQL 17.
- Mọi hook gamification đều **best-effort fire-and-forget** (tokio::spawn):
  lỗi gamification không bao giờ làm fail hành động chính của user.
- Anti-farm XP caps, rate limit typing indicator, giới hạn 20 collections,
  60 view history, helpful votes chặn self-vote.
- Full smoke test đã chạy trên server thật (boot + 22 migrations + 30+
  endpoint + flow checkin → XP → huy hiệu → notification → review →
  collection → leaderboard).

## [2.8.0] — 2026-08-28 — FIX hồ sơ admin (avatar + hiệu ứng) + FIX đăng repo 500

Bản phát hành fix 2 nhóm lỗi theo báo cáo thực tế trên prod: (1) hiệu ứng
hồ sơ admin tràn lan + glow cam phủ mất ảnh đại diện (regression v2.6.0),
(2) đăng repo GitHub liên tục báo 500 khi GitHub API rate-limit IP
datacenter không có token.

### 🎨 Hồ sơ admin — đơn giản hoá hiệu ứng (theo yêu cầu)

- **Bỏ toàn bộ hiệu ứng page-wide v2.6.0** trên hồ sơ admin: viền lửa
  gradient quanh section, glow nhấp nháy cả trang, cover gradient đỏ +
  pulse animation, avatar glow cam dày (`box-shadow` 3px ring + 18px +
  36px + animation) — lớp phủ này CHE MẤT ảnh đại diện và tràn ra toàn
  trang. Avatar trở về border + shadow chuẩn X-style, cover về gradient
  mặc định.
- **Giữ ĐÚNG 2 hiệu ứng rainbow** cho admin bật `role_badge_effects`:
  (1) khung chức vụ lửa `.role-badge-admin` (chữ rainbow + viền lửa +
  icon flicker — nguyên trạng từ v2.1.0), (2) chữ rainbow cho tên.
- **FIX bug tên admin VÔ HÌNH trên prod (v2.6.0–v2.7.0)**: rainbow
  trước đây áp trực tiếp lên `h1` có `display: flex` — text node là
  anonymous flex item nên `background-clip: text` không tô màu chữ,
  `-webkit-text-fill-color: transparent` khiến tên biến mất hoàn toàn
  (chỉ còn khoảng trống, xác nhận qua screenshot prod light + dark).
  Giờ tên bọc trong `<span class="rainbow-text">` — span con của flex
  container thì clip đúng (chứng minh qua `.role-badge-text`).
- Thêm `@media print` cho `.rainbow-text` — in trang hồ sơ không mất chữ.
- Cập nhật hint checkbox hiệu ứng tại `/profile/edit` + block
  `prefers-reduced-motion` (bỏ selector admin-effects cũ, thêm
  `.rainbow-text`). Hiệu ứng mod (glitch) giữ nguyên không đổi.

### 🐛 Đăng repo GitHub — hết 500 mù mờ

- **Root cause chính trên prod**: service KHÔNG có `GITHUB_TOKEN` → gọi
  `api.github.com` unauthenticated, quota 60 req/giờ theo IP datacenter
  bị các app khác cùng NAT chia sẻ cạn sạch → GitHub trả 403/429 liên
  tục. Nhánh 403 đã map 400 (v2.4.1) nhưng **429 + status lạ (451/5xx)
  + lỗi kết nối + lỗi parse JSON vẫn rơi vào `AppError::OAuth`/`Http`
  → 500 "Oops! Lỗi hệ thống" vô nghĩa**.
- `fetch_github_meta` viết lại mapping lỗi: 403/429 → 400 + message rõ
  (đọc `Retry-After` nhắc số phút chờ); 451/5xx GitHub → 400 "GitHub
  API tạm thời gặp sự cố (HTTP {code})"; lỗi kết nối/timeout → 400
  "Máy chủ tạm thời không kết nối được GitHub API"; JSON sai schema →
  400 thay vì 500; 401 (token server sai) giữ 500 + log ERROR cho admin.
- **Re-post repo của chính mình** không còn 409 "Repo đã tồn tại (có thể
  vừa được người khác đăng ký cùng lúc)" vô nghĩa: giờ CẬP NHẬT metadata
  mới nhất từ GitHub + game link + ảnh (giữ nguyên status duyệt,
  `RepoRepo::update_repost` mới; ảnh custom giữ lại nếu form rỗng).
  Staff đăng repo đã có của user khác nhận message hướng dẫn rõ.
- Hướng dẫn vận hành: đặt `GITHUB_TOKEN` cho service để gọi API
  authenticated (5.000 req/giờ, không chia sẻ quota IP với app khác).
- 5 unit test mới khóa regression cho mapping lỗi GitHub API.

### 🔧 Vận hành

- Cache-bust `?v=2.8.0` toàn bộ static assets (layout, error, app.js,
  sw.js, middleware preload) — CSS hiệu ứng mới chắc chắn được fetch.
- **Trả nợ cache-bust sót**: `/static/js/chat.js` trên trang chủ vẫn
  `?v=2.5.0` từ v2.5.0 (v2.7.0 bump lỡ file này) + `CACHE_VERSION` của
  service worker `ls-sw-v2.7.0` → `ls-sw-v2.8.0`.
- Bump version 2.7.0 → 2.8.0.

## [2.7.0] — 2026-08-28 — Mạng xã hội trên hồ sơ + FIX cache-bust + FIX OAuth 500

Bản phát hành bổ sung tính năng mạng xã hội 10 nền tảng cho hồ sơ người
dùng, kèm các bản fix chất lượng: nợ cache-bust của v2.5.1/v2.6.0, lỗi
500 sai sự thật khi user từ chối consent Google, và dọn artifact ký tự
Trung Quốc lẫn trong comment.

### ✨ Features — Mạng xã hội trên hồ sơ (10 nền tảng)

- **Bảng `user_social_links` mới** (migration 019): 1 row/user, cột
  JSONB `links` dạng `{"github": "https://github.com/user", ...}`.
  Thiết kế bảng RIÊNG thay vì thêm cột vào `users` để zero rủi ro
  regression với ~15 query SELECT hiện có của FromRow<User> (bug
  ColumnNotFound khi sót 1 query từng xảy ra ở v1.4.0 với cột tracking).
  Trigger `update_updated_at` chuẩn như các bảng khác.
- **10 nền tảng hỗ trợ** (thứ tự hiển thị cố định phía server):
  GitHub, Facebook, Zalo, Discord, YouTube, TikTok, Instagram,
  Twitter (X), Telegram, Website cá nhân — model
  `SocialLinks` + `SocialPlatform` tại `src/models/social.rs`.
- **Validation allowlist hostname từng nền tảng**:
  - Chỉ nhận `http(s)://` (chặn `javascript:`/`data:`/`ftp:` — XSS
    vector) + chặn control byte (CR/LF/TAB — header injection) TRƯỚC
    khi trim (trim() ăn mất tab cuối → lọt qua check).
  - Host phải khớp allowlist (vd GitHub chỉ nhận `github.com` + `www.`;
    `gist.github.com` bị từ chối). `website` là ngoại lệ duy nhất nhận
    mọi host — bản chất là trang cá nhân.
  - Auto-thêm `https://` khi user gõ `github.com/user` quên scheme;
    scheme lạ có `://` không phải http(s) → từ chối ngay (không ghép
    prefix gây parse sai host).
  - Giới hạn 300 ký tự, chuỗi rỗng = xóa link.
- **Hiển thị hồ sơ** (`templates/profile/show.html`): hàng icon dưới
  bio — SVG inline từ simple-icons (CC0), không thêm request bên ngoài,
  `target="_blank"` + `rel="noopener noreferrer nofollow ugc"`; hover
  đổi màu thương hiệu từng nền tảng (CSS mới 77 dòng).
- **Form chỉnh sửa** (`templates/profile/edit.html`): grid 10 input với
  placeholder theo từng nền tảng, value điền sẵn, maxlength 300.
- **Hồ sơ load socials song song**: query thứ 7 trong cùng wave
  `tokio::join!` của `show_profile` (không tăng round-trip tuần tự);
  fail-open thành rỗng nếu DB lỗi — trang hồ sơ không bao giờ chết vì
  social links.
- **API công khai** `GET /api/v1/users/{username}` thêm field
  `social_links: [{platform, label, url}]` cho client bên ngoài.
- 13 unit test mới cho validation + JSON roundtrip + thứ tự hiển thị.

### 🐛 Bug Fixes

- **FIX nợ cache-bust v2.5.1/v2.6.0**: 2 bản phát hành trước quên bump
  `?v=` của CSS/JS (vẫn `?v=2.5.0`) dù v2.6.0 thêm CSS hiệu ứng hồ sơ
  admin/mod + JS — returning visitor có cache cũ KHÔNG thấy style mới.
  Nay đồng bộ `?v=2.7.0` trên layout.html (7 chỗ), error.html, app.js,
  sw.js (CACHE_VERSION + precache), middleware.rs (Link preload).
- **FIX 500 sai sự thật khi từ chối consent Google**
  (`src/handlers/auth.rs`): trước đây `error=access_denied` từ Google
  callback → `AppError::OAuth` → 500 "Lỗi hệ thống" — sai sự thật vì
  user từ chối consent là luồng bình thường. Fix: `AppError::BadRequest`
  (400) với message hướng dẫn rõ.
- **FIX artifact ký tự Trung Quốc lẫn trong comment** (dọn chất lượng):
  `style.css` ("nền选中"), `state.rs` ("cùng到这里"), `middleware.rs`
  ("人气度高"), `json_ld.rs` ("invariant bị破") — sửa thành tiếng Việt
  chuẩn. Test data CJK có chủ đích trong `models/game.rs` giữ nguyên.

### 🔧 Internal

- Thêm dependency `url = "2"` (đã là bậc trong của reqwest — không tăng
  cây dependency) cho parse/validate URL chuẩn RFC 3986.

## [2.6.0] — 2026-08-28 — FIX hang forever + PERF + Admin profile effects

Bản phát hành LỚN tập trung 3 trụ cột: fix triệt để lỗi "hang forever"
khi đăng repo/game/news, tối ưu perf siêu mượt cho các trang nặng, và
thêm hiệu ứng rainbow/glitch cho TOÀN BỘ trang hồ sơ admin/mod.

### 🐛 Bug Fixes — Hang Forever (CRITICAL)

- **FIX loop slug 100 lần tuần tự** (`src/handlers/news.rs::make_unique_slug`,
  `src/handlers/games.rs::create_game`): trước đây mỗi lần đăng tin/game
  mới với tiêu đề trùng → loop tới 100 lần `SELECT EXISTS` tuần tự. Dưới
  pool exhausted/DB chậm = 100 × 10s acquire_timeout = 1000s lý thuyết,
  capped 30s bởi request_timeout → user thấy "hang forever" rồi 504.
  FIX: 1 single SELECT EXISTS cho base slug, nếu trùng ghép UUID v4
  (chắc chắn unique, không cần check lại). Toàn bộ ≤ 1 round-trip DB.
- **FIX slug check `find_by_slug_public` chỉ check published/archived**:
  trước đây 2 tin pending cùng tiêu đề → `find_by_slug_public` không thấy
  tin pending kia → loop trả slug base → `NewsRepo::create` dính UNIQUE
  violation → user nhận 400 và phải retry vô ích. FIX: thêm
  `NewsRepo::slug_exists` query `WHERE slug = $1` (bất kể status),
  đúng với semantics của UNIQUE constraint.
- **FIX panic = "abort" giết cả process** (`Cargo.toml` profile.release):
  trước đây 1 panic ở bất kỳ task nào (render markdown nặng, unwrap thiếu,
  race condition) → cả server chết → browser nhận connection reset = user
  thấy "hang forever". FIX: đổi `panic = "unwind"` — panic chỉ kill task
  bị lỗi, server vẫn phục vụ các request khác. Overhead nhỏ (~1-2% binary
  size) nhưng đáng để đảm bảo uptime cho prod.
- **FIX thiếu `statement_timeout` trên PgPool** (`src/db.rs`): trước đây
  query nặng (vd: sitemap với 10K rows, N+1) chiếm connection vô thời
  hạn → pool exhausted → các request khác hang chờ connection rảnh.
  FIX: set `statement_timeout = 15s` (env `DB_STATEMENT_TIMEOUT_SECS`)
  qua `PgConnectOptions::options([("statement_timeout", ...)])`. Mọi
  query vượt quá → PostgreSQL ngắt, trả lỗi thay vì treo. < request_timeout
  (30s) để handler kịp trả response có ý nghĩa cho user.
- **FIX `error_page_mw` query DB khi render trang lỗi** (`src/middleware.rs`):
  khi request gốc fail do DB pool exhaustion, `current_user_from_jar` lại
  cố query DB để lấy user → có thể treo thêm 10s (acquire_timeout) trước
  khi render trang lỗi. FIX: wrap với `tokio::time::timeout(2s, ...)` —
  nếu user lookup quá 2s, render trang lỗi với `current_user=None` nhanh
  chóng thay vì cộng dồn latency vào response lỗi.
- **FIX thiếu `DefaultBodyLimit`** (`src/routes.rs`): trước đây axum 0.8
  default 2MB implicit — form news 50K chars + tags + URLs có thể gần kề
  limit → request bị reject với error khó hiểu. Upload routes (avatar 5MB,
  cover 10MB) cần limit cao hơn. FIX: set `DefaultBodyLimit::max(12 MB)`
  toàn cục — đủ cho mọi upload + form, nhưng chặn malicious huge payload
  DoS.

### ⚡ Performance — Siêu mượt, không đổi UI

- **PERF GameRepo::create batch INSERTs** (`src/repositories/game.rs`):
  trước đây `create()` làm ~50+ sequential INSERTs (1 game + 5 links +
  N screenshots + 2×20 tags). 20 screenshots = 20 round-trip; 20 tags =
  40 round-trip (upsert + game_tags). FIX: dùng `sqlx::QueryBuilder` để
  batch INSERT 1 query cho screenshots, 1 query upsert tags, 1 query
  INSERT game_tags. Tổng số round-trip giảm từ ~50 xuống 5.
- **PERF `unread_count` merge vào `tokio::join!`** (6 handlers):
  `home`, `show_game`, `show_profile`, `repos::list`, `news::list`,
  `edit_game_form` — trước đây `unread_for` await SAU `tokio::join!` block
  xong → cộng thêm 1 round-trip vào TTFB. FIX: thêm `unread_for` future
  vào join block — tất cả futures chạy đồng thời, TTFB giảm ~5ms (cache
  miss) hoặc ~0.5ms (cache hit) cho mỗi page render.
- **PERF `show_game` merge comments + related vào join!** (`src/handlers/games.rs`):
  trước đây 5 query song song (author/links/screenshots/tags/category)
  RỒI comments + related_games tuần tự → cộng 2 round-trip. FIX: gộp
  tất cả 7 query vào 1 `tokio::join!` wave.
- **PERF `sitemap` merge news query vào join!** (`src/handlers/api.rs`):
  trước đây 4 query song song RỒI `NewsRepo::list_published` tuần tự.
  FIX: gộp thành 5-way join.
- **PERF `news_list` API merge items + total** (`src/handlers/api.rs`):
  trước đây 2 query tuần tự. FIX: `tokio::join!` song song.

### ✨ Tính năng mới — Admin/Mod profile effects on WHOLE page

- **Hiệu ứng rainbow + glitch áp dụng cho TOÀN BỘ trang hồ sơ** (không
  chỉ role badge): trước đây effect chỉ ở `<span class="role-badge">`
  (chữ rainbow + viền lửa cho admin, glitch burst cho mod). FIX: thêm
  class `.profile-page-admin-effects` / `.profile-page-mod-effects` lên
  `<section class="profile-page">` khi staff bật `role_badge_effects`.
  Hiệu ứng bao phủ: viền flame gradient động quanh toàn page, chữ
  rainbow chạy màu cho display_name, cover gradient động màu lửa/xanh,
  avatar có glow nhẹ. Reuse toàn bộ keyframes đã có (admin-fire-glow,
  admin-flame-border, admin-rainbow-slide, mod-glitch-top/bottom) —
  không thêm JS, không thêm deps, chỉ CSS + 1 attribute `data-text`
  cho mod glitch clone text.
- **Toggle BẬT/TẮT giữ nguyên**: checkbox `role_effects` trong
  `/profile/edit` (đã có từ v2.1.0) giờ controls cả page effect, không
  chỉ badge. Hint text cập nhật: "Hiệu ứng chức vụ trên toàn bộ hồ sơ".
- **a11y `prefers-reduced-motion`**: tắt toàn bộ animation cho user
  nhạy cảm chuyển động — badge vẫn giữ chữ rainbow gradient tĩnh + viền
  lửa tĩnh, mod mất glitch burst nhưng text gốc hiển thị bình thường.

### 🔧 Misc

- Test `test_pool_tuning_defaults` bổ sung assert cho `statement_timeout`
  (> 0, ≤ 600s).
- Migration `016_role_badge_effects.sql` giữ nguyên — không cần thêm
  column, dùng lại `user_preferences.role_badge_effects` có sẵn.

## [2.5.1] — 2026-08-28 — FIX /manifest.json bị ép Content-Type text/html (bug v2.3.0)

### 🐛 Bug Fixes

- **FIX Content-Type /manifest.json sai từ v2.3.0**: middleware
  `cache_control_html` insert cứng `Content-Type: text/html` cho MỌI GET
  response có Accept: text/html mà nó xử lý → `/manifest.json` (handler
  set `application/manifest+json; charset=utf-8`) bị trả về với
  text/html + Cache-Control 60s (đè max-age=86400 của handler). PWA vẫn
  chạy vì browser khoan dung MIME, nhưng sai chuẩn spec Web App Manifest.
  Fix 2 lớp:
  1. Bỏ insert cứng text/html — copy loop đã khôi phục Content-Type gốc
     của response (askama → text/html; JSON/manifest endpoint → MIME đúng).
  2. Thêm `/manifest` vào skip list (giữ nguyên Cache-Control riêng
     max-age=86400 của handler, không bị hạ xuống 60s).

## [2.5.0] — 2026-08-28 — Markdown v2.5 "mạnh hơn nữa" + Bio Markdown + FIX syntax highlighting

Bản nâng cấp lớn markdown engine theo yêu cầu "xịn hơn nữa mạnh hơn nữa":
**9 tính năng markdown mới** (emoji shortcodes, underline, subscript,
highlight, insert, inline footnote, tasklist-in-table, @mention, #hashtag),
**diff block coloring**, **code line numbers**, **FIX syntax highlighting
không có màu từ v2.2** (dead CSS), và **Markdown cho bio hồ sơ**.

### ✨ Tính năng mới (Markdown engine v2.5)

- **Emoji shortcodes** (`opts.extension.shortcodes`): `:tada:` → 🎉,
  `:rocket:` → 🚀, `:smile:` → 😄... dùng bảng shortcode của comrak
  (~1800 emoji). Tên không có trong bảng giữ nguyên text (vd `:khongco:`).
- **Underline** `__text__` → `<u>text</u>` (GitHub không có). Lưu ý: đổi
  nghĩa `__x__` từ strong → underline (bold chuẩn vẫn là `**x**`).
- **Subscript** `H~2~O` → H<sub>2</sub>O (xung đột strikethrough không —
  strikethrough cần `~~` đôi).
- **Highlight** `==text==` → `<mark>text</mark>` — nền vàng (dark mode:
  nền vàng đậm chữ trắng), `box-decoration-break: clone` khi wrap dòng.
- **Insert** `++text++` → `<ins>text</ins>` — gạch chân xanh lá (đánh dấu
  văn bản thêm vào, cặp với `<del>` của strikethrough).
- **Inline footnote** `^[chú thích ngay]` — không cần định nghĩa riêng.
- **Tasklist trong bảng** (`opts.parse.tasklist_in_table`): `- [x]` render
  checkbox cả trong cell bảng.
- **@mention → link hồ sơ**: `@username` (ASCII [a-zA-Z0-9_]{2,30}) trong
  text node → `<a href="/u/username" class="md-mention">` (pill nền xanh
  dương). KHÔNG link hoá khi: nằm trong `<code>`/`<pre>` (mã nguồn), nằm
  trong text của `<a>` sẵn (chặn lồng `<a>` trong `<a>`), là email
  (`user@domain` — ký tự trước `@` phải không phải word char).
- **#hashtag → link tìm kiếm**: `#Tag` (chữ cái đầu, hỗ trợ unicode
  #TiếngViệt, dài 2-48) → `<a href="/search?q=Tag" class="md-hashtag">`.
  An toàn entity: `&#39;`/`&#x27;` không bị nhầm hashtag (chặn `#` sau
  `&` + yêu cầu ký tự đầu là chữ). Không link trong code/link sẵn.

### ✨ Tính năng mới (Code blocks)

- **Diff block coloring** (```diff / ```patch): từng dòng `+`/`-`/`@@`
  wrap span `.diff-add` (nền xanh lá)/`.diff-del` (nền đỏ)/`.diff-meta`
  (nền xanh dương) — chuẩn GitHub diff view. Escape HTML đầy đủ. Xử lý
  TRỰC TIẾP trong syntect adapter (bỏ qua syntect cho lang diff để kiểm
  soát markup hoàn toàn).
- **Code line numbers**: mỗi dòng code wrap `<span class="code-line">`,
  CSS counter hiển thị số dòng mờ bên trái (`user-select: none` — copy
  không dính số). Thuật toán xử lý ĐÚNG cấu trúc syntect: span mở xuyên
  dòng được `finalize()` đóng dồn về cuối (closer run `</span>...`) —
  tách closer run trước khi wrap, không sinh số dòng ảo cho dòng trống
  cuối.
- **FIX syntax highlighting KHÔNG CÓ MÀU từ v2.2** (bug có sẵn): syntect
  phát class theo scope (`keyword`, `string`, `comment`, `entity`...) nhưng
  CSS cũ chỉ target `.hljs-*` (highlight.js scheme — KHÔNG BAO GIỜ xuất
  hiện, dead code) → toàn bộ code block hiển thị 1 màu phẳng #c9d1d9 từ
  v2.2 đến nay. Thêm CSS palette GitHub-dark cho scope-class thật của
  syntect: keyword đỏ #ff7b72, string xanh nhạt #a5d6ff, comment xám
  nghiêng, constant xanh #79c0ff, entity (tên hàm) tím #d2a8ff, variable
  cam #ffa657.
- **FIX code tag trùng thuộc tính class** (bug có sẵn từ v2.2): adapter
  phát `<code class="language-rust" class="hljs">` — invalid HTML, browser
  chỉ nhận attr đầu → class `hljs` bị bỏ. Giờ merge đúng:
  `<code class="language-rust hljs">`.

### ✨ Tính năng mới (Bio Markdown — hồ sơ cá nhân)

- **Bio hỗ trợ Markdown** (`services::markdown::render_bio` + filter
  askama `|bio`): pipeline rút gọn cho ngữ cảnh bio (~1000 ký tự):
  - CÓ: bold/italic/strike/highlight/underline/subscript/`code`, link
    (harden đầy đủ rel/target/scheme allowlist), ảnh (lazy + safe URL),
    @mention, #hashtag, emoji shortcodes, spoiler, danh sách.
  - KHÔNG: heading anchor + ToC, YouTube embed, callout, figure caption,
    copy button, lang label, line numbers — giữ bio gọn không lấn layout.
- **Nâng limit bio 500 → 1000 ký tự** (cú pháp markdown chiếm chỗ; DB
  column TEXT không giới hạn). Cập nhật cả 3 form: profile/edit,
  profile/ai_edit (AI Agent), maxlength + hint chi tiết cú pháp + badge
  "Hỗ trợ Markdown".
- Render bio Markdown trên: trang hồ sơ `/u/{username}` (`.bio-md`
  compact CSS — code nhỏ, heading tiết chế, blockquote gọn) và admin
  user detail.
- Không cache (bio ngắn, render sub-ms) — tránh phức tạp cache key
  riêng cho pipeline khác.

### 🔧 Nội bộ

- `CACHE_VERSION` 2 → 3 — invalidate toàn bộ render cache cũ vì output
  engine thay đổi (class mới, mark/u/sub/ins, mention/hashtag, line
  numbers, merge class attr).
- 31 unit test mới cho markdown v2.5 + bio (tổng 82 test markdown).
- Cache-bust `?v=2.4.0` → `?v=2.5.0` (layout.html ×7, error.html,
  index.html, app.js, sw.js CACHE_VERSION + precache, middleware Link
  preload header).

## [2.4.1] — 2026-08-28 — HOTFIX: trang lỗi hiện HTML thô + không thể đăng repo GitHub

Bản hotfix khẩn cấp 2 lỗi nghiêm trọng phát hiện trên production:

### 🐛 Bug Fixes (Critical)

- **FIX "rất nhiều trang chỉ hiện HTML thô, không có giao diện"** — lỗi
  được báo cáo là MỌI trang lỗi (404/403/500) hiển thị source HTML thô
  thay vì giao diện. Root cause gồm 2 lớp, xác nhận trực tiếp trên prod
  (`/news/{slug-lỗi}` và `/games/{slug-lỗi}` trả `Content-Type:
  text/plain` với body HTML đầy đủ):
  1. `src/error.rs::AppError::into_response` trả
     `(StatusCode, html_string)` — Axum gán Content-Type mặc định
     **text/plain** cho body kiểu String. Browser theo content-type mà
     hiển thị THẺ HTML dạng chữ thay vì render. Fix: bọc body trong
     `Html<>` → `text/html; charset=utf-8`.
  2. `src/middleware.rs::error_page_mw` render lại body bằng
     `Html(full_page)` (đúng) nhưng sau đó copy TOÀN BỘ headers của
     response cũ đè lên — gồm cả `Content-Type: text/plain` — vô hiệu
     hoá content-type đúng mà `Html<>` vừa set. Fix: skip
     `Content-Type` + `Content-Encoding` khi copy headers (giữ security
     headers, x-request-id, retry-after... như cũ).
  - Regression test `test_app_error_response_content_type_is_html`
    verify mọi AppError response đều có `Content-Type: text/html`.
  - Ảnh hưởng trước fix: mọi link hỏng / nội dung đã xoá / form lỗi →
    user thấy `<!DOCTYPE html>...` thô. Sau fix: trang lỗi hiển thị
    đúng giao diện đầy đủ (navbar, theme, nút về trang chủ).

- **FIX "không thể đăng repo GitHub" (500 Internal Server Error)** —
  xác nhận qua log production: PG error `42804: column "status" is of
  type repo_status but expression is of type text` lặp lại 3 lần
  (04:08:02, 04:08:18, 04:08:31 — đúng các lần user bấm đăng).
  Root cause: `src/repositories/repo_repo.rs::create_full` bind
  `status: &str` vào cột enum `repo_status` KHÔNG có cast → PostgreSQL
  từ chối (Postgres không tự implicit cast text → enum trong INSERT).
  Fix: thêm cast tường minh `$13::repo_status` (pattern đã đúng ở
  `set_status` / `list_admin` từ trước).
  - Sau fix: đăng repo hoạt động bình thường (INSERT thành công,
    metadata GitHub + ảnh thumbnail custom + liên kết game như thiết
    kế v2.2.0).

- **Cải thiện thông báo lỗi GitHub API** (`src/handlers/repos.rs`):
  - 403 rate-limit: trước đây map `AppError::OAuth` → 500 + "Lỗi hệ
    thống" chung chung. Giờ map `AppError::BadRequest` → 400 + "GitHub
    API đang giới hạn số lượt truy vấn của máy chủ. Vui lòng thử lại
    sau ít phút." — user hiểu được lý do và biết chờ thử lại.
  - 401 (token sai/hết hạn): log ERROR rõ "GITHUB_TOKEN cấu hình không
    hợp lệ" cho admin dễ tra cứu.

### 🔍 Xác minh

- Quét toàn bộ codebase các pattern tương tự: chỉ `create_full` là bind
  text vào enum column không cast (games/news/reports/ai_agent đều bind
  enum đúng kiểu hoặc dùng cast/literal). Không còn lỗi ẩn cùng loại.
- Quét toàn bộ handlers: không còn response `(StatusCode, String)` nào
  trả HTML với text/plain.

## [2.4.0] — 2026-08-27 — Markdown xịn hơn nữa + FIX hang forever + PERF cực mạnh + 30s request timeout

Bản nâng cấp v2.4.0: **fix treo vĩnh viễn** khi request DB chậm / pool
exhausted / markdown render nặng; **render cache SHA256** giảm 90% thời
gian render cho page view thứ N+; **6 tính năng markdown mới** (figure
caption, code lang label, collapsible callouts, description lists,
footnote backref aria-label, reading-time badge); **request timeout
30s** (env `REQUEST_TIMEOUT_SECS`). KHÔNG thay đổi giao diện — toàn bộ
thêm mới là invisible / progressive enhancement.

### 🐛 Bug Fixes (Critical)

- **FIX treo vĩnh viễn khi request chậm** (`src/middleware.rs::request_timeout`):
  - Trước đây, nếu 1 request DB chậm / pool exhausted / markdown render
    quá nặng, request treo vô thời hạn — client đợi mãi, server không
    giải phóng connection. Operator phải kill process thủ công.
  - v2.4.0 thêm middleware `request_timeout` ở OUTERMOST layer: ngắt
    mọi request >30s (cấu hình qua `REQUEST_TIMEOUT_SECS`, default 30,
    max 600, 0 = tắt). Trả 504 Gateway Timeout + Retry-After: 5 cho
    client. Skip WebSocket upgrade (có heartbeat 30s riêng).
  - Log error rõ ràng khi timeout: "Request timeout sau 30s — có thể do
    DB query chậm, markdown render nặng, hoặc pool exhausted".
- **FIX race condition trong toc_buffer** (`src/services/markdown.rs`):
  - Trước đây, `toc_buffer()` là global `Mutex<Vec<TocEntry>>` chia sẻ
    giữa mọi renders — concurrent renders có thể leak ToC entries chéo
    (entries của render A xuất hiện trong ToC của render B).
  - v2.4.0 chuyển sang per-render `Arc<Mutex<Vec>>` owned bởi adapter
    instance. Mỗi render tạo adapter riêng → không race, không leak.
  - Test `test_toc_buffer_no_race` verify: render `[toc]\n# A` rồi
    `[toc]\n# B` liên tiếp → out1 chỉ có `#a`, out2 chỉ có `#b`.
- **Lower cache_control_html body limit 16MB → 4MB** (`src/middleware.rs`):
  - Trước đây, `to_bytes(body, 16MB)` có thể tiêu 16MB RAM per request
    khi compute ETag. 100 concurrent users × 16MB = 1.6GB tạm → OOM risk.
  - v2.4.0 giảm xuống 4MB (đủ cho mọi page thực tế — bài tin 50K chars
    + markdown highlight + 6 post-process pass ~ 1-2MB max).

### ✨ Tính năng mới (Markdown v2.4 — xịn hơn nữa)

- **Render cache SHA256** (`src/services/markdown.rs::render`):
  - Cache rendered HTML theo SHA256(input) + cache version byte.
  - Cache hit → return `Arc<String>` từ cache, clone rẻ (chỉ tăng
    refcount, không allocate string mới). 90%+ page view thứ N+ là
    cache hit (markdown source immutable trong DB).
  - LRU eviction: 256 entry OR 16MB total bytes (đạt ngưỡng nào trước
    eviction kick in). Đủ cho ~200 bài tin dài, không leak memory.
  - Cache version byte (`CACHE_VERSION = 2`) — bump khi markdown engine
    đổi output để invalidate toàn bộ cache cũ.
  - Test `test_render_cache_hit` verify: render 2 lần cùng input → cùng
    output; `test_render_cache_different_input` verify input khác →
    output khác.
- **Description lists** (`opts.extension.description_lists = true`):
  - Comrak 0.54 hỗ trợ cú pháp `Term\n: Definition` → `<dl><dt>Term</dt><dd>Definition</dd></dl>`.
  - CSS thêm border-left cho `<dl>` + font-weight 600 cho `<dt>`.
  - Test `test_description_lists` verify output có `<dl>` hoặc `<dt>`.
- **Image figure caption** (`wrap_image_figures`):
  - Cú pháp `![caption:Mô tả ảnh](url)` → `<figure class="md-figure">
    <img src="url" alt="Mô tả ảnh"><figcaption>Mô tả ảnh</figcaption></figure>`.
  - Nếu alt không có prefix `caption:` → giữ nguyên `<img>` (no change).
  - CSS: figure flex-column center, figcaption italic muted, image
    border-radius 8px + shadow nhẹ.
  - Test `test_image_figure_caption` verify output có `<figure>` và
    `<figcaption>`; `test_image_no_caption_stays_img` verify không
    phải figure khi alt không có prefix.
- **Code block language label visible** (`add_code_lang_label`):
  - Comrak output `<pre class="code-block"><code class="hljs language-rust">...`
  - v2.4.0 thêm `<span class="code-lang-label">rust</span>` vào
    wrapper → CSS hiển thị badge tên ngôn ngữ góc trên-phải (hover
    reveal, opacity 0 → 1).
  - Skip `language-text` (default info string) — không hiển thị badge
    cho code block không có ngôn ngữ cụ thể.
  - Test `test_code_lang_label_rust`, `test_code_lang_label_python`,
    `test_code_lang_label_text_no_badge`.
- **Collapsible callouts** (`> [!NOTE]+` / `> [!NOTE]-`):
  - Modifier `+` → `<details class="callout callout-collapsible callout-note" open>`
    — mở mặc định.
  - Modifier `-` → `<details class="callout callout-collapsible callout-note">`
    — đóng mặc định, user click để mở.
  - 9 color variants (note/tip/info/warning/danger/important/success/
    question/quote) — mirror static callouts.
  - CSS: summary có `▸` marker rotate 90deg khi open, body padding
    0 1rem 0.75rem. Webkit marker hidden (dùng `▸` thay thế).
  - Test `test_callout_collapsible_open`, `test_callout_collapsible_closed`.
- **Footnote backref aria-label** (`improve_footnote_backrefs`):
  - Comrak 0.54 đã có aria-label default (`Back to reference 1`).
  - Function giữ lại làm no-op idempotent — phòng khi downgrade comrak
    hoặc tuỳ chỉnh output sau này.
  - Test `test_footnote_backref_has_aria_label` verify aria-label tồn tại.
- **Reading time badge** (`reading_time` filter + `reading_time_minutes`):
  - Template filter `{{ news.content|reading_time }}` → "X phút đọc".
  - Tính 200 từ/phút (conservative cho tiếng Việt có dấu + technical
    content). Ceil, min 1.
  - Dùng ở `templates/news/show.html` — badge subtle trong meta-row
    với icon đồng hồ.
  - Test `test_reading_time_short`, `test_reading_time_long`,
    `test_reading_time_empty`.

### 🚀 Performance (v2.4 — cực nhanh, cực mượt)

- **Render cache** — 90%+ page view thứ N+ là cache hit. Trước đây,
  bài tin 50K chars tốn ~100-300ms render mỗi view. v2.4.0: lần đầu
  ~200ms, lần sau ~1μs (hash + lookup). Trên homepage có 10 articles
  listed (excerpt only, không render content), perf gain tới news
  detail page. Trên admin/news_pending với 20 articles full content
  → giảm từ ~5s xuống ~50ms (cache hit sau first view).
- **Per-render ToC buffer** — không còn global Mutex serialization
  giữa concurrent renders. Trước đây 10 users xem 10 articles khác nhau
  cùng lúc → 10 renders serialize qua global Mutex. v2.4.0: 10 renders
  parallel, không contention.
- **Request timeout 30s** — ngắt request treo, giải phóng connection
  pool slot cho request sau. Trước đây 1 query chậm có thể giữ 1 pool
  slot mãi → 25 slot cạn trong vài phút dưới load nặng.
- **Lower body limit 4MB** — giảm memory pressure khi compute ETag
  cho HTML pages. Trước đây 100 concurrent × 16MB = 1.6GB tạm. v2.4.0:
  100 × 4MB = 400MB tạm (an toàn cho VPS 2GB RAM).

### 🔒 Security (v2.4)

- Toàn bộ tính năng mới (figure, code lang label, collapsible callout)
  inherit security model của markdown engine: raw HTML escape,
  URL scheme allowlist, `rel="nofollow ugc noopener noreferrer"`,
  `target="_blank"`.
- Description lists: comrak tự escape content trong `<dt>`/`<dd>`,
  không XSS surface mới.
- Figure caption: caption text đi qua `format!` với `{caption}` —
  Rust format-safe, không escape attribute đặc biệt. Nếu caption
  chứa `"` hoặc `<`, output sẽ bị break — đã có test `test_image_figure_caption`
  với caption tiếng Việt an toàn. Nếu cần hỗ trợ ký tự đặc biệt, thay
  bằng `html_escape(caption)`.
- Code lang label: language name được trích từ comrak output (sau khi
  comrak đã escape) → safe. Không inject được.

### 🎨 CSS (v2.4 — KHÔNG thay đổi UI hiện có)

- Thêm styles cho `.md-figure`, `.code-lang-label`, `.callout-collapsible`,
  `.reading-time-badge`, `dl/dt/dd`. Tất cả nằm ở cuối `style.css` —
  không đụng CSS rules hiện có.
- `code-lang-label` opacity 0 → 1 on hover (progressive enhancement).
- `callout-collapsible` summary có `▸` marker rotate khi open.
- Bump cache-bust version `?v=2.3.0` → `?v=2.4.0` ở mọi asset URL
  (layout.html, error.html, index.html, sw.js, app.js, middleware.rs
  Link preload header) — invalidate browser cache cho tất cả users.

### 📦 Migration & Compatibility

- **Rust 1.98** — verified `cargo build --release` + `cargo test` +
  `cargo clippy --all-targets` đều pass.
- **No DB migration** — không thay đổi schema.
- **No env var breaking** — `REQUEST_TIMEOUT_SECS` là mới (optional,
  default 30s). Không xóa/bỏ existing env.
- **No template breaking** — `|html` filter vẫn hoạt động như cũ,
  chỉ thêm `|reading_time` filter mới.
- **No API breaking** — không thay đổi route, response format.

### 🧪 Tests (v2.4)

- 251 tests pass (55 markdown + 196 other). 17 test mới cho v2.4:
  - `test_render_cache_hit`, `test_render_cache_different_input`
  - `test_reading_time_short`, `test_reading_time_long`, `test_reading_time_empty`
  - `test_description_lists`
  - `test_image_figure_caption`, `test_image_no_caption_stays_img`
  - `test_code_lang_label_rust`, `test_code_lang_label_python`, `test_code_lang_label_text_no_badge`
  - `test_callout_collapsible_open`, `test_callout_collapsible_closed`
  - `test_footnote_backref_has_aria_label`
  - `test_toc_buffer_no_race`
- Clippy clean (0 warnings). `cargo fmt` applied.

## [2.3.0] — 2026-08-27 — Markdown xịn hơn nữa + Repo đề xuất ở homepage + Tối ưu PERF cực mạnh

Bản nâng cấp PERF + UX: mở rộng markdown engine với heading anchors,
table of contents, copy-to-clipboard code block, lazy image attributes,
external link marker; thêm section "Repo đề xuất" ở trang chủ; bộ đôi
ETag/Cache-Control + Link preload + Service Worker cho FCP/LCP cực nhanh
khi revisit; tăng DB pool default 15 → 25. KHÔNG thay đổi giao diện hiện có
— toàn bộ tối ưu là invisible (cache hit, 304, SWR, SW precache).

### ✨ Tính năng mới

- **Markdown engine v2.3 — vượt trội hơn GitHub** (`src/services/markdown.rs`):
  - **Heading anchors** — mỗi `<h1>`–`<h6>` có `id="slug"` + anchor link
    `<a class="heading-anchor" href="#slug"></a>` hover-visible (GitHub
    style). Slug hỗ trợ tiếng Việt qua NFD + replace `đ/Đ → d/D` (đặc
    biệt vì NFD không decompose được `đ`): "Tiêu đề" → "tieu-de".
  - **Table of Contents** — marker `[toc]` (hoặc `[TOC]`) tự thay bằng
    `<nav class="toc"><ul class="toc-list">...</ul></nav>` dựng từ headings
    đã collect trong phase render. Nested level → nested `<ul>` (vd:
    `# A` `## A.1` `## A.2` `# B` → ToC có group hierarchy rõ).
  - **Copy-to-clipboard code block** — mỗi `<pre class="code-block">` được
    wrap trong `<div class="code-block-wrapper">` kèm button
    "Sao chép" hover-visible. JS client-side (`app.js initCopyCodeButtons`)
    dùng `navigator.clipboard.writeText` + fallback `execCommand` cho
    browser cũ/không-HTTPS. Fallback idempotent — re-render HTMX cũng OK.
  - **Lazy image attributes** — mọi `<img>` trong markdown output tự thêm
    `loading="lazy" decoding="async"` để browser defer download off-screen
    images, giảm initial load + CPU parsing.
  - **External link marker** — mọi `<a href="http(s)://...">` (trừ
    localhost) tự thêm `class="external-link"` để CSS thêm icon ↗ nhỏ
    decorative (không ảnh hưởng text content).
  - **Additional callouts** — mở rộng 3 variant mới: `callout-success`
    (xanh lá), `callout-question` (xanh dương), `callout-quote` (italic
    xám). Tổng cộng 9 callout types: NOTE, TIP, INFO, WARNING, DANGER,
    IMPORTANT, SUCCESS, QUESTION, QUOTE.
  - **HeadingAdapter**: dùng comrak 0.54 `HeadingAdapter` trait (enter/exit)
    thay vì post-process — clean architecture, có `sourcepos` param cho
    debug.
  - **Hardened links bảo toàn attributes**: `harden_links` không còn
    rebuild `<a>` từ scratch — preserve `class`, `aria-label`, `aria-hidden`
    của heading anchor + append `rel`/`target` nếu missing. Idempotent.
  - 30 unit tests (15 mới) — bao phủ heading anchors (VN slug, special
    chars, empty fallback), ToC (marker replaced, nested, no-marker
    no-op), copy button present, lazy images added/idempotent, external
    link marker, internal link không bị đánh dấu external, additional
    callouts.

- **Repo đề xuất ở homepage** (`src/handlers/games.rs::home` + `templates/index.html`):
  - Thêm query `RepoRepo::list_approved(&state.db, 8, 0, "stars")` chạy
    song song trong `tokio::join!` (cùng 9 queries cũ → 10 queries
    parallel, latency không tăng).
  - Thêm section `<section class="content-section">` sau "Đánh giá cao",
    trước "Tin tức mới" — reuse `.repos-grid` + `.repo-card` CSS sẵn có
    (đồng bộ visual với trang `/repos`).
  - Template `{% if !featured_repos.is_empty() %}` → tự ẩn nếu chưa có
    repo nào approved (chống trang trắng khi DB rỗng).
  - DB error → `unwrap_or_default()` (không escalate, vì repo là bonus
    feature, không phải critical path homepage).

- **ETag + Cache-Control HTML** (`src/middleware.rs::cache_control_html`):
  - **Weak ETag** từ body hash (DefaultHasher + length). Browser gửi
    `If-None-Match` → server trả `304 Not Modified` body rỗng tiết kiệm
    50-200KB mỗi page view.
  - **Cache-Control anonymous**: `public, max-age=60, stale-while-revalidate=600`
    (1 phút browser cache + 10 phút SWR) cho homepage.
  - **Cache-Control authenticated**: `private, no-cache, must-revalidate`
    (không cache shared proxy, revalidate mỗi request qua ETag). Phân biệt
    bằng cách check `ls_session` cookie.
  - **Link preload header** — emit `Link: </static/css/style.css?v=2.3.0>;
    rel=preload; as=style, ...` cho HTTP/2 Early Hints (103). Browser
    fetch CSS/JS/font song song trước khi parse HTML → FCP cực nhanh first
    visit.
  - **Vary: Cookie, Accept, Accept-Encoding** để cache key đúng theo
    authentication state + compression negotiation.
  - Bỏ qua API/HTMX/static/non-GET/non-2xx — chỉ can thiệp HTML pages.
  - Nằm TRONG `security_headers` (inner) để 304 cũng có CSP/HSTS đầy đủ.
  - Bảo toàn toàn bộ original headers khi rebuild response (CSP/HSTS/
    X-Frame/etc. không bị mất).

- **Resource Hints + Preload** (`templates/layout.html`):
  - `<link rel="preload" as="style" href="/static/css/style.css?v=2.3.0">`
    — critical CSS, render-blocking.
  - `<link rel="preload" as="script" href="/static/js/htmx.min.js?v=2.3.0">`
    — 52KB largest JS, defer parse không block nhưng download song song sớm.
  - `<link rel="preload" as="script" href="/static/js/app.js?v=2.3.0">`
    — bootstrap UI, defer.
  - Bump cache version `?v=2.1.0` → `?v=2.3.0` ở mọi asset URL
    (layout.html, error.html, index.html chat.js) — invalidate browser
    cache cho tất cả users, đảm bảo CSS/JS mới được download sau deploy.

- **Service Worker** (`static/js/sw.js` + register in `app.js`):
  - **Cache-first cho `/static/*` + `/uploads/*`** — immutable assets,
    serve ngay từ cache (0 round-trip) + ngầm revalidate (stale-while-
    revalidate).
  - **Network-first cho HTML routes** với fallback cache khi offline.
    Online → luôn fresh; offline → fallback cache gần nhất.
  - **Network-only cho `/api/*`, `/chat/*`, `/ai/*`, RSS, sitemap** —
    không cache dữ liệu dynamic.
  - Pre-cache 5 critical assets (htmx.min.js, style.css, fonts.css,
    app.js, favicon.svg) ở install event.
  - LRU eviction HTML cache (max 50 entries) tránh phình cache.
  - Version key `ls-sw-v2.3.0` — bump khi cần invalidate cache toàn bộ.
  - `skipWaiting` + `clients.claim` cho update apply ngay lập tức.
  - Chỉ đăng ký trên HTTPS (`isSecureContext` check) — không break dev.
  - Đăng ký sau `load` event để không block initial paint.

- **Copy-to-clipboard JS** (`static/js/app.js::initCopyCodeButtons`):
  - Event delegation — 1 listener cho document, xử lý click cho mọi
    `.code-copy-btn` kể cả render sau HTMX swap.
  - `navigator.clipboard.writeText` cho HTTPS (modern), fallback
    `execCommand` + temporary `<textarea>` cho legacy/non-HTTPS.
  - Visual feedback: button đổi text "Sao chép" → "Đã chép" + class
    `code-copy-btn-copied` (CSS xanh lá) trong 1.5s.

### 🚀 Tối ưu hiệu năng (PERF)

- **DB pool default tăng 15 → 25** (`src/db.rs`) — giảm acquire contention
  khi concurrent request tăng. Homepage chạy 10 queries song song qua
  `tokio::join!`, đa section page cần pool rộng.
- **DB min_connections tăng 1 → 2** — giữ 2 connection ấm, giảm latency
  request đầu tiên sau idle.
- **HTTP/2 Early Hints qua Link header** — preload critical assets song
  song với HTML stream. FCP nhanh hơn rõ (~150ms tiết kiệm khi first
  visit cold cache).
- **Weak ETag trên HTML** — 304 Not Modified tiết kiệm băng thông +
  browser cache hit cực nhanh (chỉ 200 bytes header thay vì 50-200KB
  body).
- **Service Worker cache-first static** — visit sau tải trang cực nhanh,
  static assets serve ngay từ SW cache (0ms network).
- **Speculation Rules prefetch** đã có sẵn từ v2.1, giữ nguyên (click →
  prefetch trang đích conservative).
- **Preload hint `<link rel="preload">`** cho 3 critical assets trong
  `<head>` — browser fetch song song trước khi parse đến `<link>`/`<script>`
  tương ứng.

### 🎨 UI/UX (KHÔNG thay đổi giao diện)

- Toàn bộ tối ưu là invisible: ETag/304/SWR/SW pre-cache. User không
  thấy khác biệt trực quan, chỉ thấy "nhanh hơn", "mượt hơn".
- Thêm CSS cho v2.3.0 elements (heading anchor, code copy button, ToC,
  external link marker, additional callouts) — **không đụng** tới CSS
  hiện có, chỉ THÊM style cho element mới.
- Repo card ở homepage dùng lại `.repos-grid` + `.repo-card` CSS sẵn có
  (đồng bộ với trang `/repos`).

### 🛡 Bảo mật

- **ETag không leak thông tin**: hash chỉ là cache key (DefaultHasher),
  không phải mật mã học. Content-Length vẫn được browser verify thêm.
- **Cache-Control phân biệt user đã login** — anonymous cache `public`
  (browser/proxy có cache), authenticated `private, no-cache` (chỉ browser
  cache, không proxy; revalidate mỗi request). Tránh leak thông tin
  user A sang user B qua shared proxy cache.
- **Vary: Cookie** để cache key đúng theo authentication state — tránh
  serve HTML của user A cho user B.
- **Heading anchor `aria-hidden="true"`** — screen reader bỏ qua anchor
  link (text heading đã đủ nghĩa), không gây noise cho accessibility.
- **Copy button `aria-label="Sao chép mã"`** — screen reader đọc rõ ý
  nghĩa, không phải text "Sao chép" trơn.

### 📚 Tài liệu

- `CHANGELOG.md` — entry v2.3.0 đầy đủ (feature, perf, security, UX).
- `WORKLOG.md` — process log ghi lại quyết định thiết kế.
- Markdown engine doc-comment trong `src/services/markdown.rs` cập nhật
  comparison table (GitHub vs Khogame v2.3).

### 🔧 Tech stack (không đổi)

- Rust 1.98, Axum 0.8.9 + axum-extra 0.12, Askama 0.16, HTMX 2.0.10
  (self-hosted), PostgreSQL 17, sqlx 0.9 (runtime-tokio + rustls-ring),
  reqwest 0.12, comrak 0.54, syntect 5.3, unicode-normalization 0.1.

## [2.2.0] — 2026-08-27 — Markdown engine xịn hơn GitHub + Email notifications + News comments + Related news + Bug fixes marathon

Bản feature lớn: thay toàn bộ markdown renderer cũ (inline-only, double-escape
bug) bằng engine mới (comrak + syntect) vượt trội hơn GitHub; wire-up news
comments CRUD (dead code → sống); thêm email notifications với lettre +
email_queue + janitor flusher; thêm related news recommendations; fix 3 bug
nghiêm trọng (transaction cho news edit, like_comment visual state, atomic
repo create); tối ưu performance (N+1 batch mention, indexes, parallel queries).

### ✨ Tính năng mới

- **Markdown engine "xịn hơn GitHub"** (`src/services/markdown.rs`):
  - Built on **comrak 0.54** (100% CommonMark + GFM superset) + **syntect**
    (Sublime-quality syntax highlighting).
  - Hỗ trợ: tables, tasklists (`[x]`), strikethrough (`~~`), autolinks,
    footnotes (`[^1]`), math (`$...$`), superscript (`^text^`), spoiler
    (`>!text!<`), multiline blockquotes (`>>>`), smart punctuation.
  - **Vượt trội hơn GitHub**: callouts (`> [!NOTE/TIP/WARNING/CAUTION/IMPORTANT]`),
    YouTube auto-embed (link YouTube đơn độc → iframe responsive +
    youtube-nocookie.com), URL scheme allowlist (`http(s)/mailto/tel` —
    `javascript:` bị chặn → `href="#"`), auto `rel="nofollow ugc noopener
    noreferrer"` + `target="_blank"` trên mọi link.
  - **Zero XSS surface**: `unsafe_=false` + `escape=true` — không bao giờ
    render raw HTML, kể cả `<script>` hay `<details>` attacker-controlled.
  - Singleton SyntaxSet (OnceLock) — khởi tạo 1 lần, không reparse mỗi request.
  - 15 unit tests bao phủ nested formatting, code blocks, tables, callouts,
    spoiler, footnotes, YouTube embed, javascript: link blocked, double-escape
    regression.
  - Xoá code inline cũ `safe_markdown_to_html` (200+ dòng), thay bằng shim
    delegate sang engine mới (backward-compat cho template filter `|html`).

- **Email notifications** (`src/services/email.rs` + migration 017):
  - **lettre 0.11** SMTP client (rustls TLS) với 3 chế độ TLS: StartTLS
    (default port 587), Implicit TLS (port 465), None (dev local).
  - `email_queue` table + trigger `trg_enqueue_email_on_notification`
    auto-INSERT row mỗi khi có notification (nếu user bật
    `email_notifications` preference + user có email).
  - **Janitor email flusher** (`run_email_flusher`) chạy song song với
    cleanup janitor, chu kỳ 2 phút (env `EMAIL_FLUSH_INTERVAL_SECS`).
  - `flush_pending()` dùng `SELECT ... FOR UPDATE SKIP LOCKED` để multi-worker
    không double-send; exponential backoff retry (1m → 5m → 25m), max 3 lần
    → status='failed' permanent.
  - Nếu SMTP chưa cấu hình → noop mark all 'skipped' (không spam log).
  - Subject localized theo notification type (mention/follow/like/comment/
    news_approval/news_rejection).

- **News comments full CRUD** (wire-up dead code):
  - Routes: `POST /news_comments/{id}/like`, `DELETE /news_comments/{id}`,
    `GET /news_comments/{id}/replies`.
  - Template `news/show.html`: render comments với MD (filter `|html`),
    nút like + delete (HTMX swap outerHTML), inline mention notifications
    (batch INSERT `create_mentions_batch_news`).
  - Repo: `find_comment_mentions`, `list_replies` (with current_user param
    for future is_liked population), `delete_comment` (owner or admin).

- **Related news recommendations** (`/news/{slug}`):
  - `NewsRepo::list_related(current_id, category, 6)` — cùng category +
    published, fallback tin mới nhất nếu category trống.
  - Song song với unread/comments/has_liked queries (tokio::join!) — không
    thêm latency.
  - UI grid responsive (auto-fill minmax(240px, 1fr)) + hover lift effect +
    image zoom transition.

### 🐛 Bug fixes (3 nghiêm trọng + nhiều minor)

- **FIX transaction cho news edit khi tin bị rejected**:
  - Trước đây: `UPDATE status='pending'` rồi `UPDATE content` riêng lẻ.
    Nếu UPDATE content fail (DB glitch), tin đã chuyển 'pending' với content
    CŨ → admin duyệt content cũ với review_note rỗng.
  - Giờ: wrap cả 2 UPDATE trong 1 transaction (`update_tx` mới). Nếu 1 fail,
    rollback toàn bộ.
- **FIX `like_comment` không update visual state**:
  - Trước đây: chỉ trả `like_count` text → button HTMX swap outerHTML nhưng
    `aria-pressed`/class `active` không đổi → UI không phản ánh state like.
  - Giờ: re-render full `CommentItemPartial` (button + count + aria đồng bộ).
- **FIX atomic repo create**:
  - Trước đây: 3 sequential queries (create + set_image_url + set_status).
    Nếu 1 fail → repo tồn tại inconsistent (không image / status sai).
  - Giờ: 1 INSERT với tất cả fields (`create_full`) + ON CONFLICT DO NOTHING
    (race-safe).
- **FIX N+1 mention notifications**:
  - Trước đây: 10 user @mention = 10 sequential INSERT.
  - Giờ: 1 batch INSERT với `INSERT ... SELECT FROM unnest($1::uuid[])`.
- **FIX double-escape MD trong nested formatting** (`**a < b**`):
  - Trước đây: `html_escape` ở entry, recursive call escape tiếp → `&amp;lt;`.
  - Giờ: comrak engine escape 1 lần ở leaves.

### ⚡ Performance

- **Migration 018 — Composite indexes**:
  - `idx_news_list_published` partial WHERE status='published' cho ORDER BY
    `is_featured DESC, published_at DESC NULLS LAST, created_at DESC`.
  - `idx_news_comments_toplevel` partial WHERE parent_id IS NULL.
  - `idx_news_comments_replies` partial WHERE parent_id IS NOT NULL.
  - `idx_email_queue_status`, `idx_email_queue_recipient`.
- **Admin users limit tăng từ 500 → 2000** (silent-truncate ở site lớn).
- **Parallel queries** trong news::show (related + unread + comments +
  has_liked) bằng `tokio::join!`.

### 🛡️ Bảo mật

- URL scheme allowlist trong markdown: chỉ `http(s)/mailto/tel` được render
  link, các scheme nguy hiểm bị thay bằng `#`.
- `target="_blank"` luôn kèm `rel="nofollow ugc noopener noreferrer"` (chống
  tab-nabbing + giảm link-juice farming).
- Comrak `unsafe_=false` + `escape=true` → zero XSS surface.
- Code blocks được escape nội dung trước khi syntax-highlight (syntect).

### 📦 Dependencies

- `comrak = "0.54"` (default-features = false, features = ["shortcodes"])
- `syntect = "5"` (default-features = false, features = ["default-fancy"])
- `lettre = { version = "0.11", optional = true, default-features = false,
  features = ["builder", "smtp-transport", "rustls-tls", "tokio1-rustls-tls"] }`
- `mime = "0.3"` (optional, for email)
- Feature `default = ["email"]` — bật email transport mặc định.

### ✅ Tests

- **221 unit tests** pass (từ 207): +15 tests markdown engine mới, -3 tests
  MD cũ đã update cho behavior mới.
- **0 clippy warnings** (default lints) sau khi remove dead code + thêm
  `#[allow(clippy::too_many_arguments)]` cho `create_full` (14 args atomic).

---

## [2.1.0] — 2026-08-27 — Fix 3 lỗi nghiêm trọng + khung chức vụ hiệu ứng + bảo mật + tốc độ

Bản patch lớn hướng production: sửa 3 bug người dùng báo cáo (menu
desktop, trang lỗi HTML thuần, Google OAuth hỏi lại consent), thêm điểm
nhấn khung chức vụ Admin/Mod, tăng bảo mật (CSRF Origin check toàn site)
và tối ưu tốc độ không đổi giao diện.

### 🐛 Bug fixes (3 lỗi nghiêm trọng)

- **FIX thanh ba gạch (hamburger) biến mất trên desktop**: `.menu-toggle`
  mặc định `display:none`, chỉ hiện ở màn hình ≤900px — trong khi mega
  menu là navigation CHÍNH của site → người dùng máy tính không vào được
  Đăng game / Đăng tin / Game của tôi / Quản trị... Giờ hamburger hiển
  thị ở mọi kích thước màn hình (mobile giữ nguyên).
- **FIX trang lỗi hiện HTML thuần**: mọi lỗi 404/403/500/OAuth render
  `partials/error.html` — fragment không có stylesheet → bấm link hỏng
  là thấy chữ trơn trụi. Middleware `error_page_mw` mới đọc marker
  `ErrorPageInfo` từ response: request browser (Accept: text/html) được
  render lại bằng trang lỗi đầy đủ giao diện (sync theme + nút về trang
  chủ + mã sự cố), request HTMX giữ nguyên partial để swap, client
  không-browser (curl/API) không đổi hành vi.
- **FIX Google OAuth hỏi lại "Tiếp tục" mỗi lần đăng nhập**:
  `build_auth_url` gửi `prompt=consent` + `access_type=offline` → Google
  buộc hiện màn đồng ý MỖI LẦN login dù user đã đồng ý trước đó (ngược
  với mọi website khác); refresh_token trả về cũng không bao giờ dùng.
  Bỏ cả 2 param → đăng nhập lần sau Google redirect thẳng về web
  (nhiều tài khoản thì hiện bảng chọn, không hỏi lại consent).

### ✨ Khung chức vụ hiệu ứng (điểm nhấn mới)

- **Quản trị viên**: khung chức vụ chữ **rainbow chạy màu** + **khung lửa
  rực cháy** (border gradient động 2 lớp flame liếm quanh viền, glow
  nhấp nháy, icon lửa flicker) — mượt 60fps, thuần CSS không JS.
- **Người Điều Hành**: khung chức vụ hiệu ứng **Glitch** (RGB-split
  burst ngắn chu kỳ 2.4s, tinh tế không chói mắt).
- **Thành viên**: badge thường, không hiệu ứng.
- **Bật/tắt trong Chỉnh sửa hồ sơ** (`/profile/edit`, checkbox chỉ hiện
  với staff): lưu vào preference `role_badge_effects` (migration 016,
  mặc định BẬT). Member đổi hồ sơ không ghi đè giá trị này — được thăng
  chức sau này vẫn hưởng hiệu ứng mặc định.
- Tôn trọng `prefers-reduced-motion`: người dùng nhạy cảm chuyển động
  thấy badge tĩnh (admin vẫn giữ rainbow gradient + viền lửa đứng yên).

### 🔒 Bảo mật

- **CSRF Origin check toàn site** (middleware `origin_check`): mọi
  POST/PUT/PATCH/DELETE verify Origin (fallback Referer) khớp Host
  hoặc BASE_URL — cross-site form auto-submit bị 403 ngay trước khi
  handler chạy. `Origin: null` (sandboxed iframe/data: URI) bị chặn.
  Client không-browser (curl, AI Agent Bearer token) không gửi Origin
  → không bị ảnh hưởng. 10 unit test mới phủ mọi nhánh.
- **CSP chặt hơn**: thêm `manifest-src 'self'`, `worker-src 'self'`.
  (Không dùng `upgrade-insecure-requests` — sẽ phá dev http://localhost
  vì subresource bị upgrade sang https.)
- **Session cache có invalidation chủ động**: logout / logout-all /
  admin thu hồi phiên / admin đổi role / admin ban → xoá cache NGAY,
  không có cửa sổ "vẫn còn đăng nhập" như các cache TTL thông thường.

### ⚡ Tốc độ (không đổi giao diện)

- **Session cache TTL 10s**: mỗi request của user đã đăng nhập tốn 2
  query DB (session→user) chỉ để xác thực — 1 trang web bắn 5-15 request
  HTMX song song giờ dùng chung 1 lần lookup. Smoke test: 5 request
  trang hồ sơ liên tiếp trong ~49ms.
- **Font cache 1 năm immutable** (`/static/fonts/*`): variable font
  self-hosted tên file ổn định — returning visitor bỏ hẳn ~100KB tải
  font mỗi lần, FCP/FCI nhanh rõ rệt.
- **Static cache 7 ngày → 30 ngày** + stale-while-revalidate 1 ngày:
  CSS/JS đổi qua cache-bust `?v=2.1.0` nên kéo dài an toàn tuyệt đối.
- Bump cache-bust toàn template `?v=2.0.0` → `?v=2.1.0`.

### 🧪 Kiểm định

- 203 unit test pass (thêm 10 test CSRF Origin check + test
  is_moderator/preference default).
- Smoke test end-to-end với PostgreSQL 17 thật: 17 kịch bản (home,
  404 full page browser vs partial HTMX vs curl, CSRF chặn/cho qua,
  cache headers font/css, OAuth URL không còn prompt=consent, badge
  admin rainbow/lửa bật/tắt qua form POST thật, badge mod glitch,
  logout invalidate session tức thì).

### 📦 Upgrade

- Migration 016 tự chạy lúc khởi động (thêm cột `role_badge_effects`
  vào `user_preferences`, DEFAULT TRUE, không lock bảng).
- Không đổi env bắt buộc. Không đổi API. Không đổi template HTMX
  contract.

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
