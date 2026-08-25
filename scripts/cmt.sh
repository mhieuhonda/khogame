#!/bin/bash
# Helper: xác nhận build + clippy + fmt + test pass rồi mới commit
# Usage: ./scripts/cmt.sh "commit message"
#
# Tự dò repo root (nơi có Cargo.toml) từ vị trí script — trước đây hardcode
# đường dẫn tuyệt đối /home/z/my-project/khogame khiến script fail trên VPS
# hay máy khác clone ở chỗ khác.
set -e
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"
if [ ! -f Cargo.toml ]; then
  echo "[cmt] Không tìm thấy Cargo.toml ở $REPO_ROOT — không phải repo khogame?"; exit 1
fi
source "$HOME/.cargo/env" 2>/dev/null || true

if cargo build 2>&1 | grep -q "^error"; then
  echo "[cmt] BUILD FAILED — từ chối commit"; cargo build 2>&1 | grep "^error" | head -5; exit 1
fi
echo "[cmt] build OK"

if ! cargo clippy --all-targets -- -D warnings > /tmp/clippy.log 2>&1; then
  echo "[cmt] CLIPPY FAILED — từ chối commit"; grep -E "^error" -A 5 /tmp/clippy.log | head -20; exit 1
fi
echo "[cmt] clippy OK"

cargo fmt --all
if ! cargo fmt --all -- --check > /dev/null 2>&1; then
  echo "[cmt] FMT FAILED"; exit 1
fi
echo "[cmt] fmt OK"

if cargo test 2>&1 | grep -E "^test result" | grep -v "0 passed; 0 failed" | grep -qv " 0 failed"; then
  echo "[cmt] TESTS FAILED — từ chối commit"; cargo test 2>&1 | grep -E "FAILED|panicked" | head -5; exit 1
fi
echo "[cmt] tests OK"

git add -A
git commit -m "$1"
echo "[cmt] committed: $(git rev-parse --short HEAD)"
