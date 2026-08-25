# Hướng dẫn Deploy Production (Coolify)

Kiến trúc production: **1 Coolify Docker Compose Stack = app + PostgreSQL 17**

```
GitHub push main / tag v*
  └─► GitHub Actions (deploy.yml): cargo check → docker build (Rust 1.98)
        → push ghcr.io/mhieuhonda/khogame:{latest, sha-*, semver}
        └─► Pin image theo DIGEST vào deploy/compose.prod.yml
              └─► PATCH compose lên Coolify Service
                    └─► POST /api/v1/deploy?uuid=<service-uuid>&force=true
                          └─► Coolify pull đúng digest vừa build → recreate app (giữ DB)
```

Image được **pin theo digest** (`ghcr.io/mhieuhonda/khogame@sha256:...`) nên
Coolify luôn pull đúng phiên bản vừa build, không lo cache `:latest` cũ.

## Tài nguyên trên Coolify (hiện tại)

| Tài nguyên | UUID | Chi tiết |
|---|---|---|
| Stack `khogame` (Docker Compose) | `dwa5tq871zxdxgaysjdw7gge` | Project "Vạn Giới Studio" / production |
| └ `khogame` (app) | container do Coolify đặt tên | ghcr.io/mhieuhonda/khogame (pin digest), healthcheck `/api/v1/health` |
| └ `khogame-db` | container do Coolify đặt tên | postgres:17-alpine, volume `<uuid>_khogame-pgdata` |

> Stack đã được tạo lại từ đầu ngày 2026-08-24 (v0.3): DB cũ bị xoá cùng
> volumes, PostgreSQL 17 khởi tạo mới, app tự chạy migration lúc start.

## Domain & TLS

| Domain | Trạng thái |
|---|---|
| `https://louis.vangioitutien.com` | ✅ Let's Encrypt (Host rule) |
| `https://*.louis.vangioitutien.com` | ✅ Hoạt động qua HostRegexp; HTTPS dùng Traefik default cert (wildcard LE cần DNS-01 — cấu hình Cloudflare token trong Coolify UI nếu cần cert thật) |

DNS cần: bản ghi A `louis` (và wildcard `*.louis`) trỏ về IP public VPS.

## Biến môi trường

Secrets KHÔNG nằm trong repo. Khai báo trong tab **Environment Variables** của
Service trên Coolify, compose (`deploy/compose.prod.yml`) tham chiếu `${VAR}`:

| Key | Ý nghĩa |
|---|---|
| `DB_PASSWORD` | Mật khẩu PostgreSQL (dùng chung bởi app + db) |
| `SESSION_KEY` | Key ký session (64 hex) |
| `GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET` | OAuth Google |
| `DB_MAX_CONNECTIONS` | (tuỳ chọn, mặc định 15) Số connection tối đa của pool — giảm nếu nhiều service dùng chung cluster PG |
| `DB_MIN_CONNECTIONS` | (tuỳ chọn, mặc định 1) Connection giữ ấm |
| `DB_ACQUIRE_TIMEOUT_SECS` | (tuỳ chọn, mặc định 10) Thời gian chờ connection rảnh trước khi 500 |
| `JANITOR_INTERVAL_SECS` | (tuỳ chọn, mặc định 21600 = 6h) Chu kỳ dọn session hết hạn & notification đã đọc cũ 90 ngày |
| `GRACEFUL_SHUTDOWN_TIMEOUT_SECS` | (tuỳ chọn, mặc định 30) Force exit nếu còn connection treo sau khi nhận SIGTERM — nên khớp `stop_grace_period` |
| `AI_AGENT_SECRET` | (tuỳ chọn) Bật hệ thống AI Agent; để trống = endpoint `/auth/ai/*` trả 403 |
| `AI_AGENT_SESSION_TTL_DAYS` | (tuỳ chọn, mặc định 90) Thời gian sống phiên AI Agent |

GitHub Secrets cho CI/CD: `COOLIFY_URL`, `COOLIFY_API_TOKEN`,
`COOLIFY_SERVICE_UUID` (= UUID stack).

## Vận hành

```bash
# Deploy thủ công (sau khi image mới đã có trên GHCR)
curl -X POST -H "Authorization: Bearer $TOKEN" \
  "https://coolify.buppou.com/api/v1/deploy?uuid=dwa5tq871zxdxgaysjdw7gge&force=true"

# Xem deployments
curl -H "Authorization: Bearer $TOKEN" \
  "https://coolify.buppou.com/api/v1/deployments?uuid=dwa5tq871zxdxgaysjdw7gge"

# Hủy deployment kẹt (nếu có)
curl -X POST -H "Authorization: Bearer $TOKEN" \
  "https://coolify.buppou.com/api/v1/deployments/<deployment-uuid>/cancel"
```

## Graceful shutdown & health

- App bắt **SIGTERM/SIGINT** (docker stop) và chờ tối đa 30s cho các request
  đang xử lý hoàn tất (`stop_grace_period: 30s` đã set trong compose) — tránh
  5xx lúc deploy khi có người đang tải game. Hết grace period mà còn connection
  treo → force exit sạch (`GRACEFUL_SHUTDOWN_TIMEOUT_SECS`).
- `/api/v1/health` trả kèm `pool` (size/idle/in_use) và `uptime_secs` — dùng
  để alert khi pool cạn (leak) hoặc restart-loop (uptime luôn thấp).
- Janitor nền tự dọn `sessions` hết hạn + `notifications` đã đọc > 90 ngày
  mỗi 6h (đổi qua `JANITOR_INTERVAL_SECS`), log dòng `Janitor: đã xoá ...`.

## Quản trị phiên đăng nhập

Trang `/admin/sessions` (chỉ admin) liệt kê 200 phiên còn hạn mới nhất:
thiết bị (UA chuẩn hoá), IP, thời gian đăng nhập/hết hạn. Bấm **Thu hồi**
để buộc đăng xuất một thiết bị cụ thể (audit log ghi `session.revoke`) —
dùng khi nghi ngờ tài khoản bị truy cập trái phép mà không cần ban cả user.
User thường tự thu hồi toàn bộ phiên qua nút "Đăng xuất mọi thiết bị" ở
trang sửa hồ sơ (`POST /auth/logout-all`).

## Khắc phục sự cố đã gặp

| Triệu chứng | Nguyên nhân & xử lý đã áp dụng |
|---|---|
| Deployment kẹt `in_progress` mãi | Container exit ngay (binary dummy / DB không resolve) → docker compose `--wait` treo. Fix: stack chung network cho app+db; image có sanity check binary |
| App không kết nối được DB khi tách resource | 2 resource trên 2 network khác nhau → gộp vào 1 compose stack |
| HostRegexp không khớp | Traefik v3 cần pure-regex: ``HostRegexp(`[a-z0-9-]+\.louis\.vangioitutien\.com`)`` (cú pháp named-group `{sub<...>}` không chạy) |
| Docker build cho binary rỗng 303KB | Cargo fingerprint cache mount stale → `cargo clean -p khogame` + check binary > 2MB |
| 405 khi trigger deploy | Coolify chỉ nhận POST cho webhook/deploy |
| Chạy nhầm bản `:latest` cũ trên VPS | Docker cache không re-pull → CD mới pin image theo digest trước mỗi lần deploy |
| Orphan container trùng tên giữa các stack | Bỏ hẳn `container_name` cứng trong compose — Coolify tự đặt theo UUID stack |
