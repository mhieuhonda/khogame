-- 026 — v3.3.0: BẢNG MATCHMAKING cho 2 game arcade (Oẳn tù tì + Nối từ).
--
-- Thiết kế:
-- * Idempotent: IF NOT EXISTS toàn bộ.
-- * rps_plays / word_chain_plays (migration 024) VẪN được ghi cho mỗi
--   nước chơi PvP — giữ nguyên thống kê lifetime + huy hiệu rps_* /
--   word_chain_* (check_and_award đếm từ 2 bảng đó).
-- * `rps_matches`: 1 ván = 1 hàng. player1 tạo hàng khi POST /rps/play,
--   player2 JOIN bằng cách POST /rps/play — UPDATE hàng (SELECT ... FOR
--   UPDATE SKIP LOCKED chống 2 người cùng join 1 match).
-- * `word_chain_matches`: trận nhiều nước. `words_used` TEXT[] chặn tái
--   sử dụng từ trong trận; `turn_user_id` + `move_deadline` xử lý luân
--   phiên + timeout (timeout = thua, được poll GET /word-chain/match/{id}/status
--   thực thi server-side — client không quyết định kết quả).
-- * `is_ai_fallback`: match hết 90s không có người → tự ghép GLM 5.3
--   (AI Agent mặc định, migration 027). Cột để thống kê + hiển thị badge.

CREATE TABLE IF NOT EXISTS rps_matches (
    id             BIGSERIAL PRIMARY KEY,
    player1_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    player2_id     UUID REFERENCES users(id) ON DELETE CASCADE,
    player1_choice VARCHAR(8) NOT NULL
                   CHECK (player1_choice IN ('rock','paper','scissors')),
    player2_choice VARCHAR(8)
                   CHECK (player2_choice IS NULL OR player2_choice IN ('rock','paper','scissors')),
    status         VARCHAR(16) NOT NULL DEFAULT 'waiting'
                   CHECK (status IN ('waiting','finished','cancelled')),
    winner_id      UUID REFERENCES users(id) ON DELETE SET NULL,
    is_ai_fallback BOOLEAN NOT NULL DEFAULT FALSE,
    xp1            INT NOT NULL DEFAULT 0 CHECK (xp1 >= 0),
    xp2            INT NOT NULL DEFAULT 0 CHECK (xp2 >= 0),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Hàng đợi ghép: query chờ là WHERE status='waiting' ORDER BY created_at
-- (index partial rất nhỏ — chỉ chứa match đang chờ).
CREATE INDEX IF NOT EXISTS idx_rps_matches_waiting
    ON rps_matches(created_at) WHERE status = 'waiting';
CREATE INDEX IF NOT EXISTS idx_rps_matches_p1
    ON rps_matches(player1_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_rps_matches_p2
    ON rps_matches(player2_id, created_at DESC);

CREATE TABLE IF NOT EXISTS word_chain_matches (
    id             BIGSERIAL PRIMARY KEY,
    player1_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    player2_id     UUID REFERENCES users(id) ON DELETE CASCADE,
    status         VARCHAR(16) NOT NULL DEFAULT 'waiting'
                   CHECK (status IN ('waiting','active','finished','cancelled')),
    winner_id      UUID REFERENCES users(id) ON DELETE SET NULL,
    -- Luân phiên: NULL khi waiting; = user được phép đánh tiếp theo khi active
    turn_user_id   UUID REFERENCES users(id) ON DELETE SET NULL,
    -- NULL = nước đầu (đánh từ bất kỳ); sau đó = ký tự cuối của từ vừa nối
    current_letter CHAR(1),
    -- Chặn reuse từ trong 1 trận: 2 người sẽ lặp "anh"↔"hoa" vô hạn nếu không
    words_used     TEXT[] NOT NULL DEFAULT '{}',
    move_deadline  TIMESTAMPTZ,
    is_ai_fallback BOOLEAN NOT NULL DEFAULT FALSE,
    xp1            INT NOT NULL DEFAULT 0 CHECK (xp1 >= 0),
    xp2            INT NOT NULL DEFAULT 0 CHECK (xp2 >= 0),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_word_chain_matches_waiting
    ON word_chain_matches(created_at) WHERE status = 'waiting';
CREATE INDEX IF NOT EXISTS idx_word_chain_matches_p1
    ON word_chain_matches(player1_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_word_chain_matches_p2
    ON word_chain_matches(player2_id, created_at DESC);
