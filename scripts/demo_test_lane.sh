#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
temp_dir=$(mktemp -d "${TMPDIR:-/tmp}/jcode-test-lane-demo.XXXXXX")
trap 'rm -rf "$temp_dir"' EXIT
export JCODE_TEST_LANE_PATH="$temp_dir/test-lane.lock"

# Keep bootstrap compilation out of the serialization timing below.
JCODE_TEST_LANE=0 "$repo_root/scripts/test_lane.sh" -- true >/dev/null 2>&1

echo "[$(date +%s)] holder starting a 5-second command"
(
  "$repo_root/scripts/test_lane.sh" --label demo-holder -- sleep 5
  echo "[$(date +%s)] holder finished"
) &
holder_pid=$!

sleep 0.2
started=$SECONDS
echo "[$(date +%s)] waiter requesting the same lane"
"$repo_root/scripts/test_lane.sh" --label demo-waiter -- true
echo "[$(date +%s)] waiter acquired after $((SECONDS - started)) seconds"

wait "$holder_pid"
