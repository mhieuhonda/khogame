-- ============================================================================
-- Migration 038 — v3.8.0: BẬT/TẮT KHUNG AVATAR (fix "không thể tắt khung")
-- ============================================================================
-- Vấn đề (user report, qua nhiều bản chưa fix): sau khi mua khung avatar,
-- khung HIỂN THỊ vĩnh viễn đến hết hạn — KHÔNG có cách nào tắt. Người dùng
-- muốn lúc đeo lúc bỏ (ảnh đại diện gốc) mà không mất thời hạn đã mua.
--
-- Fix: cột avatar_frame_disabled trên user_boosts (mặc định FALSE).
--   * FALSE (mặc định): khung hiện như cũ — hành vi hiện tại giữ nguyên.
--   * TRUE: tạm ẩn khung (avatar render bình thường) — THỜI HẠN VẪN CHẠY.
-- Người dùng bật lại bất cứ lúc nào qua nút trên trang hồ sơ của mình.
--
-- Toggle endpoint: POST /profile/avatar-frame/toggle (handlers/profile.rs).
-- ============================================================================

ALTER TABLE user_boosts
    ADD COLUMN IF NOT EXISTS avatar_frame_disabled BOOLEAN NOT NULL DEFAULT FALSE;
