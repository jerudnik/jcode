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
# Usage:
#   rerere-rebase.sh <worktree-dir> <onto-ref>
#   rerere-rebase.sh <worktree-dir> <onto-ref> <old-base-ref>
#
# The optional old-base-ref form runs `git rebase --onto <onto-ref>
# <old-base-ref>`. Use it for stacked branches whose lower layer was rewritten
# separately. Without it, git may try to replay already-rebased lower-layer
# commits and either duplicate them or surface irrelevant conflicts.
set -euo pipefail

wt="${1:?worktree dir required}"
onto="${2:?onto ref required}"
old_base="${3:-}"

git -C "$wt" config rerere.enabled true
git -C "$wt" config rerere.autoupdate true

rebasing() {
  [ -d "$(git -C "$wt" rev-parse --git-path rebase-merge 2>/dev/null)" ] \
    || [ -d "$(git -C "$wt" rev-parse --git-path rebase-apply 2>/dev/null)" ]
}

rebase_state_dir() {
  local dir
  dir="$(git -C "$wt" rev-parse --git-path rebase-merge 2>/dev/null)"
  if [ -d "$dir" ]; then
    printf '%s\n' "$dir"
    return 0
  fi
  dir="$(git -C "$wt" rev-parse --git-path rebase-apply 2>/dev/null)"
  if [ -d "$dir" ]; then
    printf '%s\n' "$dir"
    return 0
  fi
  return 1
}

manual_commit_staged_replay() {
  local state_dir orig
  state_dir="$(rebase_state_dir)" || return 1
  if [ ! -f "$state_dir/done" ]; then
    return 1
  fi
  orig="$(tail -1 "$state_dir/done" | awk '{print $2}')"
  if [ -z "$orig" ]; then
    return 1
  fi
  echo "rerere-rebase: committing staged replay for $orig" >&2
  GIT_EDITOR=true git -C "$wt" commit -C "$orig"
}

rebase_args=("$onto")
if [ -n "$old_base" ]; then
  rebase_args=(--onto "$onto" "$old_base")
fi

# Kick off; if it applies cleanly there is nothing more to do.
if git -C "$wt" rebase "${rebase_args[@]}"; then
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
  # rerere already staged the replayed resolution; advance the rebase. Some
  # git versions can leave staged changes after advancing the todo without
  # creating the replayed commit, reporting: "you have staged changes in your
  # working tree". When that happens, create the replayed commit with the
  # original commit metadata, then continue the sequencer.
  out="$(mktemp)"
  if GIT_EDITOR=true git -C "$wt" rebase --continue >"$out" 2>&1; then
    cat "$out"
    rm -f "$out"
    continue
  fi
  cat "$out" >&2
  if grep -q 'you have staged changes in your working tree' "$out" \
    && ! git -C "$wt" diff --cached --quiet; then
    rm -f "$out"
    manual_commit_staged_replay || {
      echo "rerere-rebase: staged replay exists but original commit could not be inferred" >&2
      git -C "$wt" rebase --abort || true
      exit 1
    }
    continue
  fi
  rm -f "$out"
  git -C "$wt" rebase --abort || true
  exit 1
done
exit 0
