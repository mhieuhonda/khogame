-- ============================================================
-- Migration 014: image_url column cho github_repos
--
-- Thêm cột image_url TEXT để user có thể upload ảnh thumbnail
-- custom cho repo GitHub (thay vì dùng thumbnail tự sinh từ GitHub).
--
-- DEFAULT '' (empty string) để:
--   1) Tương thích lùi — repo cũ không có ảnh custom vẫn load OK
--      (code check `is_empty()` để fallback về GitHub thumbnail).
--   2) NOT NULL — code Rust có thể map trực tiếp sang String thay vì
--      Option<String>, giảm boilerplate.
--
-- Không cần index vì không query theo image_url.
-- ============================================================

ALTER TABLE github_repos
    ADD COLUMN IF NOT EXISTS image_url TEXT NOT NULL DEFAULT '';
