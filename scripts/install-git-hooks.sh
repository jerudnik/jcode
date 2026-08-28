#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [ -z "$repo_root" ]; then
  echo "install-git-hooks: not in a git repository" >&2
  exit 0
fi

managed_marker="# Managed by scripts/install-git-hooks.sh for jcode"

# Write a managed hook shim that resolves the checkout at run time.
#
# Linked worktrees share one .git/hooks directory, so a repository path
# expanded at install time is wrong for every other checkout, and becomes
# wrong for all of them once the worktree that ran the installer is removed.
# Resolving with git rev-parse on each invocation keeps every worktree
# pointed at its own scripts/git-hooks copy.
write_hook_shim() {
  local dest="$1" hook_name="$2"
  {
    printf '%s\n' '#!/usr/bin/env bash'
    printf '%s\n' "$managed_marker"
    cat <<'SHIM_HEAD'
set -euo pipefail
root="$(git rev-parse --show-toplevel)"
SHIM_HEAD
    # shellcheck disable=SC2016  # $root is deliberately literal: it expands when the hook runs
    printf 'hook="$root/scripts/git-hooks/%s"\n' "$hook_name"
    cat <<'SHIM_TAIL'
if [ ! -x "$hook" ]; then
  echo "${0##*/}: $hook is missing or not executable; refusing to skip the guard" >&2
  exit 1
fi
exec "$hook" "$@"
SHIM_TAIL
  } >"$dest"
  chmod +x "$dest"
}

hook_path="$(git rev-parse --git-path hooks/pre-push)"
hook_dir="$(dirname "$hook_path")"
mkdir -p "$hook_dir"

if [ -e "$hook_path" ] && ! grep -Fq "$managed_marker" "$hook_path"; then
  echo "install-git-hooks: existing pre-push hook left untouched: $hook_path" >&2
  echo "install-git-hooks: run scripts/git-hooks/pre-push from that hook to enable branch rails" >&2
  exit 0
fi

write_hook_shim "$hook_path" pre-push

echo "install-git-hooks: installed pre-push branch rail guard"

# Pre-commit: fast commit-time guards, including staged documentation checks.
# Installed as a managed shim like pre-push.
precommit_path="$(git rev-parse --git-path hooks/pre-commit)"
if [ -e "$precommit_path" ] && [ ! -L "$precommit_path" ] \
   && ! grep -Fq "$managed_marker" "$precommit_path"; then
  echo "install-git-hooks: existing pre-commit hook left untouched: $precommit_path" >&2
  echo "install-git-hooks: run scripts/git-hooks/pre-commit from that hook to enable commit-time guards" >&2
else
  rm -f "$precommit_path"
  write_hook_shim "$precommit_path" pre-commit
  echo "install-git-hooks: installed pre-commit guards"
fi
