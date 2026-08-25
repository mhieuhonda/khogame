# Branch Protection — Hướng dẫn

Tài liệu này mô tả rule branch protection đã áp dụng cho repo `mhieuhonda/khogame`.

## 📋 Rule hiện tại (main)

Áp dụng qua GitHub API (script `scripts/setup-branch-protection.sh`):

| Rule | Giá trị | Ý nghĩa |
|------|---------|--------|
| `required_linear_history` | `true` | Không cho phép merge commit — history phải linear (rebase hoặc squash) |
| `allow_force_pushes` | `false` | Chặn `git push --force` lên main |
| `allow_deletions` | `false` | Chặn xóa branch main |
| `enforce_admins` | `false` | Admin có quyền bypass rule (PAT holder cũng vậy) |
| `required_pull_request_reviews` | `0 approval` | PR không bắt buộc approval (cho phép self-merge) |
| `required_status_checks` | `null` | Không ép buộc CI pass trước merge |
| `restrictions` | `null` | Không giới hạn ai được push (rule enforce_admins=false lo) |

## 🔑 Effect

- **Admin user** (`mhieuhonda`): push trực tiếp OK (bypass rule).
- **PAT holder** với full quyền: push trực tiếp OK.
- **Người dùng khác** (collaborator): push trực tiếp bị 403 → phải tạo branch mới → mở PR → merge qua UI/API.

## 🛠️ Cài đặt lại

```bash
# Cách 1: dùng script
GH_TOKEN=<your_pat> ./scripts/setup-branch-protection.sh

# Cách 2: dùng GitHub CLI (đã login)
./scripts/setup-branch-protection.sh
```

Script sẽ PUT tới `https://api.github.com/repos/mhieuhonda/khogame/branches/main/protection` với body JSON rule.

## ✅ Verify

Kiểm tra rule đã áp dụng:

```bash
curl -sS -X GET "https://api.github.com/repos/mhieuhonda/khogame/branches/main/protection" \
  -H "Authorization: Bearer $GH_TOKEN" \
  -H "Accept: application/vnd.github+json" | jq .
```

## 🔄 Bypass rule (khi cần)

Nếu cần push trực tiếp mà không phải admin:

1. Tạo branch mới: `git checkout -b feature/xxx`
2. Push branch: `git push -u origin feature/xxx`
3. Mở PR trên GitHub UI
4. Merge PR (có thể self-merge)

Hoặc nếu bạn là admin và muốn tạm tắt rule:

```bash
# Tắt rule
curl -X DELETE "https://api.github.com/repos/mhieuhonda/khogame/branches/main/protection" \
  -H "Authorization: Bearer $GH_TOKEN"

# Bật lại
GH_TOKEN=$GH_TOKEN ./scripts/setup-branch-protection.sh
```

## 📚 Tham khảo

- [GitHub API: Branch Protection](https://docs.github.com/en/rest/branches/branch-protection)
- [About protected branches](https://docs.github.com/en/github/administering-a-repository/defining-the-mergeability-of-pull-requests/about-protected-branches)
