#!/bin/bash
# Helper: tạo release mới từ CHANGELOG.md
#
# Quy trình (giữ nguyên quy ước v0.1 → v1.0 của repo):
#   1. Bump version trong Cargo.toml + Cargo.lock + README badge
#   2. Cắt phần [Unreleased] trong CHANGELOG.md thành [x.y.z] - ngày
#   3. git commit "chore(release): x.y.z"
#   4. git tag ANNOTATED "vx.y.z" với message trích từ CHANGELOG
#   5. (tuỳ chọn) gh release create với --notes-file trích từ CHANGELOG
#   6. push origin main v$VERSION (CD tự build image semver + deploy)
#
# Usage:
#   ./scripts/release.sh 0.8.0             # commit + tag (không push, không gh release)
#   ./scripts/release.sh 0.8.0 --push       # commit + tag + push origin main + tag
#   ./scripts/release.sh 0.8.0 --gh         # tạo GitHub Release với notes từ CHANGELOG
#   ./scripts/release.sh 0.8.0 --push --gh  # đầy đủ (chạy sau khi đã review)
set -euo pipefail

# ─── Args parse ─────────────────────────────────────────────
VERSION="${1:-}"
DO_PUSH=0
DO_GH=0
for arg in "${@:2}"; do
  case "$arg" in
    --push) DO_PUSH=1 ;;
    --gh)   DO_GH=1   ;;
    *) echo "[release] Flag không nhận dạng: $arg"; exit 1 ;;
  esac
done

if [[ -z "$VERSION" ]]; then
  echo "Usage: $0 <version> [--push] [--gh]  (vd 0.8.0)"
  exit 1
fi

# Định dạng semver — chặn cả pre-release suffix cho đơn giản (CD tag
# pattern `v*` match `v0.8.0-rc.1` cũng OK nhưng script này không sinh).
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "[release] Version phải semver x.y.z (được: $VERSION) — pre-release dùng git tag trực tiếp"
  exit 1
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"
if [ ! -f Cargo.toml ] || [ ! -f CHANGELOG.md ]; then
  echo "[release] Không tìm thấy Cargo.toml/CHANGELOG.md ở $REPO_ROOT"; exit 1
fi

# Kiểm tra working tree sạch (release không trộn thay đổi lơ lửng).
# Bao gồm untracked file — nếu có file mới chưa add, release có thể
# quên đưa vào tag.
if [ -n "$(git status --porcelain)" ]; then
  echo "[release] Working tree không sạch — commit/stash trước khi release:"
  git status --short
  exit 1
fi

# Kiểm tra tag chưa tồn tại (annotated + lightweight đều bắt).
if git rev-parse -q --verify "refs/tags/v$VERSION" >/dev/null; then
  echo "[release] Tag v$VERSION đã tồn tại"; exit 1
fi

# Kiểm tra trên branch main — release ngoài main dễ tạo tag không fast-forward.
BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [[ "$BRANCH" != "main" ]]; then
  echo "[release] Đang ở branch '$BRANCH', không phải 'main' — checkout main trước khi release"
  exit 1
fi

# Kiểm tra upstream đã synced (tránh tag local cũ, push sẽ reject).
if git rev-parse --abbrev-ref --symbolic-full-name '@{u}' >/dev/null 2>&1; then
  if ! git diff --quiet '@{u}' HEAD; then
    echo "[release] Local main lệch với upstream — git pull --ff-only trước khi release"
    exit 1
  fi
fi

TODAY=$(date +%F)
TAG_NAME="v$VERSION"
NOTES_FILE="$(mktemp -t release_notes.XXXXXX.md)"
trap 'rm -f "$NOTES_FILE"' EXIT

# 1) Bump Cargo.toml version (chỉ thay line `version = "x.y.z"` ở root
#    [package], không đụng dependencies version).
sed -i.bak "s/^version = \"[0-9]*\.[0-9]*\.[0-9]*\"$/version = \"$VERSION\"/" Cargo.toml
rm -f Cargo.toml.bak

# Update Cargo.lock package version — cargo tự sync khi build nhưng
# làm thủ công để tag khớp với lock ngay (nếu operator git tag mà
# chưa build, lock vẫn đúng).
if [ -f Cargo.lock ]; then
  python3 - "$VERSION" << 'PYEOF'
import re, sys
ver = sys.argv[1]
content = open('Cargo.lock').read()
# Chỉ thay version của package "khogame" (match name+version liền nhau).
pattern = r'(name = "khogame"\nversion = ")[^"]+(")'
new_content, n = re.subn(pattern, rf'\g<1>{ver}\g<2>', content, count=1)
if n == 0:
    print('[release] ⚠ Cargo.lock: không tìm thấy package khogame — không bump lock', file=sys.stderr)
    sys.exit(0)  # không fail — có thể lock format khác
open('Cargo.lock', 'w').write(new_content)
PYEOF
fi

# 2) README badge version (chỉ thay dạng `badge/version-x.y.z-`).
sed -i.bak "s|badge/version-[0-9][0-9.]*-|badge/version-$VERSION-|" README.md 2>/dev/null || true
rm -f README.md.bak

