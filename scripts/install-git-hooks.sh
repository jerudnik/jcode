#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [ -z "$repo_root" ]; then
  echo "install-git-hooks: not in a git repository" >&2
  exit 0
fi

managed_marker="# Managed by scripts/install-git-hooks.sh for jcode"
hook_path="$(git rev-parse --git-path hooks/pre-push)"
hook_dir="$(dirname "$hook_path")"
mkdir -p "$hook_dir"

if [ -e "$hook_path" ] && ! grep -Fq "$managed_marker" "$hook_path"; then
  echo "install-git-hooks: existing pre-push hook left untouched: $hook_path" >&2
  echo "install-git-hooks: run scripts/git-hooks/pre-push from that hook to enable branch rails" >&2
  exit 0
fi

cat >"$hook_path" <<EOF
#!/usr/bin/env bash
$managed_marker
exec "$repo_root/scripts/git-hooks/pre-push" "\$@"
EOF
chmod +x "$hook_path"

echo "install-git-hooks: installed pre-push branch rail guard"
