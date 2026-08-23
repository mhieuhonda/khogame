-- ============================================
-- 002: GitHub Repos, Admin Logs, Settings, Daily Stats
-- ============================================

-- GitHub repositories do nguoi dung dang
CREATE TYPE repo_status AS ENUM ('pending', 'approved', 'hidden');

CREATE TABLE github_repos (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    game_id         UUID REFERENCES games(id) ON DELETE SET NULL,
    owner           VARCHAR(100) NOT NULL,
    repo_name       VARCHAR(150) NOT NULL,
    description     TEXT DEFAULT '',
    homepage        TEXT DEFAULT '',
    primary_language VARCHAR(50) DEFAULT '',
    stars           INTEGER NOT NULL DEFAULT 0,
    forks           INTEGER NOT NULL DEFAULT 0,
    open_issues     INTEGER NOT NULL DEFAULT 0,
    pushed_at       TIMESTAMPTZ,
    status          repo_status NOT NULL DEFAULT 'approved',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner, repo_name)
);

CREATE INDEX idx_repos_status ON github_repos(status);
CREATE INDEX idx_repos_user ON github_repos(user_id);
CREATE INDEX idx_repos_stars ON github_repos(stars DESC);
CREATE INDEX idx_repos_updated ON github_repos(updated_at DESC);
CREATE TRIGGER trigger_repos_updated BEFORE UPDATE ON github_repos
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- Audit log cho hanh dong admin
CREATE TABLE admin_logs (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    admin_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    action      VARCHAR(100) NOT NULL,
    target_type VARCHAR(50) DEFAULT '',
    target_id   VARCHAR(100) DEFAULT '',
    detail      TEXT DEFAULT '',
    ip          VARCHAR(45),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_admin_logs_created ON admin_logs(created_at DESC);
CREATE INDEX idx_admin_logs_admin ON admin_logs(admin_id);

-- Site settings key-value
CREATE TABLE settings (
    key        VARCHAR(100) PRIMARY KEY,
    value      TEXT NOT NULL DEFAULT '',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL
);

-- Thong ke theo ngay cho dashboard chart
CREATE TABLE daily_stats (
    day       DATE NOT NULL,
    game_id   UUID REFERENCES games(id) ON DELETE CASCADE,
    views     INTEGER NOT NULL DEFAULT 0,
    downloads INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (day, game_id)
);

CREATE INDEX idx_daily_stats_day ON daily_stats(day DESC);

-- Gia tri mac dinh
INSERT INTO settings (key, value) VALUES
    ('site_name', 'Kho Game'),
    ('site_description', 'Nền tảng chia sẻ game độc lập cho cộng đồng Việt Nam'),
    ('maintenance_mode', 'off'),
    ('announcement', ''),
    ('announcement_type', 'info'),
    ('footer_text', 'Nền tảng chia sẻ game độc lập cho cộng đồng Việt Nam.')
ON CONFLICT (key) DO NOTHING;
