# Chính sách Bảo mật — Louis Space

## Các phiên bản được hỗ trợ

Louis Space là một dịch vụ web triển khai liên tục (CD). Chỉ phiên bản
mới nhất đang chạy trên production được hỗ trợ bảo mật.

| Phiên bản | Được hỗ trợ |
|-----------|-------------|
| latest (main) | ✅ |
| các bản cũ hơn | ❌ |

## Báo cáo lỗ hổng

**Vui lòng KHÔNG báo cáo lỗ hổng qua GitHub issue công khai.**

Gửi báo cáo qua email: **khongdich.admin@gmail.com**

Thông tin liên hệ chính thức cũng được công bố tại
[`/.well-known/security.txt`](https://louis.vangioitutien.com/.well-known/security.txt)
theo RFC 9116.

Báo cáo nên bao gồm:

- Mô tả lỗ hổng và ảnh hưởng tiềm năng
- Các bước tái hiện (repro) cụ thể, kèm request/response nếu có
- Phiên bản/URL bị ảnh hưởng
- Bất kỳ nghiên cứu PoC/exploit nào (không gây hại cho dữ liệu thật)

## Cam kết của chúng tôi

- Xác nhận tiếp nhận báo cáo trong vòng **72 giờ**
- Đánh giá và phân loại mức độ nghiêm trọng trong vòng **7 ngày**
- Bản vá cho lỗ hổng nghiêm trọng được ưu tiên deploy sớm nhất có thể
- Công khai ghi nhận (credit) nhà nghiên cứu nếu được cho phép

## Phạm vi

Các lĩnh vực chúng tôi quan tâm nhất:

- Đăng nhập/phiên làm việc (Google OAuth, session cookie)
- Bảo mật phím tắt AI Agent (endpoint `/ai/*`, `/auth/ai/*`)
- SQL injection (sqlx đã dùng bind parameter — nhưng vẫn báo nếu thấy lọt)
- XSS qua nội dung do người dùng tạo (bình luận, mô tả game, markdown)
- IDOR — truy cập tài nguyên của người khác qua UUID/slug đoán được
- SSRF qua các URL user-submitted (cover image, screenshot, link tải)

**Ngoài phạm vi:** báo lỗi UI thuần, spam, brute-force rate-limit
đã có biện pháp, lỗ hổng của dịch vụ hạ tầng bên thứ ba (Google,
Cloudflare, Coolify) — báo cho nhà cung cấp đó.
