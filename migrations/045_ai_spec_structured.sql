-- ============================================================================
-- Migration 045 — v3.11.0: THÔNG SỐ AI AGENT CẤU TRÚC (structured spec)
-- ============================================================================
-- Theo yêu cầu chủ sở hữu: XÓA toàn bộ hệ "thông số chi tiết" cũ
-- (ai_agent_params — key/value tự do trộn lẫn Temperature/Top-p với trạng
-- thái hệ thống như rate-limit, TTL phiên, cơ chế cấp quyền → trình bày
-- lộn xộn, "tham số kích hoạt" bị hiểu sai hoàn toàn).
--
-- THAY BẰNG đúng 10 trường cấu trúc, ít chi tiết hơn, đúng nghĩa:
--   Model, Vendor, Khả năng, Nhà phát triển, Kiến trúc, Cửa sổ ngữ cảnh,
--   Output tối đa, Ngôn ngữ, Tổng tham số, Tham số kích hoạt.
--
-- Định nghĩa chuẩn (theo chủ sở hữu):
--   * TỔNG THAM SỐ  = toàn bộ số lượng trọng số có trong mô hình AI.
--   * THAM SỐ KÍCH HOẠT = số lượng tham số thực tế được tính toán để xử
--     lý MỘT đầu vào tại một thời điểm (kiến trúc MoE chỉ kích hoạt một
--     phân nhỏ expert mỗi token).
--
-- 7 trường mới nằm trên chính bảng ai_agent_profiles (1-1 với user, không
-- cần bảng phụ + không còn khái niệm public/private từng dòng — toàn bộ
-- hồ sơ AI đã có privacy_level public/anonymous kiểm soát hiển thị).
--
-- Idempotent: ADD COLUMN IF NOT EXISTS + UPDATE seed + DROP TABLE IF EXISTS.
-- ============================================================================

-- ============ 1) Cột spec cấu trúc ============
ALTER TABLE ai_agent_profiles
    ADD COLUMN IF NOT EXISTS developer      VARCHAR(100) NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS architecture   VARCHAR(150) NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS context_window VARCHAR(60)  NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS max_output     VARCHAR(60)  NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS languages      VARCHAR(200) NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS total_params   VARCHAR(60)  NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS active_params  VARCHAR(60)  NOT NULL DEFAULT '';

-- ============ 2) Seed GLM 5.3 — specs thật của dòng GLM-5 (Z.ai) ============
-- GLM-5 technical report: MoE 744B total / 40B active, 256 experts.
-- Context 204.800 tokens (~200K), output tối đa 131.072 tokens (~128K).
-- Giữ nguyên giá trị cũ đã đúng: context 200K, output 32K→128K chuẩn hoá
-- theo spec API thật của dòng GLM-5.
UPDATE ai_agent_profiles SET
    developer      = 'Z.ai (Zhipu AI)',
    architecture   = 'Mixture-of-Experts (MoE), 256 experts',
    context_window = '204.800 tokens (~200K)',
    max_output     = '131.072 tokens (~128K)',
    languages      = 'Đa ngôn ngữ: Tiếng Việt, English, 中文, 日本語…',
    total_params   = '744B (744 tỷ trọng số)',
    active_params  = '40B (40 tỷ trọng số mỗi đầu vào)',
    updated_at     = NOW()
WHERE user_id IN (SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53');

-- ============ 3) Xóa hệ params cũ ============
-- Bảng chỉ chứa dữ liệu seed/trình bày sai (không phải dữ liệu người
-- dùng cần giữ) — mọi code đã chuyển sang structured spec columns.
DROP TABLE IF EXISTS ai_agent_params;

-- ============ 4) GUARD: khẳng định cột mới tồn tại + seed đã chạy ============
DO $$
DECLARE
    cols INT;
    glm_spec RECORD;
BEGIN
    SELECT count(*) INTO cols
      FROM information_schema.columns
     WHERE table_name = 'ai_agent_profiles'
       AND column_name IN ('developer','architecture','context_window',
                           'max_output','languages','total_params','active_params');

    IF cols <> 7 THEN
        RAISE EXCEPTION 'Migration 045: thiếu %/7 cột spec mới trên ai_agent_profiles', 7 - cols;
    END IF;

    SELECT developer, total_params, active_params INTO glm_spec
      FROM ai_agent_profiles
     WHERE user_id = (SELECT id FROM users WHERE google_sub = 'ai_agent:default-glm53');

    IF glm_spec.developer IS NULL OR glm_spec.developer = ''
       OR glm_spec.total_params = '' OR glm_spec.active_params = '' THEN
        RAISE EXCEPTION 'Migration 045: seed GLM 5.3 chưa chạy (developer/total_params/active_params rỗng)';
    END IF;
END $$;
