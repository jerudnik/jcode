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

# Pre-commit: surface-contract guard (PM/tracking must land in notes, not
# repo docs/). Installed as a managed shim like pre-push.
precommit_path="$(git rev-parse --git-path hooks/pre-commit)"
if [ -e "$precommit_path" ] && [ ! -L "$precommit_path" ] \
   && ! grep -Fq "$managed_marker" "$precommit_path"; then
  echo "install-git-hooks: existing pre-commit hook left untouched: $precommit_path" >&2
  echo "install-git-hooks: run scripts/git-hooks/pre-commit from that hook to enable the surface guard" >&2
else
  rm -f "$precommit_path"
  cat >"$precommit_path" <<EOF
#!/usr/bin/env bash
$managed_marker
exec "$repo_root/scripts/git-hooks/pre-commit" "\$@"
EOF
  chmod +x "$precommit_path"
  echo "install-git-hooks: installed pre-commit surface-contract guard"
fi
