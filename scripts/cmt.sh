#!/bin/bash
# Helper: xác nhận build + clippy + fmt + test pass rồi mới commit
# Usage: ./scripts/cmt.sh "commit message"
set -e
cd /home/z/my-project/khogame
source "$HOME/.cargo/env"

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
