-- ============================================
-- 019: Mạng xã hội trên hồ sơ người dùng (v2.7.0)
-- ============================================
-- Thiết kế: bảng riêng `user_social_links` thay vì thêm cột JSONB
-- trực tiếp vào `users`. Lý do AN TOÀN CHO PROD:
--   * Mọi SELECT hiện tại của FromRow<User> / UserWithGameCount liệt kê
--     cột tường minh (~15 query). Thêm cột vào `users` mà sót 1 query
--     nào không cập nhật list cột → sqlx trả ColumnNotFound lúc RUNTIME
--     → 500 trên đúng trang đó (bug v1.4.0 từng xảy ra với cột tracking).
--   * Bảng riêng + PRIMARY KEY (user_id) = 1 row/user, SELECT ngoài
--     không đụng các query cũ → zero regression rủi ro.
--   * 1 row trống mặc định: JOIN/lookup riêng rẻ, handler chạy song song
--     trong tokio::join! nên không tăng thêm round-trip tuần tự.
-- Nền tảng hỗ trợ (10): github, facebook, zalo, discord, youtube,
-- tiktok, instagram, twitter (x), telegram, website.
-- `links` là JSON object {"platform_id": "https://..."} — platform_id
-- do server kiểm soát (validate trước khi lưu), value là URL hợp lệ
-- đã qua allowlist hostname từng nền tảng (xem SocialLinks::validate).

CREATE TABLE user_social_links (
    user_id     UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    links       JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Trigger giữ updated_at đúng chuẩn các bảng khác (migration 001 tạo
-- hàm update_updated_at dùng chung).
CREATE TRIGGER trigger_user_social_links_updated BEFORE UPDATE ON user_social_links
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();
