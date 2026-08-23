# 🎮 Kho Game

> Nền tảng chia sẻ game độc lập cho cộng đồng Việt Nam, xây dựng bằng Rust.

![Rust](https://img.shields.io/badge/Rust-1.98-orange?logo=rust)
![Axum](https://img.shields.io/badge/Axum-0.8-blue)
![PostgreSQL](https://img.shields.io/badge/PostgreSQL-17-blue?logo=postgresql)
![HTMX](https://img.shields.io/badge/HTMX-1.9-blue)
![Askama](https://img.shields.io/badge/Askama-0.12-purple)

## ✨ Tính năng

### Tính năng cốt lõi
- 🔐 **Đăng nhập bằng Google OAuth 2.0** (đăng nhập duy nhất)
- 🎮 **Đăng game** với link tải cho 5 nền tảng: Android, iOS, Windows, Linux, macOS
- 🔒 **Link tải ẩn** - người xem chỉ thấy nút "Tải về cho [nền tảng]", không thấy link thực
- 💬 **Bình luận** có trả lời (threaded)
- ❤️ **Like** bài viết và bình luận
- 📤 **Chia sẻ** qua Facebook, Twitter, Telegram, WhatsApp, copy link, native share
- 🚩 **Báo cáo** bài viết vi phạm

### 20+ tính năng nâng cao
1. ⭐ **Đánh giá sao (1-5)** cho game
2. 🔖 **Bookmark/Lưu** game yêu thích
3. 👥 **Theo dõi** tác giả
4. 🔔 **Thông báo** real-time (bình luận, like, follow, báo cáo...)
5. 🏷️ **Tags** - gắn thẻ cho game
6. 📁 **Thể loại** (10+ thể loại có sẵn)
7. 🖼️ **Screenshot gallery** - ảnh chụp màn hình
8. 🎬 **Trailer YouTube** nhúng
9. 🔍 **Tìm kiếm** với bộ lọc (thể loại, nền tảng, sắp xếp)
10. 🔥 **Game thịnh hành** ( Trending)
11. ⬇️ **Top tải nhiều**
12. ⭐ **Top đánh giá cao**
13. 🎯 **Game nổi bật** (Featured)
14. 📊 **Theo dõi lượt xem/tải/thích/bình luận**
15. 🌓 **Dark/Light mode** - chuyển đổi giao diện
16. 🌐 **Đa ngôn ngữ** - Tiếng Việt / English
17. 👤 **Hồ sơ người dùng** với thống kê
18. 🛡️ **Admin dashboard** - quản trị, kiểm duyệt
19. 📋 **Quản lý báo cáo** với workflow (pending → reviewing → resolved/dismissed)
20. 📌 **Ghim bình luận** (moderator)
21. 🔗 **Game liên quan** - gợi ý game cùng thể loại
22. 📈 **Thống kê chi tiết** cho admin
23. 🍪 **Session bảo mật** - cookie httpOnly, hash SHA-256
24. 📱 **Responsive** - hoạt động tốt trên mobile/tablet/desktop

## 🛠️ Công nghệ

| Thành phần | Công nghệ |
|-----------|-----------|
| Ngôn ngữ | Rust 1.98 |
| Web framework | Axum 0.8 |
| Template engine | Askama 0.12 |
| Frontend interactivity | HTMX 1.9 |
| Database | PostgreSQL 17 |
| ORM | sqlx 0.8 (compile-time checked) |
| Auth | Google OAuth 2.0 (oauth2 crate) |
| HTTP Client | reqwest 0.12 |
| Styling | Custom CSS (no framework) |

## 🚀 Cài đặt

### Yêu cầu
- Rust 1.98+ (`rustup default 1.98.0`)
- PostgreSQL 17+
- Google OAuth credentials

### Bước 1: Clone & cài đặt
```bash
git clone https://github.com/mhieuhonda/khogame.git
cd khogame
cp .env.example .env
```

### Bước 2: Cấu hình environment
Chỉnh sửa file `.env`:
```env
DATABASE_URL=postgres://khogame:khogame@localhost:5432/khogame
GOOGLE_CLIENT_ID=your-google-client-id
GOOGLE_CLIENT_SECRET=your-google-client-secret
GOOGLE_REDIRECT_URI=http://localhost:3000/auth/google/callback
SESSION_KEY=your-64-byte-hex-session-key
```

### Bước 3: Tạo database
```bash
# Tạo database và user
sudo -u postgres psql -c "CREATE USER khogame WITH PASSWORD 'khogame';"
sudo -u postgres psql -c "CREATE DATABASE khogame OWNER khogame;"
sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE khogame TO khogame;"
```

### Bước 4: Chạy ứng dụng
```bash
cargo run
```
Migration tự động chạy. Server khởi động tại `http://localhost:3000`.

## 📁 Cấu trúc dự án

```
khogame/
├── src/
│   ├── main.rs              # Entry point
│   ├── lib.rs               # Library root
│   ├── config.rs            # App config
│   ├── state.rs             # AppState
│   ├── db.rs                # DB connection
│   ├── auth.rs              # Google OAuth
│   ├── error.rs             # Error handling
│   ├── middleware.rs        # Auth middleware
│   ├── utils.rs             # Utility functions
│   ├── routes.rs            # Router
│   ├── templates.rs         # Askama templates + filters
│   ├── models/              # Data models
│   ├── repositories/        # Database queries
│   └── handlers/            # HTTP handlers
├── templates/               # Askama HTML templates
│   ├── layout.html
│   ├── index.html
│   ├── auth/
│   ├── game/
│   ├── profile/
│   ├── admin/
│   ├── notifications/
│   └── partials/            # HTMX partials
├── static/
│   ├── css/style.css
│   ├── js/app.js
│   └── img/
├── migrations/
│   └── 001_init.sql
├── Cargo.toml
├── .env.example
└── README.md
```

## 🔐 Bảo mật

- **Cookie httpOnly** - không thể truy cập bằng JavaScript
- **Session hash SHA-256** - token không lưu plaintext
- **CSRF protection** qua OAuth state
- **SQL injection prevention** - dùng sqlx prepared statements
- **HTML escaping** - tất cả output được escape tự động
- **Role-based access control** - admin/moderator/user

## 📜 License

MIT License - xem file LICENSE để biết chi tiết.
