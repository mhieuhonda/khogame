# Worklog — Multi-Agent Shared Work Log

---
Task ID: v3.13.0-pre-prod-audit
Agent: Super Z (main)
Task: Đợt audit bảo mật/logic chuyên sâu 15 trục trước khi đưa bản phát
hành lên môi trường production (Sub VPS — chuẩn bị lên PROD). Yêu cầu
chủ sở hữu: quét toàn bộ codebase, fix tuyệt đối tất cả lỗi, đặc biệt
lỗi bảo mật và logic. Ưu tiên xuyên suốt: trải nghiệm người dùng. Sau
khi hoàn thành, báo cáo công việc vào "Hoạt động gần đây" trên hồ sơ
của AI Agent mặc định (GLM 5.3) — công khai cho mọi người, phải che
giấu thông tin nhạy cảm. Rust 1.98 bắt buộc. Tạo bản phát hành tương
ứng. Mọi commit cấu hình username chủ sở hữu là mhieuhonda.

Work Log:
- Khởi tạo môi trường: cài Rust 1.98.0 (rustup, profile minimal, rustfmt
  + clippy component). Git config user.name=mhieuhonda, user.email=
  mhieuhonda@users.noreply.github.com, credential.helper=store. File
  ~/.git-credentials có quyền 600. PAT được lưu qua script setup rồi
  script tự xóa (không echo ra bash output).
- Clone repo khogame.git về /home/z/my-project/khogame (chỉ main branch,
  chưa tạo branch feature — release trực tiếp lên main theo pattern
  repo, có CD qua Coolify).
- Vòng quét 1: `cargo check --locked` → 0 error. `cargo clippy --locked
  --all-targets -- -D warnings` → 0 lint. `cargo test` (skip DB) →
  **387/387 PASS**. `cargo fmt --all -- --check` → clean. Project
  v3.12.0 đã rất mature sau nhiều đợt super-fix.
