#!/usr/bin/env bash
# install_git_hooks.sh
#
# Idempotently install the warn-only Backlog.md tracking-divergence git
# hooks from scripts/git-hooks/ into .git/hooks/.
#
# - Skips the README.
# - Prefers symlinks; falls back to copies if symlinking is unsupported.
# - Backs up any existing hook to `<hook>.backup-YYYYMMDD-HHMMSS` before
#   overwriting (unless that hook is already a symlink to our source).
# - Marks installed hooks executable.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [ -z "$REPO_ROOT" ]; then
  printf 'error: not inside a git repository\n' >&2
  exit 1
fi
cd "$REPO_ROOT"

SRC_DIR="scripts/git-hooks"
HOOKS_DIR="$(git rev-parse --git-path hooks)"

if [ ! -d "$SRC_DIR" ]; then
  printf 'error: %s not found\n' "$SRC_DIR" >&2
  exit 1
fi
mkdir -p "$HOOKS_DIR"

ts="$(date +%Y%m%d-%H%M%S)"
installed=0
skipped=0

for src in "$SRC_DIR"/*; do
  name="$(basename "$src")"
  case "$name" in
    README.md|*.md) continue ;;
    *.backup-*)     continue ;;
  esac
  [ -f "$src" ] || continue

  # Ensure the source is executable for the user.
  chmod +x "$src" 2>/dev/null || true

  dest="$HOOKS_DIR/$name"
  abs_src="$REPO_ROOT/$src"

  # If destination is already a symlink to the source, nothing to do.
  if [ -L "$dest" ]; then
    cur_target="$(readlink "$dest")"
    case "$cur_target" in
      "$abs_src"|"../../$src"|"$src")
        printf 'ok      %s (already linked)\n' "$dest"
        skipped=$((skipped + 1))
        continue
        ;;
    esac
  fi

  # Back up any pre-existing hook that is not ours.
  if [ -e "$dest" ] || [ -L "$dest" ]; then
    backup="$dest.backup-$ts"
    mv "$dest" "$backup"
    printf 'backup  %s -> %s\n' "$dest" "$backup"
  fi

  # Try symlink first; fall back to copy.
  if ln -s "$abs_src" "$dest" 2>/dev/null; then
    printf 'link    %s -> %s\n' "$dest" "$abs_src"
  else
    cp "$abs_src" "$dest"
    chmod +x "$dest"
    printf 'copy    %s\n' "$dest"
  fi
  installed=$((installed + 1))
done

printf '\nDone. installed=%d skipped=%d\n' "$installed" "$skipped"
printf 'Hooks are warn-only. Set EXIT_ON_FINDING=1 to make them blocking.\n'
