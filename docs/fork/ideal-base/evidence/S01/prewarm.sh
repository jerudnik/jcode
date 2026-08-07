#!/usr/bin/env bash
# S01 pre-warm: compile everything both rounds need, BEFORE either round runs.
#
# Why this exists (decided before the first round, not after seeing a hash):
# a cold cargo cache makes round A emit "Compiling <crate>" lines that a warm
# round B does not. That is a fixture difference in the transcript, not a
# determinism finding about the system under test. The honest fix is to make
# both rounds warm. The dishonest fix would be to add a "strip Compiling
# lines" rule to the normalizer after observing a disagreement, which
# NORMALIZER_SPEC.md forbids: the erasure list is closed at N1-N7.
#
# This script compiles; it asserts nothing and produces no evidence. Its only
# output that matters is a populated target/ directory.
#
# Build locus is pinned local for the same reason s01_matrix.sh pins it: see
# FINDINGS.md S01-F2. Pre-warming a remote cache and then running rounds
# locally would warm the wrong cache and defeat the purpose.

set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../../.." && pwd)"
cd "$REPO"

export PATH="$HOME/.cargo/bin:$HOME/.nix-profile/bin:/etc/profiles/per-user/$USER/bin:/run/current-system/sw/bin:/nix/var/nix/profiles/default/bin:/usr/local/bin:/usr/bin:/bin:$PATH"
export JCODE_REMOTE_CARGO=0
export RUST_TEST_THREADS=1

CARGO=(env -u IN_NIX_SHELL -u DEV_CARGO_NIX_REEXEC "$REPO/scripts/dev_cargo.sh")

# Compile the test binaries for every package the matrix exercises, without
# running them. --no-run is the point: warming must not itself execute the
# suites, or the pre-warm becomes an unrecorded extra round.
for pkg in jcode-app-core jcode-base jcode-build-support; do
    printf '[prewarm] compiling test binary: %s\n' "$pkg"
    "${CARGO[@]}" test -p "$pkg" --lib --no-run || {
        printf '[prewarm] FAILED to compile %s\n' "$pkg" >&2
        exit 1
    }
done

printf '[prewarm] done; target/ is warm for both rounds\n'
