#!/usr/bin/env bash
# Fork rail health check: verify the three-branch model invariants.
#
# This fork maintains exactly three branches on GitHub:
#   vendor/upstream : byte-identical mirror of 1jehuang/jcode master
#   distro/nix      : vendor/upstream + Nix packaging + fork CI policy
#   main            : distro/nix + custom fork work (default branch)
#
# Checks:
#   1) GitHub branch set is exactly the three rails (+ transient automation/*)
#   2) vendor/upstream == upstream/master (or strictly behind; sync owns catch-up)
#   3) Ancestry: vendor/upstream ⊆ distro/nix ⊆ main
#   4) Scope: vendor..distro touches only allowed packaging/CI-policy paths
#   5) Workflow ownership: main adds no .github/workflows changes over distro/nix
#
# Runs identically locally and in CI (.github/workflows/fork-health.yml).
# Requires: git with the fork + upstream remotes fetched; gh (only for check 1,
# skipped with a warning when gh is unavailable or unauthenticated).
#
# Usage:
#   scripts/fork-health.sh [--repo jerudnik/jcode]
#                          [--fork-remote github] [--upstream-remote upstream]
set -euo pipefail

repo="jerudnik/jcode"
fork_remote="${FORK_REMOTE:-github}"
upstream_remote="${UPSTREAM_REMOTE:-upstream}"
vendor_branch="vendor/upstream"
distro_branch="distro/nix"
main_branch="main"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo) repo="$2"; shift ;;
    --fork-remote) fork_remote="$2"; shift ;;
    --upstream-remote) upstream_remote="$2"; shift ;;
    -h|--help)
      sed -n '2,22p' "$0" | sed 's/^# \{0,1\}//'
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

fork_vendor="$fork_remote/$vendor_branch"
fork_distro="$fork_remote/$distro_branch"
fork_main="$fork_remote/$main_branch"
upstream_ref="$upstream_remote/master"
need_ref "$fork_vendor"; need_ref "$fork_distro"; need_ref "$fork_main"; need_ref "$upstream_ref"

echo "=== Fork health: $repo ==="

# ── 1) Branch set ────────────────────────────────────────────────────────────
if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
  expected=$'distro/nix\nmain\nvendor/upstream'
  actual="$(gh api "repos/$repo/branches" --paginate --jq '.[].name' \
    | grep -v '^automation/' | sort)"
  if [ "$actual" = "$expected" ]; then
    ok "branch set is exactly {main, distro/nix, vendor/upstream}"
  else
    fail "unexpected branch set on $repo:"
    diff <(printf '%s\n' "$expected") <(printf '%s\n' "$actual") | sed 's/^/      /' >&2 || true
  fi
else
  warn "gh unavailable or unauthenticated; skipping remote branch-set check"
fi

# ── 2) Mirror equality ───────────────────────────────────────────────────────
vendor_sha="$(git rev-parse "$fork_vendor")"
upstream_sha="$(git rev-parse "$upstream_ref")"
if [ "$vendor_sha" = "$upstream_sha" ]; then
  ok "$vendor_branch matches upstream/master (${vendor_sha:0:12})"
elif git merge-base --is-ancestor "$vendor_sha" "$upstream_sha"; then
  behind="$(git rev-list --count "$fork_vendor..$upstream_ref")"
  warn "$vendor_branch is $behind commit(s) behind upstream/master (sync owns catch-up)"
else
  fail "$vendor_branch has diverged from upstream/master (fork-only commits on a mirror)"
  git log --oneline "$upstream_ref..$fork_vendor" | head -20 | sed 's/^/      /' >&2 || true
fi

# ── 3) Ancestry ──────────────────────────────────────────────────────────────
if git merge-base --is-ancestor "$fork_vendor" "$fork_distro"; then
  ok "$vendor_branch is an ancestor of $distro_branch"
else
  fail "$vendor_branch is NOT an ancestor of $distro_branch (rebase drift)"
fi
if git merge-base --is-ancestor "$fork_distro" "$fork_main"; then
  ok "$distro_branch is an ancestor of $main_branch"
else
  fail "$distro_branch is NOT an ancestor of $main_branch (rebase drift)"
fi

# ── 4) distro/nix scope ──────────────────────────────────────────────────────
# The packaging layer touches only distribution and fork-CI-policy paths.
# Keep in lockstep with docs/BRANCHING.md "Expected distro/nix touched areas".
allowed_scope_regex='^(flake\.(nix|lock)|nix/|docs/(NIX|BRANCHING)\.md|docs/AMBIENT_MODE\.md|docs/fork/SECURITY_TRIAGE\.md|README\.md|\.cargo/audit\.toml|\.github/workflows/|scripts/(branch-model-status|fork-health|update_packages)\.sh)'
out_of_scope="$(git diff --name-only "$fork_vendor" "$fork_distro" \
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
  "$distro_branch" "$(git rev-list --count "$fork_vendor..$fork_distro")" "$vendor_branch"
printf 'INFO: %s payload: %s commit(s) over %s\n' \
  "$main_branch" "$(git rev-list --count "$fork_distro..$fork_main")" "$distro_branch"

echo
if [ "$failures" -eq 0 ]; then
  echo "=== Fork health: all invariants hold ==="
else
  echo "=== Fork health: $failures invariant violation(s) ===" >&2
  exit 1
fi
