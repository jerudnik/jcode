#!/usr/bin/env bash
# ci_local.sh — run the Fork CI build+test job on fleet hardware before pushing.
#
# preflight.sh already mirrors the cheap half of CI (ratchets, fmt, clippy). The
# expensive half is the `Build & Test (macOS)` job: a release build plus the
# full test suite, ~21 minutes on GitHub-hosted macos-latest. That is the class
# of failure that today only surfaces after a costly hosted round-trip.
#
# This script runs that job's EXACT cargo command list (extracted from
# .github/workflows/fork-ci.yml by ci_workflow_commands.py, so it cannot drift)
# through scripts/dev_cargo.sh, which offloads to the configured fleet builder
# (serious-callers-only) with a warm incremental target/.
#
# Performance (measured, aarch64-apple-darwin release+test on the SCO builder):
#   * FIRST run for a given profile/target primes a cold target/ and pays full
#     compilation: ~7 min for the release build alone. This is a one-time cost.
#   * WARM runs rebuild only what changed: the same release build drops to ~40s,
#     so a full pass is a few minutes (test *execution* is CPU-bound and does not
#     cache), versus ~21 min on GitHub-hosted macos-latest.
# The win is real once warm; the first invocation is not representative. This is
# a pre-push correctness+speed gate, not a replacement for the authoritative
# hosted checks that fork PRs still receive.
#
# Usage:
#   scripts/ci_local.sh                 # run the macOS job on the local host triple
#   scripts/ci_local.sh --job linux-tests   # run the Linux job's command list
#   scripts/ci_local.sh --target <triple>   # override the target triple
#   scripts/ci_local.sh --list          # print the commands that would run, then exit
#
# Exit non-zero on the first failing command, mirroring CI's fail-fast steps.
set -uo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root" || { printf 'ci_local: cannot cd to repo root %s\n' "$repo_root" >&2; exit 2; }

job="macos"
target=""
list_only=0

while [ $# -gt 0 ]; do
  case "$1" in
    --job) job="${2:?--job needs a value}"; shift 2 ;;
    --target) target="${2:?--target needs a value}"; shift 2 ;;
    --list) list_only=1; shift ;;
    -h|--help) sed -n '2,23p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) printf 'ci_local: unknown argument %s\n' "$1" >&2; exit 2 ;;
  esac
done

# Default the target to the local host triple so the offloaded build matches the
# machine that actually runs it. The fleet builder is aarch64 macOS, same as the
# CI macOS target, so the default `--job macos` needs no rewrite; an explicit
# --target still wins.
if [ -z "$target" ]; then
  host_triple=$(rustc -vV 2>/dev/null | sed -n 's/^host: //p')
  target="${host_triple:-aarch64-apple-darwin}"
fi

mapfile_compat() {
  # bash 3.2 (macOS system bash) has no `mapfile`; read into an array portably.
  # Populates the global `commands` array from stdin, one element per line.
  commands=()
  while IFS= read -r _line; do
    [ -n "$_line" ] && commands+=("$_line")
  done
}
commands=()
mapfile_compat < <(python3 "$repo_root/scripts/ci_workflow_commands.py" "$job" --host-target "$target")
if [ "${#commands[@]}" -eq 0 ]; then
  printf 'ci_local: no commands extracted for job %s; aborting\n' "$job" >&2
  exit 1
fi

printf 'ci_local: job=%s target=%s (%d commands, via fleet builder)\n' \
  "$job" "$target" "${#commands[@]}"
for cmd in "${commands[@]}"; do
  printf '  %s\n' "$cmd"
done

if [ "$list_only" -eq 1 ]; then
  exit 0
fi

start_all=$(date +%s)
failed=0
declare -a summary=()
for cmd in "${commands[@]}"; do
  printf '\n=== ci_local: %s\n' "$cmd"
  start=$(date +%s)
  # shellcheck disable=SC2086 -- cmd is a controlled cargo invocation from the
  # workflow; word-splitting into argv is intended.
  if "$repo_root/scripts/dev_cargo.sh" ${cmd#cargo }; then
    rc=0
  else
    rc=$?
  fi
  dur=$(( $(date +%s) - start ))
  if [ "$rc" -eq 0 ]; then
    summary+=("PASS ${dur}s  $cmd")
  else
    summary+=("FAIL(${rc}) ${dur}s  $cmd")
    failed=1
    printf 'ci_local: command failed (exit %d); stopping like CI fail-fast\n' "$rc" >&2
    break
  fi
done

printf '\n=== ci_local summary (job=%s, %ds total) ===\n' "$job" "$(( $(date +%s) - start_all ))"
for line in "${summary[@]}"; do
  printf '  %s\n' "$line"
done

if [ "$failed" -ne 0 ]; then
  printf 'ci_local: FAILED — fix before opening a PR (no hosted CI minutes spent)\n' >&2
  exit 1
fi
printf 'ci_local: all %d commands passed on the fleet builder\n' "${#commands[@]}"
