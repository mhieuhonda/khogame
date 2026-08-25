#!/bin/bash
# Helper: xác nhận build + test pass rồi mới commit (dùng trong marathon)
# Usage: ./scripts/cmt.sh "commit message"
set -e
cd /home/z/my-project/khogame
source "$HOME/.cargo/env"

if ! cargo build 2>&1 | grep -q "^error"; then
  echo "[cmt] build OK"
else
  echo "[cmt] BUILD FAILED — từ chối commit"
  cargo build 2>&1 | grep "^error" | head -5
  exit 1
fi

TEST_OUT=$(cargo test 2>&1 | grep -E "^test result" | grep -v "0 passed; 0 failed" || true)
if echo "$TEST_OUT" | grep -q "0 failed"; then
  echo "[cmt] tests OK"
else
  echo "[cmt] TESTS FAILED — từ chối commit"
  exit 1
fi

git add -A
git commit -m "$1"
echo "[cmt] committed: $(git rev-parse --short HEAD)"
