# Hướng dẫn Deploy Production (Coolify)

Kiến trúc production: **1 Coolify Docker Compose Stack = app + PostgreSQL 17**

```
GitHub push main/tag
  └─► GitHub Actions: cargo check → docker build → push ghcr.io/mhieuhonda/khogame
        └─► POST https://coolify.buppou.com/api/v1/deploy?uuid=<stack-uuid>
              └─► Coolify pull image mới → recreate container khogame-app-v7 (keep DB)
```

## Tài nguyên trên Coolify (hiện tại)

| Tài nguyên | UUID | Chi tiết |
|---|---|---|
| Stack `khogame` (Docker Compose) | `mlqvarityzusuzakkgup7tbb` | Project "Vạn Giới Studio" / production |
| └ `khogame` (app) | container `khogame-app-v7` | ghcr.io/mhieuhonda/khogame:latest, healthcheck /api/v1/health |
| └ `khogame-db` | container `khogame-db-v7` | postgres:17-alpine, volume `khogame-pgdata` |

## Domain & TLS

| Domain | Trạng thái |
|---|---|
| `https://louis.vangioitutien.com` | ✅ Let's Encrypt (Host rule) |
| `https://*.louis.vangioitutien.com` | ✅ Hoạt động qua HostRegexp; HTTPS dùng Traefik default cert (wildcard LE cần DNS-01 — cấu hình Cloudflare token trong Coolify UI nếu cần cert thật) |

DNS cần: bản ghi A `louis` (và wildcard `*.louis`) trỏ về IP public VPS.

## Biến môi trường (đã set trong compose stack)

`DATABASE_URL`, `SESSION_KEY`, `GOOGLE_CLIENT_ID/SECRET/REDIRECT_URI`, `BASE_URL`, `ADMIN_EMAIL=khongdich.admin@gmail.com`, `RUST_LOG`, `TZ`.

Secret thật KHÔNG nằm trong repo — chỉ có trong Coolify (env của stack) và GitHub Secrets cho CI:
`COOLIFY_URL`, `COOLIFY_API_TOKEN`, `COOLIFY_APP_UUID` (= UUID stack).

## Vận hành

```bash
# Deploy thủ công (bắt buộc POST — Coolify 4.3.7 webhook GET trả 405)
curl -X POST -H "Authorization: Bearer $TOKEN" \
  "https://coolify.buppou.com/api/v1/deploy?uuid=mlqvarityzusuzakkgup7tbb"

# Xem deployments
curl -H "Authorization: Bearer $TOKEN" \
  "https://coolify.buppou.com/api/v1/deployments?uuid=mlqvarityzusuzakkgup7tbb"

# Hủy deployment kẹt (nếu có)
curl -X POST -H "Authorization: Bearer $TOKEN" \
  "https://coolify.buppou.com/api/v1/deployments/<deployment-uuid>/cancel"
```

## Khắc phục sự cố đã gặp

| Triệu chứng | Nguyên nhân & xử lý đã áp dụng |
|---|---|
| Deployment kẹt `in_progress` mãi | Container exit ngay (binary dummy / DB không resolve) → docker compose `--wait` treo. Fix: stack chung network cho app+db; image có sanity check binary |
| App không kết nối được DB khi tách resource | 2 resource trên 2 network khác nhau → gộp vào 1 compose stack |
| Domain chính 503 sau nhiều lần tạo stack | Container orphan trùng tên `khogame` giữa các stack → đặt container_name phiên bản (`khogame-app-v7`) + restart proxy |
| HostRegexp không khớp | Traefik v3 cần pure-regex: ``HostRegexp(`[a-z0-9-]+\.louis\.vangioitutien\.com`)`` (cú pháp named-group `{sub<...>}` không chạy) |
| Docker build cho binary rỗng 303KB | Cargo fingerprint cache mount stale → `cargo clean -p khogame` + check binary > 2MB |
| 405 khi trigger deploy | Coolify 4.3.7 chỉ nhận POST cho webhook/deploy |
