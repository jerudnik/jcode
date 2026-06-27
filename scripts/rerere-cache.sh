#!/usr/bin/env bash
# Shared transport for `git rerere` (reuse recorded resolution) entries.
#
# Why this exists: CI rebases `distro/nix` and `main` onto a fast-moving upstream
# every six hours. Whenever upstream touches one of the few files this fork
# rewrites, the SAME downstream-vs-upstream conflict recurs. `git rerere` can
# replay your one-time resolution automatically -- but only on a machine that
# holds the recording. rerere stores recordings in `$GIT_DIR/rr-cache`, which is
# per-clone and never pushed, so a fresh CI checkout starts empty and re-fails
# every cycle. (Verified: an empty rr-cache gives rerere nothing to replay.)
#
# This makes a tracked `.rerere-cache/` directory the shared source of truth:
#   setup  : enable rerere config for this clone, then import (run on shell entry)
#   import : tracked .rerere-cache -> live rr-cache   (seed a clone or CI)
#   export : live rr-cache -> tracked .rerere-cache   (after you resolve a new one)
#   status : show entry counts and drift
#
# `.rerere-cache/` is outside the Nix `src` fileset and the CI build-path filter,
# so committing resolutions never triggers a rebuild. The live rr-cache lives in
# the git common dir, so it is shared across `git worktree`s (which is how the CI
# sync job rebases).
set -euo pipefail

cmd="${1:-status}"

git rev-parse --git-dir >/dev/null 2>&1 || { echo "rerere-cache: not a git repo" >&2; exit 0; }
repo_root="$(git rev-parse --show-toplevel)"
tracked="$repo_root/.rerere-cache"
live="$(git rev-parse --git-common-dir)/rr-cache"

# Copy only *resolved* entries (those that have a postimage); skip in-progress
# preimage-only recordings and any non-entry files like the README.
copy_resolved() {
  local src="$1" dst="$2" n=0 entry hash
  [ -d "$src" ] || { echo 0; return 0; }
  mkdir -p "$dst"
  for entry in "$src"/*/; do
    [ -d "$entry" ] || continue
    [ -f "${entry}postimage" ] || continue
    hash="$(basename "$entry")"
    mkdir -p "$dst/$hash"
    cp -f "${entry}preimage"  "$dst/$hash/preimage"  2>/dev/null || true
    cp -f "${entry}postimage" "$dst/$hash/postimage" 2>/dev/null || true
    n=$((n + 1))
  done
  echo "$n"
}

count_resolved() {
  local dir="$1" n=0 entry
  [ -d "$dir" ] || { echo 0; return 0; }
  for entry in "$dir"/*/; do
    [ -d "$entry" ] && [ -f "${entry}postimage" ] && n=$((n + 1))
  done
  echo "$n"
}

case "$cmd" in
  setup)
    git config rerere.enabled true
    git config rerere.autoupdate true
    n="$(copy_resolved "$tracked" "$live")"
    [ "$n" -gt 0 ] && echo "rerere-cache: imported $n recorded resolution(s)"
    exit 0
    ;;
  import)
    n="$(copy_resolved "$tracked" "$live")"
    echo "rerere-cache: imported $n resolution(s) -> $live"
    ;;
  export)
    before="$(count_resolved "$tracked")"
    n="$(copy_resolved "$live" "$tracked")"
    after="$(count_resolved "$tracked")"
    echo "rerere-cache: exported $n resolution(s) -> .rerere-cache ($before -> $after tracked)"
    if [ "$n" -gt 0 ] \
       && { ! git -C "$repo_root" diff --quiet -- .rerere-cache 2>/dev/null \
            || [ -n "$(git -C "$repo_root" ls-files --others --exclude-standard -- .rerere-cache)" ]; }; then
      echo "rerere-cache: new/changed resolutions -- commit them:"
      echo "  git add .rerere-cache && git commit -m 'fork: record rerere resolution'"
    fi
    ;;
  status)
    echo "rerere-cache: tracked=$(count_resolved "$tracked") live=$(count_resolved "$live")"
    git config --get rerere.enabled >/dev/null 2>&1 \
      && echo "rerere: enabled=$(git config --get rerere.enabled) autoupdate=$(git config --get rerere.autoupdate 2>/dev/null || echo unset)" \
      || echo "rerere: DISABLED (run: scripts/rerere-cache.sh setup)"
    ;;
  *)
    echo "usage: rerere-cache.sh {setup|import|export|status}" >&2
    exit 2
    ;;
esac
