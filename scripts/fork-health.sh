#!/usr/bin/env bash
# Fork health check: verify the single-rail model invariants.
#
# This fork maintains exactly one branch on GitHub: `main`.
#
# It previously had three rails. `vendor/upstream` was a moving mirror of
# 1jehuang/jcode master, and `distro/nix` was a packaging layer sandwiched
# between that mirror and `main` so upstream could be rebased through the
# stack without packaging changes colliding with fork work every six hours.
# Both existed to serve upstream tracking. This is now a hard fork
# (docs/BRANCHING.md): the divergence point is a fixed historical commit
# recorded by the immutable `fork-point` tag, there is no mirror to sync, and
# no rebase for a packaging layer to survive. Both rails were retired, their
# payloads already fully contained in `main`.
#
# Checks:
#   1) fork-point tag exists and is an ancestor of main
#   2) GitHub carries the rail (+ topic branches, reported; stale ones flagged)
#   3) docs/BRANCHING.md's CI table names every workflow that exists
#   4) GitHub rulesets still describe the current rail set
#   5) no Windows CI has crept back in (issue #19)
#
# Runs identically locally and in CI (.github/workflows/fork-health.yml).
# Requires: git with the fork remote and tags fetched; gh (only for check 2,
# skipped with a warning when gh is unavailable or unauthenticated). It does
# not need the upstream remote at all, which is the point of a hard fork.
#
# Usage:
#   scripts/fork-health.sh [--repo jerudnik/jcode] [--fork-remote github]
set -euo pipefail

repo="jerudnik/jcode"
fork_remote="${FORK_REMOTE:-github}"
fork_point_ref="${FORK_POINT_REF:-fork-point}"
main_branch="main"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo) repo="$2"; shift ;;
    --fork-remote) fork_remote="$2"; shift ;;
    -h|--help)
      sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) printf 'error: unknown option: %s\n' "$1" >&2; exit 2 ;;
  esac
  shift
done

failures=0
fail() { printf 'FAIL: %s\n' "$*" >&2; failures=$((failures + 1)); }
ok()   { printf 'OK:   %s\n' "$*"; }
warn() { printf 'WARN: %s\n' "$*"; }

fork_main="$fork_remote/$main_branch"
git show-ref --verify --quiet "refs/remotes/$fork_main" \
  || { printf 'error: missing ref %s (fetch %s first)\n' "$fork_main" "$fork_remote" >&2; exit 2; }
if ! git rev-parse --verify --quiet "${fork_point_ref}^{commit}" >/dev/null; then
  printf 'error: missing %s tag (fetch tags: git fetch --tags %s)\n' \
    "$fork_point_ref" "$fork_remote" >&2
  exit 2
fi
fork_point="$(git rev-parse "${fork_point_ref}^{commit}")"

echo "=== Fork health: $repo ==="

# ── 1) Fork point anchoring ──────────────────────────────────────────────────
# The fork-touched clippy/rustfmt gates compute their file set as the diff
# between this commit and HEAD. If it stops being an ancestor of the rail, the
# gate silently starts measuring against an unrelated tree and goes quiet
# instead of going red. That failure mode is why this check exists.
if git merge-base --is-ancestor "$fork_point" "$fork_main"; then
  ok "$fork_point_ref (${fork_point:0:12}) is an ancestor of $main_branch"
else
  fail "$fork_point_ref (${fork_point:0:12}) is NOT an ancestor of $main_branch; the fork-touched gates are measuring against the wrong base"
fi

# ── 2) Branch set ────────────────────────────────────────────────────────────
# The invariant is that the rail exists, not that nothing else does. Requiring
# an exact match made this fail for the entire lifetime of any topic branch,
# which is the normal way work reaches main, so it was permanently red and had
# stopped carrying information. A missing rail is a real breakage and fails.
# Topic branches are reported; ones already contained in main are called out as
# residue worth deleting rather than treated as violations. Containment is
# ancestry, not tree equality: a merged topic branch keeps diverging in content
# as main moves on, so comparing trees would stop recognising it as residue the
# moment the next commit landed.
if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
  actual="$(gh api "repos/$repo/branches" --paginate --jq '.[].name' \
    | grep -v '^automation/' | sort)"
  topics="$(printf '%s\n' "$actual" | grep -vx "$main_branch" || true)"

  if ! printf '%s\n' "$actual" | grep -qx "$main_branch"; then
    fail "missing the maintained rail '$main_branch' on $repo"
  elif [ -z "$topics" ]; then
    ok "branch set is exactly {$main_branch}"
  else
    ok "rail present ($(printf '%s\n' "$topics" | wc -l | tr -d ' ') topic branch(es) alongside)"
    while IFS= read -r topic; do
      [ -n "$topic" ] || continue
      if git fetch -q "$fork_remote" "$topic" 2>/dev/null \
        && git merge-base --is-ancestor FETCH_HEAD "$fork_main" 2>/dev/null; then
        warn "topic branch '$topic' is already contained in $main_branch; safe to delete"
      else
        printf 'INFO: topic branch: %s\n' "$topic"
      fi
    done <<< "$topics"
  fi
