# Lấy lại IP thật của người dùng (real client IP)

> Áp dụng cho kiến trúc deploy hiện tại của Louis Space:
> **client → VPS chính (nginx, TCP forwarding 443) → tunnel riêng → VPS phụ (Traefik/Coolify) → app**
>
> Tài liệu này giải thích (1) vì sao admin hiện thấy **cùng một IP cho
> toàn bộ người dùng**, (2) những gì app đã tự xử lý từ v1.3.0, và
> (3) 2 thao tác cấu hình trên hạ tầng để hiện IP thật.

---

## 1. Chẩn đoán (đã xác minh bằng thực nghiệm 2026-08-26)

- `louis.vangioitutien.com` trỏ về **VPS chính** (163.44.96.79).
- Port 443 của VPS chính **forward TCP thuần** (nginx stream, SNI
  passthrough) về Traefik trên **VPS phụ** (10.187.247.3) — TLS kết thúc
  ở Traefik (bằng chứng: certificate "TRAEFIK DEFAULT CERT" cho SNI lạ,
  challenge ACME TLS-ALPN-01 hoạt động).
- Forwarding TCP **không giữ source IP**: Traefik chỉ thấy IP tunnel của
  VPS chính cho **mọi** kết nối.
- Thực nghiệm: 2 máy khác nhau (sandbox + crawler r.jina.ai) request
  cùng lúc → **dính chung bucket rate-limit** → app tính ra CÙNG một IP
  cho cả hai. Kết luận: IP client bị mất ở hop TCP, KHÔNG thể khôi phục
  bằng bất kỳ logic nào trong app.

Hệ quả trước v1.3.0:

| Triệu chứng | Nguyên nhân |
|---|---|
| Admin → Quản lý phiên: mọi session cùng 1 IP (IP private kiểu `10.x`) | App đọc X-Real-IP do Traefik set = IP tunnel của VPS chính |
| User bị 429 "Thao tác quá nhanh" ngẫu nhiên khi site đông | Toàn site chia CHUNG bucket rate-limit theo IP (1 user spam = chặn cả site) |

## 2. App đã tự xử lý gì từ v1.3.0 (không cần cấu hình thêm)

1. **Rate-limit an toàn khi IP là IP proxy**: nếu IP tính ra là
   private/loopback (`10.x`, `172.16-31.x`, `192.168.x`, `127.x`,
   `unknown`), bucket key tự chuyển sang **định danh per-browser**:
   - user đã đăng nhập → hash session cookie (ổn định theo user);
   - khách → cookie `ls_anon` (UUID ngẫu nhiên, HttpOnly, không PII)
     tự set ở lần đầu.
   Khi hạ tầng truyền IP thật, IP là public → key trở lại theo IP như cũ.
2. **`TRUSTED_PROXY_HOPS`** (env, mặc định 1): parse X-Forwarded-For đúng
   số hop proxy. Khi bạn thêm CDN/WAF/nginx L7 trước Traefik, đặt = 2
   (hoặc 3) để lấy đúng IP client thay vì IP của proxy cuối.
3. **Log cảnh báo 1 lần** khi phát hiện IP private dùng chung, kèm con
   trỏ tới tài liệu này.

Xem test đầy đủ: `src/middleware.rs` → `mod client_ip_tests`.

## 3. Bật IP thật — 2 thao tác bắt buộc trên hạ tầng

IP client chỉ còn tồn tại ở **VPS chính** (nginx). Muốn nó tới được app
phải dùng **PROXY protocol** trên suốt chuỗi:

### Bước 1 — VPS chính: nginx gửi PROXY protocol

Tìm block `stream` đang forward 443 về VPS phụ (thường trong
`/etc/nginx/nginx.conf` hoặc `/etc/nginx/stream-conf.d/*.conf`), thêm
`proxy_protocol on;`:

```nginx
stream {
    map $ssl_preread_server_name $backend {
        louis.vangioitutien.com   khogame_sub;   # qua tunnel
        default                   local_https;
    }

    upstream khogame_sub {
        server 10.187.247.3:443;   # IP tunnel VPS phụ
    }

    server {
        listen 443;
        proxy_pass $backend;
        proxy_protocol on;         # ← THÊM DÒNG NÀY
    }
}
```

> Cảnh báo: khi bật `proxy_protocol on`, **đầu kia bắt buộc** phải hiểu
> PROXY protocol (bước 2). Làm bước 2 TRƯỚC hoặc cùng lúc — nếu chỉ làm
> bước 1, Traefik nhận chữ binary PROXY nối trước request HTTP sẽ trả
> 400 cho toàn bộ traffic.

### Bước 2 — VPS phụ: Traefik tin PROXY protocol từ VPS chính

Coolify UI → Server **vangioi-vps** → **Proxy** → **Advanced** (chỉnh
static config của Traefik). Thêm 2 flag vào `command:`:

```yaml
    command:
      # ... các flag hiện có giữ nguyên ...
      - '--entrypoints.http.proxyProtocol.trustedIPs=10.187.247.0/24'
      - '--entrypoints.https.proxyProtocol.trustedIPs=10.187.247.0/24'
```

`10.187.247.0/24` = dải IP tunnel — Traefik chỉ nhận PROXY header từ
IP này, client ngoài vẫn không spoof được. Nếu IP tunnel của VPS chính
khác (vd `10.187.200.1`), thay bằng IP/dải tương ứng. Deploy lại proxy
trong Coolify.

Sau 2 bước: Traefik biết IP client thật → tự set `X-Real-Ip` /
`X-Forwarded-For` = IP client → app hiển thị đúng ở admin, rate-limit
key theo IP thật (bucket cookie tự tắt).

### Bước 3 (tuỳ chọn) — port 80

Nếu nginx cũng forward port 80 theo kiểu stream, thêm
`proxy_protocol on;` tương tự cho block đó.

## 4. Kiểm tra sau khi cấu hình

```bash
# Từ 2 mạng khác nhau (vd 4G + WiFi), vào web, rồi admin xem:
# /admin/sessions — 2 dòng phải hiện 2 IP public khác nhau.

# Hoặc check header app nhận được:
curl -s https://louis.vangioitutien.com/ -o /dev/null -w '%{http_code}\n'
# Vào Coolify → khogame → Logs, tìm dòng WARN "client IP = ..." —
# sau khi fix đúng, dòng này KHÔNG xuất hiện nữa (IP là public).
```

## 5. Khi nào cần đổi TRUSTED_PROXY_HOPS?

| Kiến trúc | Giá trị |
|---|---|
| client → Traefik → app (hiện tại) | `1` (mặc định) |
| client → Cloudflare/CDN → Traefik → app | `2` |
| client → CDN → nginx L7 → Traefik → app | `3` |

Set trong tab Environment Variables của Service khogame trên Coolify.
Lưu ý: CDN phải là lớp ngoài CÙNG và Traefik phải cấu hình
`forwardedHeaders.trustedIPs` tin XFF từ CDN, nếu không XFF sẽ bị
Traefik bỏ như hiện tại (Traefik v3 mặc định không tin XFF từ nguồn lạ).
