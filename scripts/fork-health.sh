#!/usr/bin/env bash
# Fork health check: verify the single-rail model and governance invariants.
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
# Local checks (no network, always run):
#   1) fork-point tag exists and is an ancestor of main
#   2) docs/BRANCHING.md's CI table names every workflow that exists
#   3) no Windows CI has crept back in (issue #19)
#
# Governance comparison (exactly one source is mandatory):
#   --fixture PATH   compare an on-disk aggregate snapshot; no GitHub access
#   --live           acquire the aggregate snapshot from GitHub and compare it
#
# The governance comparison is the fork's drift detector for branch protection,
# which is repository configuration no clone reveals. R07 design.md section 6
# forbids the previous behaviour of warning and skipping when `gh` was missing
# or unauthenticated: a governance check that degrades to a warning reports
# green for an unobserved state, which is worse than reporting nothing. So a
# source is required, and an acquisition failure is exit 2, never a pass.
#
# Exit codes:
#   0  every invariant holds and the governance snapshot matches the manifest
#   1  one or more invariant or governance violations
#   2  usage error, or the governance source could not be acquired/parsed
#
# Requires: git with the fork remote and tags fetched; python3. Live mode
# additionally requires `gh` authenticated with a credential that can read
# ruleset bypass actors (see .github/workflows/fork-health.yml).
#
# Usage:
#   python3 scripts/generate_governance_fixture.py --output target/fork-health/governance-valid.json
#   scripts/fork-health.sh --fixture target/fork-health/governance-valid.json
#   scripts/fork-health.sh --live [--repo jerudnik/jcode] [--fork-remote github]
set -euo pipefail

repo="jerudnik/jcode"
fork_remote="${FORK_REMOTE:-github}"
fork_point_ref="${FORK_POINT_REF:-fork-point}"
main_branch="main"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest="$repo_root/scripts/required-checks.json"
comparator="$repo_root/scripts/governance_compare.py"
mode=""
fixture=""
# `gh` is indirected so tests can substitute a shim that returns planted
# responses. Live mode is otherwise untestable without touching the real API.
gh_bin="${FORK_HEALTH_GH:-gh}"

usage_error() { printf 'error: %s\n' "$*" >&2; exit 2; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo) repo="${2:-}"; [ -n "$repo" ] || usage_error "--repo needs a value"; shift ;;
    --fork-remote) fork_remote="${2:-}"; [ -n "$fork_remote" ] || usage_error "--fork-remote needs a value"; shift ;;
    --fixture)
      [ -z "$mode" ] || usage_error "--fixture and --live are mutually exclusive"
      fixture="${2:-}"; [ -n "$fixture" ] || usage_error "--fixture needs a path"
      mode="fixture"; shift ;;
    --live)
      [ -z "$mode" ] || usage_error "--fixture and --live are mutually exclusive"
      mode="live" ;;
    --manifest) manifest="${2:-}"; [ -n "$manifest" ] || usage_error "--manifest needs a path"; shift ;;
    -h|--help)
      sed -n '2,42p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) usage_error "unknown option: $1" ;;
  esac
  shift
done

[ -n "$mode" ] || usage_error "one of --fixture PATH or --live is required (see --help)"
[ -f "$manifest" ] || usage_error "manifest not found: $manifest"
[ -f "$comparator" ] || usage_error "comparator not found: $comparator"
command -v python3 >/dev/null 2>&1 || usage_error "python3 is required"

# The manifest names the repository this comparison is *for*, and the comparator
# reads it from there rather than from a flag. `--repo` therefore cannot select a
# repository; it can only disagree with the manifest. Accepting a disagreement
# silently would let `--repo someone/else` produce a green run that says nothing
# about someone/else, so a mismatch is a usage error. The scheduled workflow
# passes `--repo "$GITHUB_REPOSITORY"`, which makes this an assertion that the
# workflow is running where the manifest thinks it is.
manifest_repo="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["repository"])' "$manifest")" \
  || usage_error "manifest is not readable JSON with a repository key: $manifest"
if [ "$repo" != "$manifest_repo" ]; then
  usage_error "--repo '$repo' disagrees with the manifest's repository '$manifest_repo'"
