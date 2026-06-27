#!/usr/bin/env bash
# Pull the CI-reconciled branch stack down to this local clone.
#
# Upstream maintenance is automated on GitHub (see
# .github/workflows/fork-maintenance.yml -> nix.yml `sync-upstream`): every six
# hours CI fast-forwards `vendor/upstream` to `upstream/master`, rebases
# `distro/nix` onto it, then rebases `main` onto `distro/nix`, force-pushing all
# three. GitHub is therefore the authoritative surface; a local clone only goes
# stale. This script reconciles the local clone to match, without losing local
# work:
#
#   - vendor/upstream : reset to the fork mirror (a pure upstream import, never
#                       edited locally).
#   - distro/nix      : if the local branch has no commits the fork lacks, reset
#                       to the fork; otherwise rebase local work onto it.
#   - main            : same policy as distro/nix.
#
# A dirty working tree is auto-stashed and popped around the sync. Local-only
# commits are preserved by rebasing; a rebase conflict aborts cleanly and leaves
# the branch untouched so you can resolve by hand.
#
# Usage:
#   scripts/sync-local.sh            # sync all three rails
#   scripts/sync-local.sh --check    # report drift only, change nothing
#
# Remotes (override via env): FORK_REMOTE=github, UPSTREAM_REMOTE=upstream.
set -euo pipefail

fork_remote="${FORK_REMOTE:-github}"
upstream_remote="${UPSTREAM_REMOTE:-upstream}"
vendor_branch="${VENDOR_BRANCH:-vendor/upstream}"
distro_branch="${DISTRO_BRANCH:-distro/nix}"
main_branch="${MAIN_BRANCH:-main}"

check_only=false
[ "${1:-}" = "--check" ] && check_only=true