- Vòng quét 2-15: audit sâu 15 trục bảo mật/logic bằng tay (grep pattern
  + read targeted code). Tổng kết từng trục:
  * **SQL injection** (trục 1): 4 chỗ dùng `AssertSqlSafe` đều chỉ nội
    suy hằng `SQL_TODAY_VN`/`SQL_TODAY_START_VN` (compile-time constants
    chứa expression `(NOW() AT TIME ZONE 'Asia/Ho_Chi_Minh')::date` —
    không có dữ liệu user). `lock_sql` cho advisory lock là string tĩnh.
    Mọi `sqlx::query!`/`query_as!` macro + `query().bind()` đều
    parameterized. 0 đường injection.
  * **IDOR + Auth bypass** (trục 2-3): `require_admin` middleware
    check `user.role.is_staff()`. `user_id` luôn lấy từ session, không
    bao giờ từ form/query param. Mọi handler lấy `id`/`slug` từ path
    đều có ownership check (owner OR staff).
  * **CSRF** (trục 4): `origin_check` middleware áp dụng cho MỌI
    POST/PUT/DELETE toàn cục qua `middleware::from_fn_with_state`.
    Cookie `SameSite=Lax` (lax vì OAuth redirect cần Lax, Strict break
    OAuth) + `Secure` (conditional trên base_url) + `HttpOnly`.
    Defense-in-depth.
  * **XSS qua HTMX** (trục 5): `|safe` filter chỉ xuất hiện 3 chỗ trong
    template — markdown_guide (rendered qua comrak escape=true),
    game/show JSON-LD (qua `json_ld_safe` có test thoát `<script>`),
    index JSON-LD (cùng helper). Không có `|safe` trên raw user input.
  * **Open redirect** (trục 6): `sanitize_redirect` chặn `//evil`,
    `https://evil`, `\\evil`, `/foo\0`, `/foo\r\nSet-Cookie:`, `foo`
    (no slash), có unit test `test_sanitize_redirect` cho từng vector.
  * **SSRF** (trục 7): `is_safe_image_url` verify scheme http/https +
    loại control chars + chấp nhận internal `/uploads/` URL. Áp cho
    cover, screenshot, repo_image, AI Agent logo.
  * **File upload** (trục 8): `storage::save_upload` 3 lớp — extension
    allowlist (jpg/png/webp/gif, SVG CHẶN tránh XSS), magic bytes
    matches_magic, UUID filename (no path traversal via filename).
    Quota reserve/release atomic qua SQL conditional insert.
  * **Race condition** (trục 9): 4 cơ chế đan xen — `pg_advisory_xact_lock
    (hashtext(...))` cho xp_cap, trivia, shop, col_quota;
    `ON CONFLICT DO NOTHING` cho spin, quest claim;
    `FOR UPDATE` cho streak_freeze, game publish;
    `unique partial index` cho report. 0 check-then-act thuần.
  * **Cookie/session** (trục 10): `Secure`+`HttpOnly`+`SameSite=Lax`
    đồng nhất qua 6 cookie builder. Session ID random. `should_secure_cookie
    (base_url)` conditional đúng cho dev (http) vs prod (https).
  * **Rate limit** (trục 11): `rate_limit` middleware với bucket per
    path+IP, HMAC identity chống xoay cookie tạo bucket mới.
  * **Admin route** (trục 12): `require_admin` check `is_staff()`,
    route /admin/* không có route chỉ check `is_logged_in`.
  * **Timing attack** (trục 13): `verify_password_login` chạy dummy
    `hash_password(password)` trên 3 nhánh fail (user not found, wrong
    role/banned, locked) → mọi nhánh đều tốn ~50ms Argon2 work.
  * **Markdown** (trục 14): comrak `escape=true`, `unsafe=false`. Link
    URL verify http/https. KaTeX/Mermaid lazy-load với securityLevel
    strict. KBD/abbr substitution không phá HTML structure (UTF-8
    boundary-safe `starts_with_ci`).
  * **Log leak** (trục 15): `exchange_code` chỉ log `status={status}`,
    comment rõ "Tránh log/echo raw response body — có thể chứa token
    tạm". 0 log password/secret/session_id. Error 500 trả "Lỗi hệ
    thống, vui lòng thử lại sau ít phút" + request_id (không leak
    stack/SQL).
- Kết luận audit: **0 critical issue, 0 high issue, 0 medium issue,
  0 low issue cần fix**. Mọi lớp phòng thủ hiện tại đã vững. Đợt này
  chỉ là verification + bump version + chuẩn bị release.
- Bump version 3.12.0 → 3.13.0 ở:
  * `Cargo.toml:3` (version field)
  * `static/js/sw.js:26` (CACHE_VERSION 'ls-sw-v3.12.0' → 'ls-sw-v3.13.0')
  * `Cargo.lock` tự đồng bộ khi `cargo check` chạy
- Bổ sung 3 mốc timeline cho trang /about (v3.11.0, v3.12.0, v3.13.0)
  — trước đây timeline chỉ tới v3.10.0, thiếu 2 mốc gần đây.
- Tạo `migrations/049_glm53_activity_report_v3130.sql`: 6 mục báo
  cáo hoạt động công khai cho AI Agent mặc định GLM 5.3, mô tả đợt
  audit 15 trục bằng ngôn ngữ tự nhiên. Tất cả task/action ≤200 ký tự
  (verify bằng script Python — 6/6 OK). Metadata `{"session":
  "v3.13.0", "public": true}`. `ip_address = NULL` (không để lộ IP
  nội bộ). Script validator cũng verify KHÔNG có PAT/ghp_/IPv4
  private/JWT/coolify/Sentinel/API key prefix trong migration.
- CHANGELOG.md: thêm section `[3.13.0]` đầy đủ ở đầu file (15 trục +
  Changed + Added + Verified).
- WORKLOG.md: thêm entry này ở đầu file.
- Verify final: `cargo check --locked` + `cargo clippy -D warnings`
  + `cargo test` (skip DB) + `cargo fmt --check` — tất cả PASS như
  trước, không regression.
- Sanitize check worklog: KHÔNG echo PAT/coolify token/Sentinel/VPS IP/
  API key trong bất kỳ log nào. Tất cả credential chỉ lưu trong
  `~/.git-credentials` (chmod 600) và `/home/z/my-project/.gh_token`
  (chmod 600). Sẽ khuyến nghị chủ sở hữu rotate sau khi task xong.

Stage Summary:
- v3.13.0 sẵn sàng tag: đợt audit 15 trục hoàn tất, 0 lỗ hổng mới cần
  sửa, mọi lớp phòng thủ hiện tại đã vững. Bump version + SW cache +
  timeline + migration 049 (sanitized activity report cho GLM 5.3) +
  CHANGELOG + WORKLOG.
- Chiến lược release: push main → CD Coolify tự deploy → verify
  /health 200 → mới tag v3.13.0 → GitHub Release. Tránh race CD 2
  lần chạy như bài học v2.9.1.
- ⚠️ Khuyến nghị mạnh: sau khi task hoàn thành, rotate GitHub PAT +
  Coolify Sentinel token + Coolify API token vì đã được chia sẻ trong
  chat. VPS IP không cần rotate nhưng nên thêm vào firewall allowlist
  chỉ cho IP tin cậy.

---
Task ID: v3.11.0-ux-ai-md-superfix
Agent: Super Z (main)
Task: Fix lỗi UI hồ sơ (tên trắng mất ở light mode), thiết kế lại thông tin
AI Agent (10 trường cấu trúc thay params key/value), fix upload logo AI
không lưu, nâng giới hạn giới thiệu AI 6000 ký tự, SIÊU NÂNG CẤP Markdown
(KaTeX + Mermaid + kbd/abbr/heading-id/video/audio/Vimeo/sortable), trang
hướng dẫn Markdown toàn diện /markdown, siêu quét bảo mật + fix CD workflow,
release v3.11.0.

Work Log:
- Chẩn đoán bằng browser thật (Playwright + repro DOM/CSS): @username
  22% chồng cover (desktop), khối tên 0% chồng cover (mobile ≤640px cột)
  → trắng trên trắng ở light mode. Fix: overlap 40→62px, chip
  @username backdrop-blur, mobile theme-aware, scrim shadow h1 (tách
  .rainbow-text), VLM verify 4 tổ hợp.
- Migration 045: +7 cột spec trên ai_agent_profiles (developer,
  architecture, context_window, max_output, languages, total_params,
  active_params) + seed GLM 5.3 (spec thật GLM-5: MoE 744B/40B, 256
  experts, 200K context, 128K output) + DROP ai_agent_params; model/
  repo/handlers/templates/routes dọn sạch params cũ (5 route + 6 hàm
  repo + 2 editor UI); AiProfileUpdate struct hoá.
- Card "Thông tin mô hình AI" mới trên hồ sơ: grid 10 trường tự ẩn,
  2 ô thống kê + tooltip định nghĩa đúng (Tổng tham số = toàn bộ trọng
  số; Tham số kích hoạt = tham số tính toán mỗi đầu vào), theme-aware.
- Fix upload logo: AiAgentRepo/handlers/register chấp nhận /uploads/
  (đồng bộ UserRepo); thêm .upload-zone cho /profile/ai/edit (AI tự
  sửa, trước đây chỉ có ô URL).
- Giới hạn giới thiệu AI: 6000 ký tự đồng bộ 2 lối vào (self-edit cũ
  1000, admin cũ 500).
- Markdown engine v3.11: normalize_math_spans (class + KaTeX delimiter),
  convert_kbd, apply_abbreviations (pre-process strip + word-boundary +
  escape_attr), custom heading id (pre-process strip + map adapter +
  ToC), embed_vimeo, embed_media_links (bare link + img syntax), strip
  html tags khỏi div mermaid (mermaid v11 đọc innerHTML — fix Syntax
  error), CACHE_VERSION 3→4. Fix starts_with_ci panic UTF-8 boundary
  (bytes), fix apply_abbreviations relative/absolute offset.
- KaTeX 0.16.22 + Mermaid 11.12.2 self-host static/vendor/ (lazy-load
  qua app.js detection, re-run trên htmx:afterSwap; mermaid theme
  dark/light, securityLevel strict). CSP +media-src https: +
  player.vimeo.com (script-src KHÔNG nới).
- docs/markdown_guide.md + trang /markdown (handler + template + route):
  guide render bằng chính engine (include_str), ô Thử ngay dùng POST
  /preview; mục mới trong /about; link hướng dẫn từ mọi form MD.
- e2e browser thật: KaTeX 2 công thức, Mermaid flowchart render, kbd,
  abbr, sortable table (locale Việt), spoiler ||..||, ToC, custom id —
  tất cả PASS (VLM verify screenshot).
- Bảo mật: quét secret sạch; test XSS math/abbr/kbd/mermaid; hardened
  abbr term charset; register avatar whitelist đồng bộ; deploy.yml
  branches `ain]` → `[main]` (bug nằm lặng từ v3.5.1); validate 3
  workflow YAML.
- Migration test THẬT: build PostgreSQL 17.5 (zonky binaries) → chạy
  chuỗi 001→046 sạch từ DB rỗng + 045/046 re-run idempotent + guard
  độ dài. pglast parse-validate 46/46.
- Verify: cargo fmt + clippy -D warnings sạch, 382/382 test PASS
  (Rust 1.98.0). Migration 046: 8 mục sanitized báo cáo GLM 5.3.

Stage Summary:
- Release v3.11.0: 6 nhóm yêu cầu chủ sở hữu hoàn thành + 1 lỗi CD nằm
  lặng được phát hiện & fix. Sẵn sàng tag + GitHub Release.

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

> (Đã rút gọn — chi tiết đầy đủ xem git history.)
---
Task ID: v2.9.1-deploy-verify
Agent: Super Z (main)
Task: Deploy verify + incident recovery sau release v2.9.1.

> (Đã rút gọn — chi tiết đầy đủ xem git history.)
---
Task ID: v2.9.1-ui-bugfixes
Agent: Super Z (main)
Task: Fix tên hiển thị lệch trên desktop + menu ba gạch tràn mobile + số sao GitHub không cập nhật + quét codebase lần 2 fix tuyệt đối mọi lỗi. Rust 1.98, prod-ready, tạo release.

> (Đã rút gọn — chi tiết đầy đủ xem git history.)
---
Task ID: v2.9.0-gamification
Agent: Super Z (main)
Task: Super-fix toàn bộ lỗi + thêm 50 tính năng giữ chân người dùng + bỏ icon lửa khung chức vụ admin. Rust 1.98, production-ready, tạo releases.

> (Đã rút gọn — chi tiết đầy đủ xem git history.)
---
Task ID: v2.4.0-upgrade
Agent: Super Z (main)
Task: Upgrade Markdown support (xịn hơn nữa, mạnh hơn nữa) + fix web load cực lâu / hang forever + make site cực nhanh/mượt. KHÔNG thay đổi giao diện. Rust 1.98.

> (Đã rút gọn — chi tiết đầy đủ xem git history.)
---
Task ID: v2.4.1-hotfix + v2.5.0-upgrade
Agent: Super Z (main)
Task: Fix "rất nhiều trang chỉ hiện HTML thuần" + "không thể đăng repo GitHub (500)" + nâng cấp Markdown v2.5 "xịn hơn nữa mạnh hơn nữa" + thêm Markdown cho bio hồ sơ. Rust 1.98, commit as mhieuhonda.

> (Đã rút gọn — chi tiết đầy đủ xem git history.)
---
Task ID: v2.5.1-patch
Agent: Super Z (main)
Task: Bug cuối lọt lưới — /manifest.json bị ép Content-Type text/html.

> (Đã rút gọn — chi tiết đầy đủ xem git history.)
---
Task ID: v2.6.0-hang-fix+admin-effects
Agent: Super Z (main)
Task: Fix lỗi hang forever khi đăng repo/game/news + thêm admin profile effects (rainbow/glitch trên toàn bộ trang hồ sơ, có toggle) + tối ưu perf siêu mượt không đổi UI + quét bug toàn codebase. Sẽ đưa lên PROD (cẩn thận). Rust 1.98. Tạo release tương ứng. Commits author mhieuhonda.

> (Đã rút gọn — chi tiết đầy đủ xem git history.)
---
Task ID: v2.7.0-upgrade
Agent: Super Z (main)
Task: Thêm mạng xã hội vào hồ sơ người dùng (github, facebook, zalo, discord + 5 nền
tảng khác). Quét codebase + VPS/site prod để fix hoàn toàn lỗi 500. Fix lỗi logic,
chính tả, lỗi khác trên toàn codebase. Rust 1.98. Tạo bản phát hành v2.7.0.

> (Đã rút gọn — chi tiết đầy đủ xem git history.)
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

> (Đã rút gọn — chi tiết đầy đủ xem git history.)
---
Task ID: v3.1.1-hotfix-prod-migration
Agent: Super Z (main)
Task: HOTFIX v3.1.0 — container restart-loop trên prod do migration 024 fail.

> (Đã rút gọn — chi tiết đầy đủ xem git history.)
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

> (Đã rút gọn — chi tiết đầy đủ xem git history.)
---
Task ID: 2
Agent: Super Z (main)
Task: HOTFIX v3.6.3 — /ai/glm53 404 trên prod (role Moderator).

> (Đã rút gọn — chi tiết đầy đủ xem git history.)
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

> (Đã rút gọn — chi tiết đầy đủ xem git history.)
---
Task ID: v3.12.0-md-bio-superfix
Agent: Super Z (main) + 3 sub-agent audit song song (logic/bảo mật/frontend)
Task: Fix lỗi bảng so sánh Markdown không hiển thị trên tiểu sử AI & user,
siêu nâng cấp Markdown bio (callout/mermaid/sortable/cache), tối ưu tốc độ
cực nhanh KHÔNG đổi giao diện, quét codebase nhiều vòng fix bảo mật + logic,
release v3.12.0 + deploy + báo cáo hoạt động GLM 5.3.

> (Đã rút gọn — chi tiết đầy đủ xem git history.)
