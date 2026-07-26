#!/usr/bin/env bash
# Gate: a `cargo test` run must not write into the developer's real ~/.jcode.
#
# Tests resolve storage paths through `jcode_storage::jcode_dir()`. Any test
# that reaches it without setting JCODE_HOME used to land in the real home,
# which is a correctness bug before it is a tidiness one: suites then depend on
# (and mutate) whatever the developer happens to have on disk. This repo has
# already paid for that three times: a test read a real provider credential, a
# second raced on the real config cache, a third loaded the real ambient queue.
# It also leaked, ~229 stub session files per `-p jcode-app-core --lib` run,
# 7,225 of which had accumulated in ~/.jcode/sessions before the fix.
#
# `jcode_dir()` now redirects test-harness processes to a per-process temp home.
# This gate proves that redirect still holds end to end, by running a real suite
# and asserting the real home did not gain entries. A unit test cannot prove
# this: the leak arises from *other* crates' test binaries calling in, and only
# an actual `cargo test` process has the layout the classifier keys on.
#
# Usage:
#   scripts/check_real_home_isolation.sh              # run a suite, assert no leak
#   scripts/check_real_home_isolation.sh --snapshot   # record a baseline, then
#   scripts/check_real_home_isolation.sh --verify-only # compare against it (CI:
#                                                      # brackets the test steps)
#   JCODE_ISOLATION_PROBE_PKG=jcode-tui scripts/check_real_home_isolation.sh
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

mode=run
for arg in "$@"; do
  case "$arg" in
    --snapshot) mode=snapshot ;;
    --verify-only) mode=verify ;;
    *) printf 'usage: %s [--snapshot|--verify-only]\n' "$0" >&2; exit 2 ;;
  esac
done

pkg=${JCODE_ISOLATION_PROBE_PKG:-jcode-app-core}

real_home=${HOME:?HOME must be set}/.jcode
sessions_dir="$real_home/sessions"
baseline_file=${JCODE_ISOLATION_BASELINE:-${TMPDIR:-/tmp}/jcode-real-home-baseline}

count_entries() {
  # Count, not size: the assertion is "no new files", which is what a leak
  # produces. Missing directory counts as zero so a clean machine still gates.
  if [ -d "$1" ]; then
    find "$1" -mindepth 1 -maxdepth 1 | wc -l | tr -d ' '
  else
    printf '0'
  fi
}

# JCODE_HOME must stay unset: setting it would mask the very defect this gate
# exists to catch, since the explicit override short-circuits the redirect.
unset JCODE_HOME || true

if [ "$mode" = "snapshot" ]; then
  # Record the pre-test state. Explicit baseline rather than "assume HOME is
  # clean": that assumption is true on a fresh CI runner but false on any
  # developer machine, and a gate that only works in one environment is a gate
  # people learn to ignore.
  printf '%s %s\n' "$(count_entries "$real_home")" "$(count_entries "$sessions_dir")" \
    > "$baseline_file"
  printf 'real-home isolation: baseline recorded in %s (%s)\n' \
    "$baseline_file" "$(cat "$baseline_file")"
  exit 0
fi

if [ "$mode" = "verify" ]; then
  # Compare against the recorded baseline. No second suite is paid for: the
  # caller's own test steps ran between --snapshot and here.
  if [ ! -f "$baseline_file" ]; then
    printf 'real-home isolation: no baseline at %s; run --snapshot first.\n' \
      "$baseline_file" >&2
    exit 1
  fi
  read -r before_home before_sessions < "$baseline_file"
  after_home=$(count_entries "$real_home")
  after_sessions=$(count_entries "$sessions_dir")
  home_delta=$((after_home - before_home))
  sessions_delta=$((after_sessions - before_sessions))

  if [ "$home_delta" -ne 0 ] || [ "$sessions_delta" -ne 0 ]; then
    printf 'real-home isolation FAILED: the test steps wrote into %s\n' "$real_home" >&2
    printf '  %s: %d -> %d (delta %+d)\n' \
      "$real_home" "$before_home" "$after_home" "$home_delta" >&2
    printf '  %s: %d -> %d (delta %+d)\n' \
      "$sessions_dir" "$before_sessions" "$after_sessions" "$sessions_delta" >&2
    printf 'newest entries under the real home:\n' >&2
    find "$real_home" -mindepth 1 -maxdepth 2 -newer "$baseline_file" 2>/dev/null \
      | head -20 >&2 || true
    printf 'A test resolved the real home. Set JCODE_HOME to a TempDir in that\n' >&2
    printf 'test, or check jcode_storage::running_under_test_harness().\n' >&2
    exit 1
  fi
  printf 'real-home isolation: OK (%s unchanged at %d entries, sessions at %d)\n' \
    "$real_home" "$after_home" "$after_sessions"
  exit 0
fi

before_home=$(count_entries "$real_home")
before_sessions=$(count_entries "$sessions_dir")

printf 'real-home isolation: running %s tests with JCODE_HOME unset...\n' "$pkg" >&2
if ! ./scripts/dev_cargo.sh test -p "$pkg" --lib >/tmp/real_home_isolation_run.txt 2>&1; then
  printf 'real-home isolation: probe suite failed; see /tmp/real_home_isolation_run.txt\n' >&2
  tail -20 /tmp/real_home_isolation_run.txt >&2
  exit 1
fi

after_home=$(count_entries "$real_home")
after_sessions=$(count_entries "$sessions_dir")

home_delta=$((after_home - before_home))
sessions_delta=$((after_sessions - before_sessions))

if [ "$home_delta" -ne 0 ] || [ "$sessions_delta" -ne 0 ]; then
  # Name the offenders: preflight discards stdout, so the failure has to be
  # actionable from stderr alone.
  # shellcheck disable=SC2016  # the backticks are literal text in the message
  printf 'real-home isolation FAILED: `cargo test -p %s` wrote into %s\n' \
    "$pkg" "$real_home" >&2
  printf '  %s: %d -> %d (delta %+d)\n' \
    "$real_home" "$before_home" "$after_home" "$home_delta" >&2
  printf '  %s: %d -> %d (delta %+d)\n' \
    "$sessions_dir" "$before_sessions" "$after_sessions" "$sessions_delta" >&2
  printf 'newest entries under the real home:\n' >&2
  find "$real_home" -mindepth 1 -maxdepth 2 -newer "$repo_root/Cargo.toml" 2>/dev/null \
    | head -20 >&2 || true
  printf 'A test resolved the real home. Set JCODE_HOME to a TempDir in that\n' >&2
  printf 'test, or check jcode_storage::running_under_test_harness().\n' >&2
  exit 1
fi

printf 'real-home isolation: OK (%s unchanged at %d entries, sessions at %d)\n' \
  "$real_home" "$after_home" "$after_sessions"