note() { printf '%s\n' "$*"; }
section() { printf '\n== %s ==\n' "$1"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

git rev-parse --git-dir >/dev/null 2>&1 || die "not inside a git repository"
git remote get-url "$fork_remote" >/dev/null 2>&1 || die "fork remote '$fork_remote' is missing"
git remote get-url "$upstream_remote" >/dev/null 2>&1 || die "upstream remote '$upstream_remote' is missing"

section "Fetching $fork_remote and $upstream_remote"
git fetch --prune --tags "$fork_remote"
git fetch --prune "$upstream_remote"

# Enable rerere and import shared recorded resolutions so recurring conflicts
# during the rebases below self-heal exactly like CI does.
if [ -x "$(git rev-parse --show-toplevel)/scripts/rerere-cache.sh" ]; then
  "$(git rev-parse --show-toplevel)/scripts/rerere-cache.sh" setup || true
fi

start_branch="$(git symbolic-ref --quiet --short HEAD || echo '')"
stash_ref=""
if ! git diff --quiet || ! git diff --cached --quiet; then
  if $check_only; then
    note "working tree is dirty (left untouched in --check mode)"
  else
    section "Stashing dirty working tree"
    git stash push --include-untracked -m "sync-local autostash $(date -u +%FT%TZ)" >/dev/null
    stash_ref="$(git rev-parse -q --verify stash@{0} || echo '')"
  fi
fi

restore() {
  [ -n "$start_branch" ] && git checkout --quiet "$start_branch" 2>/dev/null || true
  if [ -n "$stash_ref" ]; then
    section "Restoring stashed working tree"
    git stash pop --quiet || note "warning: 'git stash pop' had conflicts; resolve manually (stash kept)"
  fi
}
trap restore EXIT

drift=0

# sync_rail <branch> <fork-ref> <mode:mirror|rebase>
sync_rail() {
  local branch="$1" fork_ref="$2" mode="$3"
  section "$branch -> $fork_ref ($mode)"

  git show-ref --verify --quiet "refs/remotes/$fork_ref" || die "missing $fork_ref"

  if ! git show-ref --verify --quiet "refs/heads/$branch"; then
    note "creating local $branch from $fork_ref"
    $check_only && { drift=1; return; }
    git branch --quiet "$branch" "$fork_ref"
    return
  fi

  local local_sha fork_sha base
  local_sha="$(git rev-parse "$branch")"
  fork_sha="$(git rev-parse "$fork_ref")"
  if [ "$local_sha" = "$fork_sha" ]; then
    note "up to date ($(git rev-parse --short "$fork_sha"))"
    return
  fi

  local ahead behind
  ahead="$(git rev-list --count "$fork_ref..$branch")"
  behind="$(git rev-list --count "$branch..$fork_ref")"
  note "local is $ahead ahead, $behind behind $fork_ref"
  drift=1
  $check_only && { git log --oneline "$fork_ref..$branch" | sed 's/^/  local-only: /'; return; }

  if [ "$ahead" -eq 0 ]; then
    note "fast-forward/reset to $fork_ref"
    git checkout --quiet "$branch"
    git reset --hard --quiet "$fork_ref"
    return
  fi

  # Local has commits the fork lacks. A pure mirror branch must never; bail loud.
  if [ "$mode" = "mirror" ]; then
    die "$branch has $ahead local-only commit(s) but is a mirror of $fork_ref; \
inspect with: git log --oneline $fork_ref..$branch"
  fi

  # Rebase mode: preserve local work on top of the reconciled fork tip. Use the
  # shared rerere-aware helper so recurring conflicts auto-replay recorded
  # resolutions; a genuinely new conflict aborts cleanly for manual resolution.
  note "rebasing $ahead local commit(s) onto $fork_ref"
  git checkout --quiet "$branch"
  local helper
  helper="$(git rev-parse --show-toplevel)/scripts/rerere-rebase.sh"
  if [ -x "$helper" ]; then
    if ! "$helper" "$(git rev-parse --show-toplevel)" "$fork_ref"; then
      die "rebase of $branch onto $fork_ref hit a NEW conflict; resolve it, then run \
'scripts/rerere-cache.sh export' and commit .rerere-cache so it never recurs"
    fi
  elif ! git rebase "$fork_ref"; then
    git rebase --abort || true
    die "rebase of $branch onto $fork_ref hit conflicts; resolve manually"
  fi
}

sync_rail "$vendor_branch" "$fork_remote/$vendor_branch" mirror
sync_rail "$distro_branch" "$fork_remote/$distro_branch" rebase
sync_rail "$main_branch"   "$fork_remote/$main_branch"   rebase

# Cross-check: the local mirror must equal the fork mirror (already enforced by
# sync_rail). Separately, report whether the *fork* mirror has caught up to real
# upstream. Fork CI owns that catch-up on its 6h schedule, so lag here is
# informational, not local drift the user should act on.
section "Upstream mirror integrity"
if git show-ref --verify --quiet "refs/remotes/$upstream_remote/master"; then
  fork_vendor_sha="$(git rev-parse "$fork_remote/$vendor_branch" 2>/dev/null || echo '')"
  upstream_sha="$(git rev-parse "$upstream_remote/master")"
  if [ "$(git rev-parse "$vendor_branch")" != "${fork_vendor_sha:-x}" ]; then
    note "WARN: local $vendor_branch differs from $fork_remote/$vendor_branch (run without --check to reset)"
    drift=1
  elif [ "$fork_vendor_sha" = "$upstream_sha" ]; then
    note "OK: $vendor_branch == $fork_remote/$vendor_branch == $upstream_remote/master"
  else
    behind="$(git rev-list --count "$fork_remote/$vendor_branch..$upstream_remote/master" 2>/dev/null || echo '?')"
    note "INFO: $fork_remote/$vendor_branch is $behind commit(s) behind $upstream_remote/master;"
    note "      fork CI will mirror these on its next scheduled sync. Nothing to do locally."
  fi
fi

section "Result"
if $check_only; then
  if [ "$drift" -eq 0 ]; then note "all rails in sync"; else note "drift detected (run without --check to reconcile)"; fi
else
  note "local rails reconciled to $fork_remote"
  note "next: in the consumer flake run 'nix flake update jcode' to pin the new main"
fi
