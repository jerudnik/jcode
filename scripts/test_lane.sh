#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
crate_dir="$repo_root/crates/jcode-test-lane"
profile="${JCODE_TEST_LANE_PROFILE:-debug}"
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
if [[ "$target_dir" != /* ]]; then
  target_dir="$repo_root/$target_dir"
fi

case "$profile" in
  debug|dev)
    binary="$target_dir/debug/jcode-test-lane"
    build_args=(build -p jcode-test-lane)
    ;;
  *)
    binary="$target_dir/$profile/jcode-test-lane"
    build_args=(build -p jcode-test-lane --profile "$profile")
    ;;
esac

needs_build=false
if [[ ! -x "$binary" ]]; then
  needs_build=true
elif find "$crate_dir" -type f \( -name '*.rs' -o -name Cargo.toml \) -newer "$binary" -print -quit | grep -q .; then
  needs_build=true
fi

if [[ "$needs_build" == true ]]; then
  JCODE_REMOTE_CARGO=0 "$repo_root/scripts/dev_cargo.sh" "${build_args[@]}"
fi

exec "$binary" run "$@"
