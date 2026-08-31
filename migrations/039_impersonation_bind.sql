-- ============================================================================
-- Migration 039 — v3.8.0: BIND impersonation ticket vào session (audit F4)
-- ============================================================================
-- Vấn đề (security audit F4 — MEDIUM): ticket `kg_impersonator` là bearer
-- credential thuần — ai cầm giá trị ticket (36 ký tự UUID) chưa dùng đều
-- đổi được thành session ADMIN mới 4 giờ qua POST /impersonate/stop hoặc
-- /auth/logout (2 endpoint public, không yêu cầu credential khác).
--
-- Fix: cột bound_session_hash lưu hash (SHA-256) của session AI Agent mà
-- ticket tạo ra. Khi redeem, yêu cầu kg_session cookie hiện tại của
-- request PHẢI khớp hash đó:
--   * Thief chỉ có ticket cookie (không có cookie session AI) → từ chối.
--   * Browser thật của admin giữ cả cặp cookie → redeem bình thường.
--   * Ticket cũ (NULL — tạo trước migration này) → vẫn chấp nhận như legacy
--     (chỉ tồn tại tối đa 2h sau deploy vì TTL ticket = 2h), kèm warn log.
--
-- TTL session restore cũng giảm 4h → 2h cho bằng TTL ticket.
-- ============================================================================

ALTER TABLE impersonation_tickets
    ADD COLUMN IF NOT EXISTS bound_session_hash TEXT;
