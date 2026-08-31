-- ============================================================================
-- Migration 032 — v3.5.1: sửa economy Mystery Box (audit task 5-e, HIGH)
-- ============================================================================
-- BUG: giá seed = 45 XP nhưng EV phần thưởng = ~79.5 XP (payout 10–149)
-- → mỗi hộp lãi ròng +34.5 XP, KHÔNG có cap ngày, XP chèn thẳng vào
-- user_xp_totals (bypass mọi daily cap của award_xp) → máy in XP vô hạn
-- (~4.100 XP/phút ở bucket mặc định 120 req/phút), phá leaderboard/level
-- và toàn bộ cân bằng economy.
--
-- FIX (2 lớp):
--   1) Migration này: giá 45 → 100 XP. EV payout 79.5 → house edge
--      +20.5 XP/hộp (~25.8%) — hộp giờ là "cờ bạc có lãi cho nhà",
--      mở vô hạn KHÔNG còn sinh XP ròng.
--   2) Tầng repo (shop.rs): cap 5 hộp/ngày/user + advisory lock —
--      phòng khi giá bị admin chỉnh lại thấp (DB là nguồn sự thật).
--
-- Lưu ý: KHÔNG sửa migration 023 (đã apply trên prod — sqlx checksum
-- validate sẽ fail nếu sửa file cũ). UPDATE idempotent, chạy 1 lần.
-- ============================================================================

UPDATE shop_items
   SET price = 100,
       description = 'Mở ra nhận ngẫu nhiên 10–150 XP. Giờ cần đen đủi lắm mới lời!'
 WHERE id = 'mystery_box';
