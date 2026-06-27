#!/usr/bin/env bash
# Rebase a worktree with rerere auto-resolution, failing loud on NEW conflicts.
#
# Used by the CI sync-upstream job (.github/workflows/nix.yml). With rerere
# enabled and the shared `.rerere-cache` imported, a recurring conflict is
# auto-resolved and auto-staged, but `git rebase` still PAUSES needing
# `--continue` (verified behaviour). This drives that loop:
#
#   - while a rebase is in progress: if any path is still unmerged, the conflict
#     is genuinely NEW (no recorded resolution) -> abort and exit non-zero so a
#     human resolves it once locally, exports the recording, and commits it.
#   - otherwise rerere already staged the replayed resolution -> `--continue`.
#
# A bounded loop guards against pathological non-termination.
#
# Usage: rerere-rebase.sh <worktree-dir> <onto-ref>
set -euo pipefail

wt="${1:?worktree dir required}"
onto="${2:?onto ref required}"

git -C "$wt" config rerere.enabled true
git -C "$wt" config rerere.autoupdate true

rebasing() {
  [ -d "$(git -C "$wt" rev-parse --git-path rebase-merge 2>/dev/null)" ] \
    || [ -d "$(git -C "$wt" rev-parse --git-path rebase-apply 2>/dev/null)" ]
}

# Kick off; if it applies cleanly there is nothing more to do.
if git -C "$wt" rebase "$onto"; then
  exit 0
fi

i=0
while rebasing; do
  i=$((i + 1))
  if [ "$i" -gt 200 ]; then
    echo "rerere-rebase: exceeded 200 steps onto $onto; aborting" >&2
    git -C "$wt" rebase --abort || true
    exit 1
  fi
  unmerged="$(git -C "$wt" diff --name-only --diff-filter=U)"
  if [ -n "$unmerged" ]; then
    echo "rerere-rebase: NEW conflict (no recorded resolution) onto $onto:" >&2
    printf '%s\n' "$unmerged" | sed 's/^/  /' >&2
    echo "Resolve locally once, then: scripts/rerere-cache.sh export && commit .rerere-cache" >&2
    git -C "$wt" rebase --abort || true
    exit 1
  fi
  # rerere already staged the replayed resolution; advance the rebase.
  GIT_EDITOR=true git -C "$wt" rebase --continue || true
done
exit 0
