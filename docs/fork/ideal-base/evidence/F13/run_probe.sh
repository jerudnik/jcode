#!/usr/bin/env bash
# F13 concurrency probe runner. Builds the out-of-tree probe crate against the
# workspace's jcode-base (shared target dir for cache reuse) and runs it.
# Exit 0 = all cap invariants held.
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/../../../../.." && pwd)
export PATH="/nix/var/nix/profiles/default/bin:$PATH"
cd "$here/probe"
CARGO_TARGET_DIR="$repo/target" nix develop "$repo" -c cargo build --release
"$repo/target/release/f13-probe" | tee "$here/probe_output.txt"