fi

failures=0
fail() { printf 'FAIL: %s\n' "$*" >&2; failures=$((failures + 1)); }
ok()   { printf 'OK:   %s\n' "$*"; }

fork_main="$fork_remote/$main_branch"
git show-ref --verify --quiet "refs/remotes/$fork_main" \
  || usage_error "missing ref $fork_main (fetch $fork_remote first)"
if ! git rev-parse --verify --quiet "${fork_point_ref}^{commit}" >/dev/null; then
  usage_error "missing $fork_point_ref tag (fetch tags: git fetch --tags $fork_remote)"
fi
fork_point="$(git rev-parse "${fork_point_ref}^{commit}")"

echo "=== Fork health: $repo (governance source: $mode) ==="

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

# ── 2) CI table currency ─────────────────────────────────────────────────────
# docs/BRANCHING.md documents what each workflow is for. A table that silently
# omits a workflow is worse than no table: it reads as complete. This was not
# hypothetical, ios.yml went undocumented until this check was
# written. Only presence is checked; the prose is a human's job.
undocumented=""
for wf in "$repo_root"/.github/workflows/*.yml; do
  name="${wf##*/}"
  grep -qF "\`$name\`" "$repo_root/docs/BRANCHING.md" || undocumented+="$name"$'\n'
done
if [ -z "$undocumented" ]; then
  ok "docs/BRANCHING.md documents every workflow"
else
  fail "workflows missing from the docs/BRANCHING.md CI table:"
  printf '%s' "$undocumented" | sed 's/^/      /' >&2
fi

# ── 3) Windows CI stays out ──────────────────────────────────────────────────
# Windows CI was removed in issue #19: nothing consumes the artifacts, this
# fork publishes no releases, and no maintainer runs Windows, so the jobs were
# pure noise. Harvesting an upstream workflow change is the obvious way for
# them to return, and they would return *silently*, as green jobs nobody asked
# for. Windows remains a runtime target: cfg(windows) code, docs/WINDOWS.md,
# and scripts/install.ps1 are deliberately still here, so this checks CI
# configuration only.
windows_ci="$(grep -rlEi 'windows-latest|windows-11|pc-windows-msvc|cargo xwin|shell: (pwsh|powershell)' \
  "$repo_root/.github/workflows/" 2>/dev/null || true)"
if [ -z "$windows_ci" ]; then
  ok "no Windows CI jobs (issue #19)"
else
  fail "Windows CI has returned to these workflows (see issue #19):"
  printf '%s\n' "$windows_ci" | sed 's/^/      /' >&2
fi

# ── 4) Governance comparison ─────────────────────────────────────────────────
# Branch protection lives in GitHub rulesets: repository config that no file in
# a clone reveals, so it drifts silently. The rulesets named the retired rails
# for weeks after those rails were deleted, and the stale entries were what
# blocked the deletion. Since R07 this comparison is load-bearing rather than
# confirmatory (design.md section 4), so it compares the complete surfaces
# against scripts/required-checks.json rather than grepping for rail names.
if [ "$mode" = fixture ]; then
  [ -f "$fixture" ] || usage_error "fixture not found: $fixture"
  governance_args=(--snapshot "$fixture")
else
  # Live acquisition lives in the comparator so the snapshot schema has one
  # definition and every gh call is a bare `gh api <path>`; see that file's
  # "Live acquisition" section. Workflow text comes from the working tree,
  # because the required-context contract is a property of the checked-out
  # commit, not of the remote.
  governance_args=(--live --workflows-dir "$repo_root/.github/workflows")
fi

set +e
FORK_HEALTH_GH="$gh_bin" python3 "$comparator" --manifest "$manifest" "${governance_args[@]}"
governance_status=$?
set -e

case "$governance_status" in
  0) ok "governance snapshot matches scripts/required-checks.json" ;;
  1) fail "governance comparison found mismatches (listed above)" ;;
  *)
    printf 'error: governance comparison could not be completed (exit %s)\n' "$governance_status" >&2
    exit 2
    ;;
esac

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
