All verified. Issuing the addendum.

---

# Review Addendum: Candidate `62b3946b63eac0a5082b52fed98087ccafc2160c`

## VERDICT: **PASS** for promotion-readiness at `62b3946b6`

This commit differs from `36971a03d` by exactly one 4-line change to the evidence driver (`fix(n2): assert exact candidate binary identity`); no product source changed.

## Verified
- **SHA256SUMS**: 55/55 files OK. Manifest = 54 gates (one added vs prior); every gate's actual exit equals expected, only the 4 intended reds (panic/swallowed/code_size/test_size) carry exit 1. All raw logs stamped `HEAD=62b3946b6...`.
- **binary_version**: `jcode v0.46.0-dev (62b3946b6)` (exit 0) — matches the candidate exactly.
- **binary_hash_exact** (new gate): `grep -F (62b3946b6)` against binary_version.txt, exit 0.
- **Driver fix is correct**: `tui_build` now builds with `JCODE_BUILD_GIT_HASH="$(git rev-parse --short HEAD)"`, which `jcode-build-meta/build.rs` declares as `rerun-if-env-changed`, forcing a metadata refresh so the embedded hash cannot lag. The new assertion gate makes identity coherence a hard, self-checking matrix requirement.
- Product-diff scope unchanged (0 jcode-tui files); all prior-review conclusions (trusted-gate scope vs Phase 6, frozen baselines, rollback bundles, fast-forward possibility, tui --lib exclusion honesty) carry over unaffected.

## Prior IMPORTANT finding: **RESOLVED**
The stale-binary finding (`--version` showed `1e5ee7ed0` instead of the candidate) is fixed at the mechanism level, not merely re-run. Binary/build-hash now provably agree with source, satisfying COMPLETION_STANDARD D4.

## Remaining findings by severity
- **CRITICAL / IMPORTANT**: none.
- **MINOR**: (1) The tui --lib "38/37 HOME" diagnostic is still only asserted in a driver comment, not bundled as an artifact (credible, not independently reproduced here). (2) Bundles use SHA-1 git hashing, mitigated by SHA-256 file hashes plus fsck.

## Unchanged operator cautions
Main fast-forward, recovery/archive ref deletion, and runtime symlink repoint remain approval-gated and are authorized by neither review. D6 live-runtime and D7 host normalization remain separately required. I did not run builds/tests myself; I relied on the package's recorded exits and logs and the one-commit driver diff.
