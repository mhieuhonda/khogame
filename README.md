# 🎮 Kho Game

> Nền tảng chia sẻ game độc lập cho cộng đồng Việt Nam, xây dựng bằng Rust.

![Rust](https://img.shields.io/badge/Rust-1.98-orange?logo=rust)
![Axum](https://img.shields.io/badge/Axum-0.8.9-blue)
![PostgreSQL](https://img.shields.io/badge/PostgreSQL-17-blue?logo=postgresql)
![HTMX](https://img.shields.io/badge/HTMX-2.0-blue)
![Askama](https://img.shields.io/badge/Askama-0.16-purple)
![sqlx](https://img.shields.io/badge/sqlx-0.9-green)

## 🛠️ Công nghệ

| Thành phần | Công nghệ |
|-----------|-----------|
| Ngôn ngữ | Rust 1.98 (edition 2021) |
| Web framework | Axum 0.8.9 + axum-extra 0.12 |
| Template engine | Askama 0.16 |
| Frontend interactivity | HTMX 2.0.10 (self-hosted) |
| Database | PostgreSQL 17 |
| ORM | sqlx 0.9 (runtime-tokio + rustls) |
| Auth | Google OAuth 2.0 |
| HTTP Client | reqwest 0.12 |
| Styling | Custom CSS (dark/light mode) |

## ✨ Tính năng cốt lõi
- 🔐 **Đăng nhập Google OAuth 2.0** (duy nhất)
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
12. 🌐 **Public JSON API v1** — /api/v1/games, /api/v1/repos, /api/v1/stats, /api/v1/health
13. 📡 **RSS feed** — /rss.xml
14. 🗺️ **Sitemap + robots.txt** — SEO thân thiện
15. 🔍 **OG meta tags** — chia sẻ đẹp trên mạng xã hội
16. 🚦 **Rate limiting** — download 20/phút, comment 10/phút theo IP
17. 📋 **My Games** — quản lý game của tôi (kể cả draft), xuất bản 1 click
18. ✏️ **Sửa bình luận** trong 5 phút
19. ⚠️ **Cảnh báo trùng tiêu đề** khi đăng game (AJAX realtime)
20. 📥 **Export backup JSON** (admin) + Health check nâng cao kèm DB status

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
│   ├── config.rs            # AppConfig (env)
│   ├── state.rs             # AppState + rate limiter + maintenance cache
│   ├── db.rs                # PgPool
│   ├── auth.rs              # Google OAuth
│   ├── error.rs             # AppError → HTTP
│   ├── middleware.rs        # CurrentUser/AuthUser + admin + rate limit + maintenance
│   ├── routes.rs            # Router (60+ routes)
│   ├── templates.rs         # Askama templates + custom filters
│   ├── models/              # User, Game, Comment, Repo, Settings...
│   ├── repositories/        # SQL queries
│   └── handlers/            # HTTP handlers (games, admin, repos, api, ...)
├── templates/               # Askama HTML (admin/, repos/, partials/...)
├── static/                  # CSS + JS (htmx 2.0.10 self-hosted)
├── migrations/              # SQL migrations (001, 002)
├── .github/workflows/       # CI + Build & Deploy Coolify
├── Dockerfile               # Multi-stage Rust 1.98 → debian-slim
└── docker-compose.yml       # Local: app + Postgres 17
```

## 🔐 Bảo mật
- Cookie httpOnly + session hash SHA-256 trong DB
- CSRF qua OAuth state
- SQL injection: 100% prepared statements (sqlx)
- HTML escaping tự động (Askama) + `Safe` có kiểm soát
- RBAC: user / moderator / admin
- Rate limiting theo IP
- Admin: đăng nhập Google với `ADMIN_EMAIL` được whitelist

## 📜 License
MIT
