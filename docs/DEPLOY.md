# Hướng dẫn Deploy Production (Coolify)

Kiến trúc: **GitHub Actions → GHCR → Coolify Webhook (POST) → Deploy**

```
git push main
  └─► GitHub Actions: cargo check → docker build → push ghcr.io/mhieuhonda/khogame:latest
        └─► POST https://coolify.buppou.com/webhooks/<uuid>   ⚠️ Coolify 4.3.7 bắt buộc POST (GET trả 405)
              └─► Coolify pull image mới & redeploy tự động
```

## 1. Tài nguyên cần tạo trên Coolify

| Tài nguyên | Loại | Ghi chú |
|---|---|---|
| `khogame-db` | PostgreSQL 17 | Database của app, cùng server VPS |
| `khogame` | Docker Image | `ghcr.io/mhieuhonda/khogame:latest` |
| Domain | `https://louis.vangioitutien.com` + `https://*.louis.vangioitutien.com` | Wildcard cần DNS wildcard trỏ về VPS |

## 2. Biến môi trường app (set trong Coolify, KHÔNG commit .env)

| Biến | Bắt buộc | Mô tả |
|---|---|---|
| `DATABASE_URL` | ✅ | Connection string Postgres 17 (internal URL của Coolify) |
| `SESSION_KEY` | ✅ | `openssl rand -hex 32` |
| `GOOGLE_CLIENT_ID` | ✅ | Google OAuth |
| `GOOGLE_CLIENT_SECRET` | ✅ | Google OAuth |
| `GOOGLE_REDIRECT_URI` | ✅ | `https://louis.vangioitutien.com/auth/google/callback` |
| `BASE_URL` | ✅ | `https://louis.vangioitutien.com` |
| `ADMIN_EMAIL` | ✅ | `khongdich.admin@gmail.com` — tự lên admin khi login |
| `GITHUB_TOKEN` | ⬜ | Tăng rate limit GitHub API cho trang Repos |

## 3. GitHub Secrets (đã tạo tự động)

| Secret | Dùng cho |
|---|---|
| `COOLIFY_WEBHOOK_URL` | URL webhook deploy của app (gọi POST sau khi push image) |
| `COOLIFY_URL` | `https://coolify.buppou.com` (phương án dự phòng qua API) |
| `COOLIFY_API_TOKEN` | Token API Coolify (dự phòng) |
| `COOLIFY_APP_UUID` | UUID ứng dụng (dự phòng) |

## 4. Wildcard domain `*.louis.vangioitutien.com`

Cần 2 điều kiện:
1. **DNS**: bản ghi `A` wildcard `*.louis` trỏ về IP public VPS (nếu dùng Cloudflare, bật proxy hoặc DNS-only đều được).
2. **Chứng chỉ SSL wildcard**: Coolify cần cấu hình DNS challenge (Cloudflare API token) vì HTTP challenge không cấp được wildcard cert. Nếu chưa có, dùng domain chính trước.

## 5. Storage trên VPS

App gắn persistent storage `/app/storage` (qua Coolify: *Add persistent storage*). Database PostgreSQL 17 chạy trên cùng VPS qua Coolify Stack.

## 6. Lệnh hữu ích

```bash
# Xem log app
curl -s -H "Authorization: Bearer $COOLIFY_TOKEN" https://coolify.buppou.com/api/v1/applications/{uuid}/logs

# Trigger deploy thủ công (POST!)
curl -X POST -H "Authorization: Bearer $COOLIFY_TOKEN" "https://coolify.buppou.com/api/v1/deploy?uuid={uuid}"

# Pull image mới nhất về VPS
docker pull ghcr.io/mhieuhonda/khogame:latest
```

## 7. Khắc phục sự cố

| Triệu chứng | Nguyên nhân & xử lý |
|---|---|
| Webhook trả **405** | Đang dùng GET — Coolify 4.3.7 chỉ nhận POST cho webhook deploy |
| App restart loop | Kiểm tra `DATABASE_URL` + log container |
| Migration fail | Bảng `_sqlx_migrations` conflict — chỉ xảy ra khi đổi file migration cũ |
| Google OAuth `redirect_uri_mismatch` | Sửa `GOOGLE_REDIRECT_URI` khớp URL Google Console |