else
  warn "gh unavailable or unauthenticated; skipping remote branch-set check"
fi

# ── 3) CI table currency ─────────────────────────────────────────────────────
# docs/BRANCHING.md documents what each workflow is for. A table that silently
# omits a workflow is worse than no table: it reads as complete. This was not
# hypothetical, ios-testflight.yml went undocumented until this check was
# written. Only presence is checked; the prose is a human's job.
undocumented=""
for wf in .github/workflows/*.yml; do
  name="${wf##*/}"
  grep -qF "\`$name\`" docs/BRANCHING.md || undocumented+="$name"$'\n'
done
if [ -z "$undocumented" ]; then
  ok "docs/BRANCHING.md documents every workflow"
else
  fail "workflows missing from the docs/BRANCHING.md CI table:"
  printf '%s' "$undocumented" | sed 's/^/      /' >&2
fi

# ── 4) Ruleset currency ──────────────────────────────────────────────────────
# Branch protection lives in GitHub rulesets, which are repository config: no
# file in a clone reveals them, so they drift silently. They named the retired
# rails after those rails were deleted, and the stale entries were what blocked
# the deletion. Assert they still describe the rail set. Reuses check 2's gh
# availability. Docs: docs/BRANCHING.md "Server-side rulesets".
if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
  ruleset_refs="$(gh api "repos/$repo/rulesets" --jq '.[].id' 2>/dev/null \
    | while read -r id; do
        gh api "repos/$repo/rulesets/$id" \
          --jq '.conditions.ref_name | (.include[], .exclude[])' 2>/dev/null
      done | grep '^refs/heads/' | sed 's|^refs/heads/||' | sort -u || true)"
  stale_rules="$(printf '%s\n' "$ruleset_refs" \
    | grep -vx -e "$main_branch" -e 'automation/\*\*' || true)"
  if [ -z "$ruleset_refs" ]; then
    warn "no branch rulesets found on $repo; main is unprotected"
  elif [ -z "$stale_rules" ]; then
    ok "GitHub rulesets reference only the current rail"
  else
    fail "GitHub rulesets reference branches that are not rails:"
    printf '%s\n' "$stale_rules" | sed 's/^/      /' >&2
  fi
fi

# ── 5) Windows CI stays out ──────────────────────────────────────────────────
# Windows CI was removed in issue #19: nothing consumes the artifacts, this
# fork publishes no releases, and no maintainer runs Windows, so the jobs were
# pure noise. Harvesting an upstream workflow change is the obvious way for
# them to return, and they would return *silently*, as green jobs nobody asked
# for. Windows remains a runtime target: cfg(windows) code, docs/WINDOWS.md,
# and scripts/install.ps1 are deliberately still here, so this checks CI
# configuration only.
windows_ci="$(grep -rlEi 'windows-latest|windows-11|pc-windows-msvc|cargo xwin|shell: (pwsh|powershell)' \
  .github/workflows/ 2>/dev/null || true)"
if [ -z "$windows_ci" ]; then
  ok "no Windows CI jobs (issue #19)"
else
  fail "Windows CI has returned to these workflows (see issue #19):"
  printf '%s\n' "$windows_ci" | sed 's/^/      /' >&2
fi

# ── Payload report (informational) ───────────────────────────────────────────
printf 'INFO: %s payload: %s commit(s) over %s\n' \
  "$main_branch" "$(git rev-list --count "$fork_point..$fork_main")" "$fork_point_ref"

echo
if [ "$failures" -eq 0 ]; then
  echo "=== Fork health: all invariants hold ==="
else
  echo "=== Fork health: $failures invariant violation(s) ===" >&2
  exit 1
fi