# 3) Cắt [Unreleased] → [x.y.z] - ngày, thêm [Unreleased] mới.
python3 - "$VERSION" "$TODAY" << 'PYEOF'
import sys
ver, today = sys.argv[1], sys.argv[2]
content = open('CHANGELOG.md').read()
if '## [Unreleased]' not in content:
    print('[release] CHANGELOG không có section [Unreleased] — thêm trước khi release', file=sys.stderr)
    sys.exit(1)
content = content.replace('## [Unreleased]', f'## [{ver}] - {today}', 1)
# Thêm [Unreleased] mới ngay sau header doc cho batch tiếp theo.
content = content.replace(
    'tuân thủ [Semantic Versioning](https://semver.org/spec/v2.0.0.html).\n',
    'tuân thủ [Semantic Versioning](https://semver.org/spec/v2.0.0.html).\n\n## [Unreleased]\n',
    1
)
open('CHANGELOG.md', 'w').write(content)
PYEOF

# 4) Trích section [x.y.z] - ngày từ CHANGELOG → release notes file.
python3 - "$VERSION" "$NOTES_FILE" << 'PYEOF'
import re, sys
ver, path = sys.argv[1], sys.argv[2]
content = open('CHANGELOG.md').read()
# Match `## [x.y.z] - YYYY-MM-DD\n ... đến khi gặp `## [` tiếp theo hoặc EOF.
pattern = rf'## \[{re.escape(ver)}\][^\n]*\n(.*?)(?=\n## \[|\Z)'
m = re.search(pattern, content, re.DOTALL)
if not m:
    print(f'[release] Không tìm thấy section [{ver}] trong CHANGELOG', file=sys.stderr)
    sys.exit(1)
notes = m.group(1).strip()
# Prepend header để note đọc đẹp trên GitHub Release page.
out = f'# Louis Space v{ver} — {sys.argv[1] and ""}\n\n{notes}\n'
# Simplify header (avoid empty conditional).
out = f'# Louis Space v{ver}\n\n{notes}\n'
open(path, 'w').write(out)
print(f'[release] Release notes ({len(notes.splitlines())} dòng) → {path}')
PYEOF

# 5) Commit các file bump + CHANGELOG.
git add -A
git commit -q -m "chore(release): v$VERSION

Cắt CHANGELOG Unreleased → $VERSION ($TODAY), bump Cargo.toml +
Cargo.lock + README badge. Tag $TAG_NAME sẽ trigger CD build image semver."

# 6) Tạo ANNOTATED tag (không phải lightweight) — annotated tag lưu:
#   - tagger name + email + date
#   - tag message (chỉ ra release notes tóm tắt)
#   → `git show v0.8.0` hiển thị metadata đầy đủ; `gh release` tự đọc
#   annotated tag message nếu không pass --notes.
# Lightweight tag (`git tag v0.8.0`) không có metadata → `git describe`
# vẫn hoạt động nhưng `gh release view` hiển thị rỗng.
# Annotated tag cũng cần để GitHub Release khớp với tag commit message
# khi repo enable "Automatically generate release notes".
git tag -a "$TAG_NAME" -m "Release $TAG_NAME

$(head -50 "$NOTES_FILE")"

echo "[release] Đã commit + tag $TAG_NAME (annotated)."
echo

# 7) (tuỳ chọn) push origin main + tag.
if [[ "$DO_PUSH" == "1" ]]; then
  echo "[release] Pushing main + tag $TAG_NAME..."
  git push origin main
  git push origin "$TAG_NAME"
  echo "[release] Push xong — CD sẽ tự trigger build image semver."
else
  echo "[release] Kiểm tra lại rồi push:  git push origin main $TAG_NAME"
fi

# 8) (tuỳ chọn) tạo GitHub Release với notes từ CHANGELOG.
# `--verify-tag` chặn tạo release nếu tag không tồn tại local/remote
# (race với `git push`).
# `--notes-file` dùng file release notes đã trích — tránh `--generate-notes`
# (auto-gen của GitHub chỉ list PR, không có prose tiếng Việt từ CHANGELOG).
if [[ "$DO_GH" == "1" ]]; then
  if ! command -v gh >/dev/null 2>&1; then
    echo "[release] ⚠ gh CLI không cài đặt — bỏ qua tạo GitHub Release."
    echo "         Cài: https://cli.github.com/  hoặc tạo release thủ công:"
    echo "         https://github.com/mhieuhonda/khogame/releases/new?tag=$TAG_NAME"
    exit 0
  fi
  if ! gh auth status >/dev/null 2>&1; then
    echo "[release] ⚠ gh chưa auth — chạy 'gh auth login' trước. Bỏ qua tạo release."
    exit 0
  fi
  if [[ "$DO_PUSH" != "1" ]]; then
    echo "[release] ⚠ Tạo GitHub Release cần tag đã push (annotated tag remote)."
    echo "         Chạy lại với --push --gh, hoặc push thủ công trước."
    exit 1
  fi
  echo "[release] Tạo GitHub Release $TAG_NAME với notes từ CHANGELOG..."
  gh release create "$TAG_NAME" \
    --verify-tag \
    --notes-file "$NOTES_FILE" \
    --title "Louis Space $TAG_NAME"
  echo "[release] GitHub Release tạo xong: https://github.com/mhieuhonda/khogame/releases/tag/$TAG_NAME"
fi
