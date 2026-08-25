# News Module — Hướng dẫn

Tài liệu này mô tả cách sử dụng module tin tức (`/news`) của Louis Space, dành cho người dùng cuối và admin.

## 📰 Tổng quan

Louis Space có 2 mảng chính:
1. **Game** — đăng và khám phá game độc lập
2. **News** — tin tức cộng đồng về game, công nghệ, esports, v.v.

Khác với game (user đăng là public ngay), **tin tức do user đăng phải được admin duyệt** trước khi xuất bản. Điều này tránh lan truyền tin giả trên nền tảng cộng đồng.

## 🔄 Workflow trạng thái tin

```
[User đăng tin]
       ↓
   PENDING (chờ admin duyệt)
       ↓
   ┌─────────────────────┐
   │  Admin duyệt        │
   ├─────────────────────┤
   │  → Published (công khai, hiện /news) │
   │  → Rejected  (kèm lý do, user có thể sửa và gửi lại) │
   │  → Archived  (ẩn khỏi list, vẫn xem qua direct link) │
   └─────────────────────┘
```

## 📝 Đăng tin (user)

1. Đăng nhập bằng Google.
2. Vào `/news/new` (menu Cá nhân → Đăng tin).
3. Điền:
   - **Tiêu đề** (tối đa 200 ký tự, rõ ràng, không giật tít)
   - **Tóm tắt** (tối đa 500 ký tự, hiển thị ở list)
   - **Nội dung** (tối đa 50.000 ký tự, hỗ trợ markdown cơ bản)
   - **Thể loại** (game, tech, industry, esports, community, review, update, other)
   - **Ảnh bìa** (URL, http/https, tùy chọn)
   - **Nguồn** (tên + URL, bắt buộc nếu không phải tin gốc)
4. Submit → tin vào hàng đợi `pending`.
5. Nhận notification khi admin duyệt hoặc từ chối.

## 🛡️ Admin duyệt tin

### Truy cập
- Vào `/admin/news/pending` (menu admin → Duyệt tin)
- Yêu cầu role `admin` (moderator không được duyệt tin — quyền này nhạy cảm)

### Hiển thị
- Tiêu đề + tóm tắt + nội dung đầy đủ
- **Tác giả**: tên, email (admin xem được, mod không thấy)
- **IP + UA lúc đăng** (để truy vết spam/abuse)
- **Thời điểm đăng**

### Hành động
- **Duyệt** → tin chuyển sang `published`, `published_at` set NOW()
- **Từ chối** → kèm note lý do, tin sang `rejected`, user có thể sửa và gửi lại (tự động reset về `pending`)
- **Archive** → tin đã published nhưng cũ/ít liên quan, ẩn khỏi list chính nhưng vẫn truy cập qua URL trực tiếp
- **Featured** → đánh dấu nổi bật (hiển thị đầu list /news và homepage)
- **Delete** → xóa vĩnh viễn

### Notification
- Duyệt → user nhận notification "Tin '{}' đã được duyệt"
- Từ chối → user nhận notification + review_note

## 🌐 Public API

```
GET /api/v1/news?page=1&category=game
GET /api/v1/news/{slug}
GET /news.rss                # RSS feed
```

Cache 120s cho list, 60s cho detail. Trả JSON với đầy đủ thông tin (nhưng không IP/UA — chỉ admin thấy).

## 🗺️ SEO

- Sitemap.xml có 50 URL tin published mới nhất
- `/news` được thêm vào list static URLs
- JSON-LD Article schema trên trang chi tiết
- Meta og:type=article, og:image, article:published_time

## 🔒 Bảo mật

- User đăng tin → lưu IP/UA (chỉ admin xem)
- Validate URL http/https (chống javascript: scheme)
- Validate category whitelist (chống injection)
- Clamp title 200, content 50.000, excerpt 500 ký tự
- Edit tin published: chỉ admin (user phải liên hệ admin)
- Edit tin rejected: tự động reset về pending

## 📊 Stats

- Admin dashboard hiển thị `total_news` + `pending_news` (link đến queue duyệt)
- Homepage hiển thị 3 tin published mới nhất + total_news stat

## 🎯 Categories

| Key | Label | Mô tả |
|-----|-------|-------|
| `game` | Tin game | Tin tức về game cụ thể |
| `tech` | Công nghệ | Tin công nghệ game (engine, tool, v.v.) |
| `industry` | Ngành game | Industry news, business |
| `esports` | Esports | Giải đấu, team, player |
| `community` | Cộng đồng | Sự kiện, meetup, thảo luận |
| `review` | Review | Review game, hardware |
| `update` | Cập nhật | Patch notes, version updates |
| `other` | Khác | Không phân loại |
