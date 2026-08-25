#!/bin/bash
# Helper: tạo release mới từ CHANGELOG.md
#
# Quy trình (giữ nguyên quy ước v0.1 → v0.7 của repo):
#   1. Bump version trong Cargo.toml (và Cargo.lock) + version badge README
#   2. Cắt phần [Unreleased] trong CHANGELOG.md thành [x.y.z] - ngày
#   3. git commit "chore(release): x.y.z"
#   4. git tag vx.y.z + push tag (CD tự build image semver + deploy)
#
# Usage: ./scripts/release.sh 0.8.0
set -euo pipefail

VERSION="${1:?Usage: $0 <version> (vd 0.8.0)}"
# Định dạng semver
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "[release] Version phải semver: x.y.z (được: $VERSION)"; exit 1
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"
if [ ! -f Cargo.toml ] || [ ! -f CHANGELOG.md ]; then
    echo "[release] Không tìm thấy Cargo.toml/CHANGELOG.md ở $REPO_ROOT"; exit 1
fi

# Kiểm tra working tree sạch (release không trộn thay đổi lơ lửng)
if [ -n "$(git status --porcelain)" ]; then
    echo "[release] Working tree không sạch — commit/stash trước khi release:"; git status --short; exit 1
fi

# Kiểm tra tag chưa tồn tại
if git rev-parse -q --verify "v$VERSION" >/dev/null; then
    echo "[release] Tag v$VERSION đã tồn tại"; exit 1
fi

TODAY=$(date +%F)

# 1) Bump Cargo.toml version
sed -i.bak "s/^version = \"[0-9]*\.[0-9]*\.[0-9]*\"$/version = \"$VERSION\"/" Cargo.toml && rm -f Cargo.toml.bak
if [ -f Cargo.lock ]; then
    # Update version của package khogame trong lock (cargo tự làm khi build,
    # làm thủ công để tag khớp lock ngay)
    python3 - "$VERSION" << 'PYEOF'
import re, sys
ver = sys.argv[1]
content = open('Cargo.lock').read()
content = re.sub(
    r'(name = "khogame"\nversion = ")[^"]+(")',
    rf'\g<1>{ver}\g<2>',
    content, count=1
)
open('Cargo.lock', 'w').write(content)
PYEOF
fi

# 2) README badge version
sed -i.bak "s/badge\/version-[0-9.]*-/badge\/version-$VERSION-/" README.md 2>/dev/null && rm -f README.md.bak

# 3) Cắt [Unreleased] → [x.y.z] - ngày
python3 - "$VERSION" "$TODAY" << 'PYEOF'
import sys
ver, today = sys.argv[1], sys.argv[2]
content = open('CHANGELOG.md').read()
if '## [Unreleased]' not in content:
    print('[release] CHANGELOG không có section [Unreleased]'); sys.exit(1)
content = content.replace('## [Unreleased]', f'## [{ver}] - {today}', 1)
# Thêm [Unreleased] mới ngay sau header cho batch tiếp theo
content = content.replace(
    'tuân thủ [Semantic Versioning](https://semver.org/spec/v2.0.0.html).\n',
    'tuân thủ [Semantic Versioning](https://semver.org/spec/v2.0.0.html).\n\n## [Unreleased]\n',
    1
)
open('CHANGELOG.md', 'w').write(content)
PYEOF

git add -A
git commit -q -m "chore(release): v$VERSION

Cắt CHANGELOG Unreleased → $VERSION ($TODAY), bump Cargo.toml +
README badge. Tag vx$VERSION sẽ trigger CD build image semver."
git tag "v$VERSION"
echo "[release] Đã commit + tag v$VERSION."
echo "[release] Kiểm tra lại rồi push:  git push origin main v$VERSION"
