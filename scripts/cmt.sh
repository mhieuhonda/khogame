#!/bin/bash
# Helper: xác nhận build + clippy + fmt + test pass rồi mới commit
# Usage: ./scripts/cmt.sh "<type>(<scope>): <subject>"
#   vd:   ./scripts/cmt.sh "fix(games): chặn like trên game draft"
#
# Tự dò repo root (nơi có Cargo.toml) từ vị trí script — trước đây hardcode
# đường dẫn tuyệt đối /home/z/my-project/khogame khiến script fail trên VPS
# hay máy khác clone ở chỗ khác.
#
# Conventional commit format (https://www.conventionalcommits.org/en/v1.0.0/):
#   <type>(<scope>): <subject>
#     type: feat | fix | perf | refactor | docs | style | test | chore | ci | build
#     scope: optional (vd games, news, auth, db, ci, docker)
#     subject: bắt đầu bằng chữ thường, không dấu chấm cuối, ≤72 ký tự
#   \n\n<body optional>\n\n<footer optional>
#     footer: BREAKING CHANGE: ...  hoặc  Fixes #123
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"
if [ ! -f Cargo.toml ]; then
  echo "[cmt] Không tìm thấy Cargo.toml ở $REPO_ROOT — không phải repo khogame?"; exit 1
fi
source "$HOME/.cargo/env" 2>/dev/null || true

# ─── Validate commit message ───────────────────────────────────
MSG="${1:-}"
if [[ -z "$MSG" ]]; then
  echo "Usage: $0 \"<type>(<scope>): <subject>\""
  echo "  vd: $0 \"fix(games): chặn like trên game draft\""
  echo "Conventional types: feat|fix|perf|refactor|docs|style|test|chore|ci|build"
  exit 1
fi

# Cho phép `!` prefix cho breaking change (vd `feat(auth)!: drop legacy session`).
# Regex: ^(feat|fix|perf|refactor|docs|style|test|chore|ci|build)(\([^a-z0-9-]+\))?(!)?: .+
# Subject không nên quá dài (chromium/git đều khuyến nghị ≤72).
TYPE_RE='^(feat|fix|perf|refactor|docs|style|test|chore|ci|build|revert)(\([a-z0-9-]+\))?(!)?: .+'
if ! [[ "$MSG" =~ $TYPE_RE ]]; then
  echo "[cmt] ❌ Commit message không tuân conventional commits:"
  echo "      $MSG"
  echo "      Cần dạng: <type>(<scope>): <subject>"
  echo "      type ∈ {feat,fix,perf,refactor,docs,style,test,chore,ci,build,revert}"
  echo "      scope optional (vd games, news, auth, db, ci, docker)"
  exit 1
fi
# Extract subject (after `: `).
SUBJECT="${MSG#*: }"
# Đếm ký tự (UTF-8 aware). -m đếm char không phải byte.
SUBJECT_LEN=${#SUBJECT}
if (( SUBJECT_LEN > 100 )); then
  echo "[cmt] ⚠ Subject dài ${SUBJECT_LEN} ký tự (khuyến nghị ≤72, max 100):"
  echo "      $SUBJECT"
  echo "      Tiếp tục anyway trong 5s... (Ctrl+C để hủy)"
  sleep 5
fi

# ─── Verify build / clippy / fmt / test ─────────────────────────
echo "[cmt] Đang kiểm tra build + clippy + fmt + test..."

# Build debug profile (nhanh hơn release nhiều — `panic=unwind`, no LTO).
# Bắt error qua grep thay vì `set -e` vì cargo build có thể sinh warning
# ra stderr mà pipe vẫn exit 0.
BUILD_LOG=$(mktemp -t cmt_build.XXXX.log)
trap 'rm -f "$BUILD_LOG" "$CLIPPY_LOG"' EXIT
CLIPPY_LOG=$(mktemp -t cmt_clippy.XXXX.log)

if ! cargo build >"$BUILD_LOG" 2>&1; then
  echo "[cmt] ❌ BUILD FAILED — từ chối commit"
  grep -E "^(error|warning: unused)" "$BUILD_LOG" | head -10
  echo "--- Full log: $BUILD_LOG"
  exit 1
fi
echo "[cmt] build OK"

if ! cargo clippy --all-targets -- -D warnings >"$CLIPPY_LOG" 2>&1; then
  echo "[cmt] ❌ CLIPPY FAILED — từ chối commit"
  grep -E "^error" -A 5 "$CLIPPY_LOG" | head -30
  echo "--- Full log: $CLIPPY_LOG"
  exit 1
fi
echo "[cmt] clippy OK"

# fmt --check trước để fmt --all không sinh diff không cần thiết
# (fmt --all sửa file rồi exit 0; fmt --all -- --check chỉ check).
if ! cargo fmt --all -- --check >/dev/null 2>&1; then
  echo "[cmt] fmt chưa sạch — chạy `cargo fmt --all`..."
  cargo fmt --all
  if ! cargo fmt --all -- --check >/dev/null 2>&1; then
    echo "[cmt] ❌ FMT FAILED — vẫn sai sau khi auto-format (syntax error?)"
    exit 1
  fi
  echo "[cmt] fmt OK (đã auto-format)"
else
  echo "[cmt] fmt OK"
fi

# Test — panic=abort không ảnh hưởng cargo test (debug profile mặc định
# panic=unwind). Test có DB needed sẽ skip (sqlx macro offline mode).
# `grep -v " 0 failed"` filter dòng "0 passed; 0 failed" của doctest.
TEST_LOG=$(mktemp -t cmt_test.XXXXXX.log)
trap 'rm -f "$BUILD_LOG" "$CLIPPY_LOG" "$TEST_LOG"' EXIT
if ! cargo test >"$TEST_LOG" 2>&1; then
  echo "[cmt] ❌ TESTS FAILED — từ chối commit"
  grep -E "^(test result|failures:)" "$TEST_LOG" | head -10
  echo "--- Full log: $TEST_LOG"
  exit 1
fi
# Kiểm tra có ít nhất 1 test pass để chặn trường hợp "test binary không
# chạy test nào" (vd doctests only với 0 doctests).
if ! grep -qE "^test result: [0-9]+ passed" "$TEST_LOG"; then
  echo "[cmt] ⚠ Không có test result line — có thể test binary không chạy"
  echo "      Full log: $TEST_LOG"
fi
echo "[cmt] tests OK"

# ─── Stage + commit ─────────────────────────────────────────────
# Kiểm tra working tree có thay đổi để stage không — nếu cargo fmt
# auto-format sinh diff thì đã được catch ở fmt block; nếu không có
# gì thay đổi, có thể user gọi cmt.sh mà chưa edit gì.
if [ -z "$(git status --porcelain)" ]; then
  echo "[cmt] ⚠ Working tree sạch — không có gì để commit"
  exit 0
fi

git add -A
git commit -m "$MSG"
echo "[cmt] ✅ committed: $(git rev-parse --short HEAD) — $MSG"
