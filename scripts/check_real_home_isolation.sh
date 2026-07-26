#!/usr/bin/env bash
# Gate: a `cargo test` run must not write into the developer's real user state.
#
# Tests resolve storage paths through `jcode_storage`. Any test that reaches a
# root without setting JCODE_HOME used to land in real user state, which is a
# correctness bug before it is a tidiness one: suites then depend on (and
# mutate) whatever the developer happens to have on disk. This repo has already
# paid for that four times: a test read a real provider credential, a second
# raced on the real config cache, a third loaded the real ambient queue, and a
# fourth sorted the model picker by the developer's own `model_picker_usage`
# history (five jcode-tui tests, red locally and green in CI purely because
# CI's home is empty). It also leaked, ~229 stub session files per
# `-p jcode-app-core --lib` run, 7,225 of which had accumulated before the fix.
#
# jcode-storage now redirects test-harness processes to a per-process temp home
# for every ambient root. This gate proves that redirect still holds end to
# end, by running a real suite and asserting no real root gained entries. A
# unit test cannot prove this: the leak arises from *other* crates' test
# binaries calling in, and only an actual `cargo test` process has the layout
# the classifier keys on.
#
# Scope and its limit, stated plainly: this detects *writes*. The picker bug
# was a read-only leak, and no file-count gate can see a read. Reads are
# covered on the other side, by `every_ambient_root_redirects_under_a_test_
# harness` in jcode-storage, which asserts each root resolves outside real user
# state whether or not anything writes to it. The two are complements; neither
# alone covers the defect class.
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

# Every real root jcode-storage can resolve, in the same order as the
# `roots` array in `every_ambient_root_redirects_under_a_test_harness`. Keep
# the two in step: a root that only one of them knows about is a root that is
# only half gated. The platform config dir is listed explicitly because it is
# *not* under ~/.jcode, which is exactly how the model_picker_usage read leak
# survived the first version of this gate.
: "${HOME:?HOME must be set}"
watched_roots=(
  "$HOME/.jcode"
  "$HOME/.jcode/sessions"
)
case "$(uname -s)" in
  Darwin) watched_roots+=("$HOME/Library/Application Support/jcode") ;;
  *)      watched_roots+=("${XDG_CONFIG_HOME:-$HOME/.config}/jcode") ;;
esac

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

count_all_roots() {
  local root
  for root in "${watched_roots[@]}"; do
    printf '%s\n' "$(count_entries "$root")"
  done
}

# Compare current counts against the baseline file, printing every root that
# moved. Returns non-zero if any did. Shared by --verify-only and the
# self-contained run so both report identically.
report_deltas() {
  local baseline=$1 leaked=0 i=0 before after delta root
  local befores=()
  while IFS= read -r before; do befores+=("$before"); done < "$baseline"

  if [ "${#befores[@]}" -ne "${#watched_roots[@]}" ]; then
    printf 'real-home isolation: baseline has %d roots, expected %d.\n' \
      "${#befores[@]}" "${#watched_roots[@]}" >&2
    printf 'The watched-root list changed; re-run --snapshot.\n' >&2
    return 1
  fi

  for root in "${watched_roots[@]}"; do
    before=${befores[$i]}
    after=$(count_entries "$root")
    delta=$((after - before))
    if [ "$delta" -ne 0 ]; then
      leaked=1
      printf '  LEAK %s: %d -> %d (delta %+d)\n' "$root" "$before" "$after" "$delta" >&2
      find "$root" -mindepth 1 -maxdepth 2 -newer "$baseline" 2>/dev/null \
        | head -10 | sed 's/^/       /' >&2 || true
    fi
    i=$((i + 1))
  done
  return "$leaked"
}

fail_with_guidance() {
  printf 'A test resolved a real user root. Set JCODE_HOME to a TempDir in that\n' >&2
  printf 'test, or check jcode_storage::running_under_test_harness().\n' >&2
  exit 1
}

# JCODE_HOME must stay unset: setting it would mask the very defect this gate
# exists to catch, since the explicit override short-circuits the redirect.
unset JCODE_HOME || true

if [ "$mode" = "snapshot" ]; then
  # Record the pre-test state. Explicit baseline rather than "assume HOME is
  # clean": that assumption is true on a fresh CI runner but false on any
  # developer machine, and a gate that only works in one environment is a gate
  # people learn to ignore.
  count_all_roots > "$baseline_file"
  printf 'real-home isolation: baseline recorded in %s (%s across %d roots)\n' \
    "$baseline_file" "$(tr '\n' ' ' < "$baseline_file")" "${#watched_roots[@]}"
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
  if ! report_deltas "$baseline_file"; then
    printf 'real-home isolation FAILED: the test steps wrote into real user state\n' >&2
    fail_with_guidance
  fi
  printf 'real-home isolation: OK (%d roots unchanged)\n' "${#watched_roots[@]}"
  exit 0
fi

run_baseline=${TMPDIR:-/tmp}/jcode-real-home-baseline-run
count_all_roots > "$run_baseline"

printf 'real-home isolation: running %s tests with JCODE_HOME unset...\n' "$pkg" >&2
if ! ./scripts/dev_cargo.sh test -p "$pkg" --lib >/tmp/real_home_isolation_run.txt 2>&1; then
  printf 'real-home isolation: probe suite failed; see /tmp/real_home_isolation_run.txt\n' >&2
  tail -20 /tmp/real_home_isolation_run.txt >&2
  exit 1
fi

if ! report_deltas "$run_baseline"; then
  # Name the offenders: preflight discards stdout, so the failure has to be
  # actionable from stderr alone.
  # shellcheck disable=SC2016  # the backticks are literal text in the message
  printf 'real-home isolation FAILED: `cargo test -p %s` wrote into real user state\n' \
    "$pkg" >&2
  fail_with_guidance
fi

printf 'real-home isolation: OK (%d roots unchanged after %s)\n' \
  "${#watched_roots[@]}" "$pkg"
