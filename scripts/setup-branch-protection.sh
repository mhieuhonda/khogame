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

# Repo info — hardcode theo khogame. Nếu fork repo này, sửa 2 dòng dưới.
OWNER="mhieuhonda"
REPO="khogame"
BRANCH="main"

# Token: ưu tiên GH_TOKEN, sau đó gh auth token.
# Cần PAT với scope `repo` (full) để PUT /branches/.../protection.
if [[ -z "${GH_TOKEN:-}" ]]; then
  if command -v gh >/dev/null 2>&1; then
    GH_TOKEN="$(gh auth token 2>/dev/null || true)"
  fi
fi
if [[ -z "${GH_TOKEN:-}" ]]; then
  echo "❌ Thiếu GH_TOKEN. Set GH_TOKEN=<pat> hoặc đăng nhập 'gh auth login'."
  exit 1
fi

# Validate token format trước khi gọi API —PAT classic có dạng `ghp_xxxx`
# (40 ký tự), fine-grained có dạng `github_pat_xxxx`. Token OAuth app
# (gho_) không có quyền protection API.
if [[ ! "$GH_TOKEN" =~ ^(ghp_|github_pat_|ghu_) ]]; then
  echo "⚠ GH_TOKEN không khớp định dạng PAT (ghp_/github_pat_/ghu_)."
  echo "  Tiếp tục anyway — nếu API trả 401, kiểm tra lại token."
fi

echo "🔧 Cài đặt branch protection cho $OWNER/$REPO:$BRANCH..."

# API endpoint
API_URL="https://api.github.com/repos/$OWNER/$REPO/branches/$BRANCH/protection"

# Body JSON — rule:
# - required_status_checks: null (KHÔNG ép buộc CI phải pass — admin
#   có thể merge ngay cả khi CI fail, vì admin review bằng mắt).
# - enforce_admins: false (admin/PAT có quyền bypass rule khi push).
# - required_pull_request_reviews: required_approving_review_count = 0
#   (PR không bắt buộc approval nhưng vẫn tạo để có audit trail).
# - restrictions: null (không lock theo user/team).
# - required_linear_history: true (history sạch, không merge commit).
# - allow_force_pushes: false (chặn force push → history không bị rewrite).
# - allow_deletions: false (chặn xóa branch main).
# - block_creations: false (cho phép tạo branch ref mới từ main).
# - required_conversation_resolution: false (PR có thể merge dù chưa
#   resolve mọi comment — admin quyền quyết).
#
# Lưu ý: GitHub không có rule "PAT holder push directly" explicit.
# PAT được cấp cho admin user (mhieuhonda) — user này có quyền admin
# repo nên rule `enforce_admins: false` cho phép admin bypass.
# Người khác (collaborator có quyền `push`) sẽ bị chặn push trực tiếp
# → phải tạo branch → mở PR.
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

# PUT request — lưu response + HTTP status.
# -s: silent (no progress bar). -S: show error on failure. -f: fail
#   on HTTP ≥400 (return exit 22). -i: include headers in output
#   (để parse status). Không dùng -f vì mình parse status bằng tay.
# `--max-time 30`: tránh treo nếu GitHub API không đáp ứng.
RESPONSE_FILE=$(mktemp -t branch_protection_resp.XXXX)
trap 'rm -f "$RESPONSE_FILE"' EXIT

HTTP_STATUS=$(curl -sS -o "$RESPONSE_FILE" -w "%{http_code}" \
  -X PUT "$API_URL" \
  -H "Authorization: Bearer $GH_TOKEN" \
  -H "Accept: application/vnd.github+json" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  -d "$BODY" \
  --max-time 30) || {
    echo "❌ curl fail (exit $?) — có thể lỗi mạng hoặc GitHub API không đáp ứng."
    echo "  Endpoint: $API_URL"
    exit 1
  }

# Đọc response body (có thể là JSON success hoặc JSON error).
BODY_OUT=$(cat "$RESPONSE_FILE")

case "$HTTP_STATUS" in
  200)
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
    ;;
  401)
    echo "❌ 401 Unauthorized — GH_TOKEN không hợp lệ hoặc đã hết hạn."
    echo "  Response: $BODY_OUT"
    exit 1
    ;;
  403)
    echo "❌ 403 Forbidden — PAT thiếu quyền admin."
    echo "  Cần PAT với scope 'repo' (full) hoặc fine-grained PAT với"
    echo "  permission 'Administration: Read and Write' cho repo $OWNER/$REPO."
    echo "  Response: $BODY_OUT"
    exit 1
    ;;
  404)
    echo "❌ 404 Not Found — repo '$OWNER/$REPO' không tồn tại hoặc PAT"
    echo "  không có quyền truy cập repo (private repo cần PAT có quyền)."
    echo "  Response: $BODY_OUT"
    exit 1
    ;;
  422)
    echo "❌ 422 Unprocessable Entity — rule không hợp lệ."
    echo "  Có thể Enterprise repo đã có rule tổ chức chặn override."
    echo "  Response: $BODY_OUT"
    exit 1
    ;;
  *)
    echo "❌ HTTP $HTTP_STATUS — lỗi không xác định."
    echo "  Response: $BODY_OUT"
    echo "  Endpoint: $API_URL"
    exit 1
    ;;
esac
