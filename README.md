# Louis Space

> Nền tảng chia sẻ game độc lập & tin tức cộng đồng Việt Nam — xây dựng bằng Rust.

![Rust](https://img.shields.io/badge/Rust-1.98-orange?logo=rust)
![Axum](https://img.shields.io/badge/Axum-0.8.9-blue)
![PostgreSQL](https://img.shields.io/badge/PostgreSQL-17-blue?logo=postgresql)
![HTMX](https://img.shields.io/badge/HTMX-2.0-blue)
![Tests](https://img.shields.io/badge/tests-352-brightgreen)
![Version](https://img.shields.io/badge/version-3.5.0-0f172a)

## Stack công nghệ

| Thành phần | Công nghệ |
|-----------|-----------|
| Ngôn ngữ | Rust 1.98 |
| Web framework | Axum 0.8.9 + axum-extra 0.12 |
| Templating | Askama 0.16 (compile-time) |
| Frontend | HTMX 2.0.10 (self-hosted, no SPA) |
| Database | PostgreSQL 17 + sqlx 0.9 (rustls) |
| Markdown | comrak 0.54 (GFM superset) + syntect (syntax highlight) |
| Email | lettre 0.11 (SMTP, rustls) |
| Auth | Google OAuth 2.0 + AI Agent token (48-byte entropy) |
| Styling | Custom CSS, dark/light mode, PWA manifest |
| Deploy | Docker multi-stage + Coolify |

## Tính năng chính

### Nội dung
- **Game CRUD** với link tải ẩn cho 5 nền tảng (Android, iOS, Windows, Linux, macOS)
- **Tin tức** với workflow duyệt: `draft → pending → published → archived`
- **GitHub Repos** với auto-fetch stars/forks/language từ GitHub API
- **Markdown engine** vượt trội hơn GitHub: GFM + tables + tasklists + callouts (`> [!NOTE]`) + spoiler (`>!text!<`) + footnotes + math + syntax highlighting (syntect) + YouTube auto-embed + URL allowlist + auto-`rel="nofollow ugc noopener noreferrer"`. Không bao giờ render raw HTML → zero XSS surface.
- **Related news** — cùng category, fallback tin mới nhất
- **Live chat** WebSocket realtime

### Tương tác
- Bình luận threaded 2 cấp (game + news) + like + edit (5ph) + delete
- **@mention** notifications (batch INSERT, không N+1)
- Like · Bookmark · Rate 1-5 sao · Follow author
- Email notifications (lettre + email_queue + janitor flusher 2 phút)
- Báo cáo nội dung (workflow pending → reviewing → resolved/dismissed)

### Giữ chân người dùng (v3.0.0 — Retention Engine)
- **Nhiệm vụ hằng ngày + hàng tuần** (daily/weekly quests) — tự sinh 5 nhiệm vụ/ngày deterministic theo user, progress auto-bump từ mọi hành động, claim XP thủ công (agency)
- **Vòng quay may mắn** — 1 lượt/ngày, trọng số công khai, jackpot 500 XP (0.5%), animation celebrate khi trúng lớn
- **Câu đố hằng ngày (Trivia)** — 3 câu/ngày, đáp án chỉ chấm ở server (chống inspect), đúng cả 3 nhận bonus
- **Cửa hàng XP** — Streak Freeze ❄️, XP Boost x2 24h ⚡, Viền Tên chat 30 ngày ✨, Hộp Bí Ẩn 🎁; trừ XP atomic trong 1 transaction
- **Streak Freeze tự động** — lỡ 1 ngày điểm danh tự tiêu 1 freeze, chuỗi tiếp tục (PK chống double-consume, chỉ bảo vệ đúng 1 ngày)
- **Giới thiệu bạn bè (Referral)** — link ngắn `/r/{code}` + cookie 30 ngày, cả 2 phía +100 XP khi người mới đăng nhập lần đầu
- **Heatmap hoạt động 13 tuần** trên hồ sơ (GitHub-style) + lịch điểm danh tháng
- **Streak warning banner** — nhắc điểm danh khi chuỗi đang chờ + **XP toast** cho mọi phần thưởng
- **Game của Ngày** — deterministic theo ngày VN (hashtext id+date), mỗi ngày một bất ngờ
- **Sắp ra mắt** — đếm ngược release_date trên trang game + section homepage
- **Người chơi khác cũng thích** — co-occurrence qua bảng likes (collaborative-ish)
- **Onboarding checklist** — 5 bước đầu ×20 XP cho người mới (avatar, bio, comment, bookmark, rating)
- **Độ hoàn thiện hồ sơ** — ring % avatar/bio/socials trên profile
- **Bảng xếp hạng mùa (tháng) + Hall of Fame tuần** — cạnh tranh chu kỳ ngắn từ xp_events
- **Đặc quyền theo cấp độ** — giới hạn bộ sưu tập tăng theo level (5→7→12→20)
- **Tùy chọn thông báo** — bật/tắt từng loại in-app (follow/new_game/review/mention) + **email tổng hợp hằng tuần** opt-in (job sáng thứ 2 giờ VN)

### Admin
- Dashboard chart 7 ngày (views/downloads/games/users)
- Quản lý users (role, ban), games (hide/feature/delete), comments (pin/delete)
- CRUD categories + news categories
- Audit log · Session revoke · Broadcast · Export JSON backup
- AI Agent reports (live feed 30s với progress bar)
- Settings (tên site, mô tả, footer, auto-approve)
- Maintenance mode + announcement banner

### API & SEO
- Public JSON API v1 (`/api/v1/games`, `/news`, `/repos`, `/tags`, `/categories`, `/users`, `/stats`, `/health`)
- RSS feeds (`/rss.xml`, `/news.rss`) với ETag
- Sitemap.xml + robots.txt + OpenSearch + PWA manifest + security.txt
- OG meta tags + JSON-LD VideoGame schema
- Trigram search (pg_trgm) cho game + news

### Bảo mật
- Cookie httpOnly + SameSite=Lax + session hash SHA-256 trong DB
- OAuth state CSRF verification + Origin/Referer check trên unsafe methods
- Security headers toàn site (CSP, X-Frame-Options DENY, HSTS, COOP, COEP)
- RBAC: user / moderator / admin / ai_agent
- Rate limit per-endpoint (download 20/phút, comment 10/phút, AI register 5/10phút) với `Retry-After` RFC 9110
- 100% prepared statements (sqlx) + Askama auto-escape
- URL scheme allowlist (`http(s)`, `mailto`, `tel`) — `javascript:` bị chặn

### Performance
- Static cache immutable 1 năm (fonts) + 30 ngày SWR (CSS/JS) + cache-bust URL
- ETag cho RSS/sitemap/announcement (304 Not Modified)
- View/download counters fire-and-forget (tokio::spawn)
- Parallel queries (tokio::join!)
- Partial indexes (comments top-level vs replies, news published)
- Graceful shutdown SIGTERM/SIGINT với grace period 30s
- Compression (gzip + brotli + zstd)
- Background janitor dọn session/notification/daily_stats mỗi 6h

## Chạy local

```bash
git clone https://github.com/mhieuhonda/khogame.git
cd khogame
cp .env.example .env   # sửa DATABASE_URL, GOOGLE_*
docker compose up -d --build
# hoặc: cargo run (cần Postgres 17 + Rust 1.98)
```

Server chạy tại `http://localhost:3000`. Migration tự chạy khi khởi động.

## Deploy Production (Coolify)

```bash
# 1. Coolify: tạo service từ Dockerfile (hoặc GHCR image)
# 2. Set env vars (xem .env.example):
#    DATABASE_URL=postgres://...
#    GOOGLE_CLIENT_ID=...
#    GOOGLE_CLIENT_SECRET=...
#    BASE_URL=https://your-domain.com
#    STORAGE_DIR=/app/storage  # volume mount
#    # Email (optional — không cấu hình thì email_queue noop)
#    SMTP_HOST=smtp.gmail.com
#    SMTP_PORT=587
#    SMTP_USERNAME=...
#    SMTP_PASSWORD=...
#    SMTP_FROM=Louis Space <noreply@your-domain.com>
# 3. Volume mount: khogame-storage:/app/storage
# 4. Deploy — Coolify pull image + restart container
```

Chi tiết đầy đủ: [`docs/DEPLOY.md`](docs/DEPLOY.md)

## Cấu trúc dự án

```
khogame/
├── src/
│   ├── main.rs / lib.rs        # Entry + run() + migrations + janitor
│   ├── config.rs / state.rs    # AppConfig + AppState (DB pool, cache)
│   ├── middleware.rs           # AuthUser/CurrentUser/require_admin + rate_limit + CSRF
│   ├── routes.rs               # 80+ routes
│   ├── templates.rs            # Askama templates + custom filters
│   ├── services/
│   │   ├── markdown.rs         # comrak + syntect MD engine
│   │   ├── email.rs            # lettre SMTP + email_queue flusher
│   │   ├── audit.rs            # admin audit log
│   │   ├── json_ld.rs          # schema.org builder
│   │   └── storage.rs          # upload (avatar/cover/repo)
│   ├── handlers/               # 13 HTTP handler modules
│   ├── models/                 # 13 data models
│   └── repositories/           # 14 SQL repo modules
├── templates/                  # Askama HTML
├── static/                     # CSS + JS (htmx self-hosted) + fonts + img
├── migrations/                 # 18 SQL migrations
├── deploy/compose.prod.yml     # Coolify prod compose
├── Dockerfile                  # Multi-stage Rust 1.98 → debian-slim
└── docker-compose.yml          # Local: app + Postgres 17
```

## AI Agent

Tài khoản đặc biệt cho AI được admin ủy quyền (vd: code-fixing bot):

```bash
# Admin set env: AI_AGENT_SECRET=<64-char hex>
# AI đăng ký 1 lần:
curl -X POST https://your-domain.com/auth/ai/register \
  -H 'Content-Type: application/json' \
  -d '{"secret":"<AI_AGENT_SECRET>","model_name":"Ox Alpha","vendor":"Z.ai"}'
# → returns api_token (save 1 lần, không hiển thị lại)

# AI báo cáo tiến trình:
curl -X POST https://your-domain.com/ai/progress.json \
  -H 'Authorization: Bearer kgai_<...>' \
  -d '{"task":"fix-123","percentage":50,"status":"running"}'
```

Admin xem live feed tại `/admin/ai-reports` (tự refresh 30s).

## Testing

```bash
cargo test                   # 221 unit tests
cargo clippy --all-targets   # 0 warnings
cargo build --release        # release profile (LTO + strip)
```

CI: clippy `-D warnings` + rustdoc + fmt + tests + cargo-audit RustSec mỗi push.

## License

MIT — see [`LICENSE`](LICENSE)
