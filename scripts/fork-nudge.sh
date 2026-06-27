#!/usr/bin/env bash
# Fast, non-blocking local-drift nudge for the fork rails.
#
# Problem it solves: GitHub CI rebases `distro/nix` and `main` onto upstream every
# six hours and force-pushes. Between those runs a local clone drifts silently,
# and nothing reminds you to reconcile. This is the reminder.
#
# Design (idiot-proof + compute-frugal):
#   - Never blocks shell entry on the network. It reads ALREADY-FETCHED
#     remote-tracking refs for an instant verdict.
#   - If those cached refs are older than FORK_NUDGE_MAX_AGE seconds, it kicks a
#     background `git fetch` so the NEXT entry is accurate.
#   - Auto fast-forwards `main` ONLY in the unambiguously safe case: strictly
#     behind, clean working tree, and zero local-only commits. Anything that
#     would need a rebase is a nudge, never a surprise rebase.
#
# Intended to be run from the devShell shellHook (direnv runs it on `cd`).
# Run `scripts/sync-local.sh` for the full, network-fetching reconcile.
#
# Env knobs:
#   FORK_REMOTE        (default: github)   remote holding the reconciled rails
#   MAIN_BRANCH        (default: main)
#   FORK_NUDGE_MAX_AGE (default: 10800)    secs before a background refetch fires
#   FORK_NUDGE_AUTOSYNC(default: 1)        1 = auto fast-forward the safe case
#   FORK_NUDGE_QUIET   (default: 0)        1 = print nothing when already in sync
set -euo pipefail

fork_remote="${FORK_REMOTE:-github}"
main_branch="${MAIN_BRANCH:-main}"
max_age="${FORK_NUDGE_MAX_AGE:-10800}"
autosync="${FORK_NUDGE_AUTOSYNC:-1}"
quiet="${FORK_NUDGE_QUIET:-0}"

git rev-parse --git-dir >/dev/null 2>&1 || exit 0
git_dir="$(git rev-parse --git-dir)"
fork_ref="$fork_remote/$main_branch"

# No remote-tracking ref yet (fresh clone, never fetched): refetch in background
# and stay silent. Nothing actionable to report this run.
if ! git show-ref --verify --quiet "refs/remotes/$fork_ref"; then
  ( git fetch --quiet --prune "$fork_remote" >/dev/null 2>&1 & ) 2>/dev/null || true
  exit 0
fi

# Opportunistic, non-blocking refresh when the cached refs are stale.
stamp="$git_dir/fork-nudge-last-fetch"
now="$(date +%s)"
last=0
[ -f "$stamp" ] && last="$(cat "$stamp" 2>/dev/null || echo 0)"
if [ $(( now - last )) -ge "$max_age" ]; then
  date +%s >"$stamp" 2>/dev/null || true
  ( git fetch --quiet --prune "$fork_remote" >/dev/null 2>&1 & ) 2>/dev/null || true
  refreshed_bg=1
fi

# Instant verdict from cached refs (no network).
ahead="$(git rev-list --count "$fork_ref..$main_branch" 2>/dev/null || echo 0)"
behind="$(git rev-list --count "$main_branch..$fork_ref" 2>/dev/null || echo 0)"
short_fork="$(git rev-parse --short "$fork_ref" 2>/dev/null || echo '?')"

tree_clean=1
git diff --quiet 2>/dev/null && git diff --cached --quiet 2>/dev/null || tree_clean=0
on_main=0
[ "$(git symbolic-ref --quiet --short HEAD 2>/dev/null || echo '')" = "$main_branch" ] && on_main=1

banner() { printf 'fork: %s\n' "$1"; }

if [ "$behind" -eq 0 ] && [ "$ahead" -eq 0 ]; then
  [ "$quiet" = "1" ] || banner "main in sync with $fork_ref ($short_fork)"
  exit 0
fi

# Safe auto fast-forward: strictly behind, clean tree, no local-only commits.
if [ "$behind" -gt 0 ] && [ "$ahead" -eq 0 ] \
   && [ "$autosync" = "1" ] && [ "$tree_clean" = "1" ] && [ "$on_main" = "1" ]; then
  if git merge --ff-only --quiet "$fork_ref" 2>/dev/null; then
    banner "auto-synced main to $fork_ref ($short_fork), was $behind behind"
    exit 0
  fi
fi

# Otherwise: nudge, never surprise-rebase.
if [ "$ahead" -gt 0 ] && [ "$behind" -gt 0 ]; then
  banner "main is $behind behind / $ahead ahead $fork_ref — run scripts/sync-local.sh to rebase"
elif [ "$behind" -gt 0 ]; then
  banner "main is $behind behind $fork_ref — run scripts/sync-local.sh (or git pull --ff-only)"
else
  banner "main is $ahead ahead $fork_ref (local-only commits, nothing to pull)"
fi
[ "${refreshed_bg:-0}" = "1" ] && banner "(remote state refreshing in background; rerun for an updated verdict)"
exit 0
