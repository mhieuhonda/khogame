#!/usr/bin/env bash
# ============================================================
# setup-branch-protection.sh
# Cài đặt branch protection cho main — chỉ admin/PAT holder
# mới push trực tiếp được. Người khác phải tạo branch → PR → review.
#
# Cách dùng:
#   GH_TOKEN=<your_pat> ./scripts/setup-branch-protection.sh
#
# Hoặc nếu đã đăng nhập GitHub CLI:
#   ./scripts/setup-branch-protection.sh
#
# Yêu cầu: PAT có quyền repo:admin (full quyền như user đã cấp).
# ============================================================
set -euo pipefail

# Repo info
OWNER="mhieuhonda"
REPO="khogame"
BRANCH="main"

# Token: ưu tiên GH_TOKEN, sau đó gh auth token
if [[ -z "${GH_TOKEN:-}" ]]; then
  if command -v gh >/dev/null 2>&1; then
    GH_TOKEN="$(gh auth token 2>/dev/null || true)"
  fi
fi
if [[ -z "${GH_TOKEN:-}" ]]; then
  echo "❌ Thiếu GH_TOKEN. Set GH_TOKEN=<pat> hoặc đăng nhập 'gh auth login'."
  exit 1
fi

echo "🔧 Cài đặt branch protection cho $OWNER/$REPO:$BRANCH..."

# API endpoint
API_URL="https://api.github.com/repos/$OWNER/$REPO/branches/$BRANCH/protection"

# Body JSON — rule:
# - require_pull_request: không có (admin/PAT có thể push trực tiếp)
# - allow_force_pushes: false
# - allow_deletions: false
# - require_linear_history: true (đẹp history, không merge commit)
# - required_status_checks:KHÔNG ép buộc (CI có thể fail nhưng admin vẫn merge được)
# - enforce_admins: false (admin vẫn bị rule约束 nhưng PAT holder thì có quyền)
# - restrictions: chỉ admin team (nếu có)
#
# Lưu ý: GitHub không cho phép "PAT holder push directly" qua rule.
# PAT được cấp cho admin user (mhieuhonda) — user này có quyền admin
# repo nên rule 'enforce_admins: false' cho phép admin bypass.
BODY=$(cat <<'EOF'
{
  "required_status_checks": null,
  "enforce_admins": false,
  "required_pull_request_reviews": {
    "required_approving_review_count": 0,
    "dismiss_stale_reviews": false,
    "require_code_owner_reviews": false
  },
  "restrictions": null,
  "required_linear_history": true,
  "allow_force_pushes": false,
  "allow_deletions": false,
  "block_creations": false,
  "required_conversation_resolution": false
}
EOF
)

# PUT request
RESPONSE=$(curl -sS -X PUT "$API_URL" \
  -H "Authorization: Bearer $GH_TOKEN" \
  -H "Accept: application/vnd.github+json" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  -d "$BODY" \
  -w "\nHTTP_STATUS:%{http_code}")

# Tách status code
HTTP_STATUS=$(echo "$RESPONSE" | grep -oE 'HTTP_STATUS:[0-9]+' | cut -d: -f2)
BODY=$(echo "$RESPONSE" | sed 's/HTTP_STATUS:[0-9]*$//')

if [[ "$HTTP_STATUS" == "200" ]]; then
  echo "✅ Branch protection đã cài đặt thành công cho '$BRANCH'."
  echo ""
  echo "Rule đã áp dụng:"
  echo "  • required_linear_history: true (không merge commit, history sạch)"
  echo "  • allow_force_pushes: false (chặn force push)"
  echo "  • allow_deletions: false (chặn xóa branch)"
  echo "  • enforce_admins: false (admin/PAT có quyền bypass)"
  echo "  • required_pull_request_reviews: 0 approval (PR không bắt buộc)"
  echo ""
  echo "💡 Effect:"
  echo "  - Admin user (mhieuhonda) và PAT holder vẫn push trực tiếp được."
  echo "  - Người khác push trực tiếp sẽ bị GitHub từ chối với lỗi 403."
  echo "  - Họ phải tạo branch mới, mở PR, và merge qua UI/API."
elif [[ "$HTTP_STATUS" == "403" ]]; then
  echo "❌ 403 Forbidden — PAT thiếu quyền admin. Cần PAT với scope 'repo' (full)."
  exit 1
elif [[ "$HTTP_STATUS" == "404" ]]; then
  echo "❌ 404 Not Found — repo '$OWNER/$REPO' không tồn tại hoặc PAT không có quyền truy cập."
  exit 1
else
  echo "❌ HTTP $HTTP_STATUS — lỗi không xác định."
  echo "Response: $BODY"
  exit 1
fi
