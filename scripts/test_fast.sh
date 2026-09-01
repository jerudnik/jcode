#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cargo_exec="$repo_root/scripts/cargo_exec.sh"

slow_log="$repo_root/target/nextest/ci/slow-tests.log"
junit_xml="$repo_root/target/nextest/ci/junit.xml"
mkdir -p "$(dirname "$slow_log")"

echo "=== Fast test loop (nextest: lib + bins) ==="
(
  cd "$repo_root"
  "$repo_root/scripts/test_lane.sh" \
    --label "${JCODE_TEST_LANE_LABEL:-test_fast}" \
    -- "$cargo_exec" nextest run --profile ci --workspace --lib --bins --status-level slow --final-status-level slow "$@"
) 2>&1 | tee "$slow_log"

echo ""
if [[ -x "$repo_root/target/release/jcode" ]]; then
  echo "=== Startup regression check (release binary) ==="
  "$repo_root/scripts/check_startup_budget.sh" "$repo_root/target/release/jcode"
  echo ""
else
  echo "Skipping startup regression check: build release first with cargo build --release"
  echo ""
fi

echo "For full coverage, see: scripts/test_full.sh"
echo "JUnit report: ${junit_xml#$repo_root/}"
echo "Slow-test log: ${slow_log#$repo_root/}"
