#!/usr/bin/env bash
# Fork rail health check: verify the two-branch model invariants.
#
# This fork maintains exactly two branches on GitHub:
#   distro/nix      : fork-point + Nix packaging + fork CI policy
#   main            : distro/nix + fork work (default branch)
#
# The former third rail, vendor/upstream, was a moving mirror of
# 1jehuang/jcode master, maintained so the rails could be rebased onto a live
# upstream. This is now a hard fork (docs/BRANCHING.md), so the divergence
# point is a fixed historical commit recorded by the immutable `fork-point`
# tag, and there is no mirror to keep in sync.
#
# Checks:
#   1) GitHub branch set is exactly the two rails (+ transient automation/*)
#   2) fork-point tag exists and is an ancestor of both rails
#   3) Ancestry: distro/nix ⊆ main
#   4) Scope: fork-point..distro touches only allowed packaging/CI-policy paths
#   5) Workflow ownership: main adds no .github/workflows changes over distro/nix
#
# Runs identically locally and in CI (.github/workflows/fork-health.yml).
# Requires: git with the fork remote and tags fetched; gh (only for check 1,
# skipped with a warning when gh is unavailable or unauthenticated). It no
# longer needs the upstream remote at all, which is the point of a hard fork.
#
# Usage:
#   scripts/fork-health.sh [--repo jerudnik/jcode] [--fork-remote github]
set -euo pipefail

repo="jerudnik/jcode"
fork_remote="${FORK_REMOTE:-github}"
fork_point_ref="${FORK_POINT_REF:-fork-point}"
distro_branch="distro/nix"
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

need_ref() {
  git show-ref --verify --quiet "refs/remotes/$1" \
    || { printf 'error: missing ref %s (fetch %s first)\n' "$1" "${1%%/*}" >&2; exit 2; }
}

fork_distro="$fork_remote/$distro_branch"
fork_main="$fork_remote/$main_branch"
need_ref "$fork_distro"; need_ref "$fork_main"
if ! git rev-parse --verify --quiet "${fork_point_ref}^{commit}" >/dev/null; then
  printf 'error: missing %s tag (fetch tags: git fetch --tags %s)\n' \
    "$fork_point_ref" "$fork_remote" >&2
  exit 2
fi
fork_point="$(git rev-parse "${fork_point_ref}^{commit}")"

echo "=== Fork health: $repo ==="

# ── 1) Branch set ────────────────────────────────────────────────────────────
# The invariant is that the three rails exist, not that nothing else does.
# Requiring an exact match made this check fail for the entire lifetime of any
# topic branch, which is the normal way work reaches main, so it was
# permanently red and had stopped carrying information. Missing rails are a real
# breakage and still fail. Topic branches are reported for visibility, and stale
# ones are called out, since a topic branch already merged into main is residue
# worth deleting rather than a violation.
if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
  rails=$'distro/nix\nmain'
  actual="$(gh api "repos/$repo/branches" --paginate --jq '.[].name' \
    | grep -v '^automation/' | sort)"
  missing="$(comm -23 <(printf '%s\n' "$rails") <(printf '%s\n' "$actual"))"
  topics="$(comm -13 <(printf '%s\n' "$rails") <(printf '%s\n' "$actual"))"

  if [ -n "$missing" ]; then
    fail "missing maintained rail(s) on $repo:"
    printf '%s\n' "$missing" | sed 's/^/      /' >&2
  elif [ -z "$topics" ]; then
    ok "branch set is exactly {main, distro/nix}"
  else
    ok "both rails present ($(printf '%s\n' "$topics" | wc -l | tr -d ' ') topic branch(es) alongside)"
    while IFS= read -r topic; do
      [ -n "$topic" ] || continue
      if git fetch -q "$fork_remote" "$topic" 2>/dev/null \
        && [ -z "$(git diff --stat "$fork_main" FETCH_HEAD 2>/dev/null)" ]; then
        warn "topic branch '$topic' has no content beyond $main_branch; safe to delete"
      else
        printf 'INFO: topic branch: %s\n' "$topic"
      fi
    done <<< "$topics"
  fi
else
  warn "gh unavailable or unauthenticated; skipping remote branch-set check"
fi

# ── 2) Fork point anchoring ──────────────────────────────────────────────────
# The fork-touched clippy/rustfmt gates compute their file set as the diff
# between this commit and HEAD. If it stops being an ancestor of the rails, the
# gate silently starts measuring against an unrelated tree.
if git merge-base --is-ancestor "$fork_point" "$fork_distro" \
  && git merge-base --is-ancestor "$fork_point" "$fork_main"; then
  ok "$fork_point_ref (${fork_point:0:12}) is an ancestor of both rails"
else
  fail "$fork_point_ref (${fork_point:0:12}) is NOT an ancestor of both rails; the fork-touched gates are measuring against the wrong base"
fi

# ── 3) Ancestry ──────────────────────────────────────────────────────────────
if git merge-base --is-ancestor "$fork_distro" "$fork_main"; then
  ok "$distro_branch is an ancestor of $main_branch"
else
  fail "$distro_branch is NOT an ancestor of $main_branch (rebase drift)"
fi

# ── 4) distro/nix scope ──────────────────────────────────────────────────────
# The packaging layer touches only distribution and fork-CI-policy paths.
# Keep in lockstep with docs/BRANCHING.md "Expected distro/nix touched areas".
#
# This describes distro/nix's PAYLOAD (fork-point..distro/nix), not the current
# tree. A path stays listed once distro/nix has ever touched it, even if a later
# main commit deletes the file: branch-model-status.sh is such a case, retired
# on main by the hard fork but still present in the rail's history.
allowed_scope_regex='^(flake\.(nix|lock)|nix/|docs/(NIX|BRANCHING)\.md|docs/AMBIENT_MODE\.md|docs/fork/SECURITY_TRIAGE\.md|README\.md|\.cargo/audit\.toml|\.github/workflows/|scripts/(branch-model-status|fork-health|update_packages)\.sh)'
out_of_scope="$(git diff --name-only "$fork_point" "$fork_distro" \
  | grep -Ev "$allowed_scope_regex" || true)"
if [ -z "$out_of_scope" ]; then
  ok "$distro_branch payload is within the packaging/CI-policy scope"
else
  fail "$distro_branch touches paths outside its scope:"
  printf '%s\n' "$out_of_scope" | sed 's/^/      /' >&2
fi

# ── 5) Workflow ownership ────────────────────────────────────────────────────
# CI policy is owned by distro/nix; main adding workflow diffs recreates the
# per-sync conflict problem this model exists to solve.
main_workflow_diff="$(git diff --name-only "$fork_distro" "$fork_main" -- .github/workflows/ || true)"
if [ -z "$main_workflow_diff" ]; then
  ok "$main_branch carries no .github/workflows changes over $distro_branch"
else
  fail "$main_branch modifies workflows (move these to $distro_branch):"
  printf '%s\n' "$main_workflow_diff" | sed 's/^/      /' >&2
fi

# ── Payload report (informational) ───────────────────────────────────────────
printf 'INFO: %s payload: %s commit(s) over %s\n' \
  "$distro_branch" "$(git rev-list --count "$fork_point..$fork_distro")" "$fork_point_ref"
printf 'INFO: %s payload: %s commit(s) over %s\n' \
  "$main_branch" "$(git rev-list --count "$fork_distro..$fork_main")" "$distro_branch"

echo
if [ "$failures" -eq 0 ]; then
  echo "=== Fork health: all invariants hold ==="
else
  echo "=== Fork health: $failures invariant violation(s) ===" >&2
  exit 1
fi
