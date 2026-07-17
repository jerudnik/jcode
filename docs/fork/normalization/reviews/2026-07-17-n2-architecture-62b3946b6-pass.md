All verified. The driver delta uses the supported `JCODE_BUILD_GIT_HASH` override to force the exact hash into the build, then asserts it. Ratchets remain expected-red, rustfmt green.

## Addendum: Exact Candidate `62b3946b63eac0a5082b52fed98087ccafc2160c`

**Package:** `/tmp/jcode-n2-readiness-62b3946b6-final`

### VERDICT: **PASS** for candidate promotion-readiness at `62b3946b6`

- **SHA256SUMS:** all 55 verified OK.
- **Manifest:** 54/54 rows expected==actual, zero mismatches. Diff-path lists identical.
- **binary_version:** `jcode v0.46.0-dev (62b3946b6)` — exact match, EXIT 0.
- **binary_hash_exact:** new gate `grep -F "(62b3946b6)"` passes (0/0). Achieved via the supported `JCODE_BUILD_GIT_HASH` build-script override, not a source edit.
- **Ratchets:** `panic 1/1`, `swallowed 1/1`, `code_size 1/1`, `test_size 1/1`; `panic_exact 31 -> 48`, `swallowed_exact 2987 -> 3074`. rustfmt 0/0.
- **FF-promotable:** `merge-base(main, 62b3946b6) == main`; product tree `36971a03d..62b3946b6` excluding `docs/fork` is **empty** (only `driver.sh` changed).

### Evidence-only commit confirmation
`62b3946b6` changes exactly one file (`driver.sh`) adding the exact-identity build override and assertion. The product/architecture tree is byte-identical to `36971a03d`. **My product/architecture verdict is unchanged.** This commit closes the sole MINOR (cosmetic binary-hash staleness) I raised in the prior review; that finding is now resolved.

### Remaining findings by severity
- CRITICAL: none.
- IMPORTANT: none.
- MINOR: none open. (Prior binary-hash-staleness MINOR is closed by this commit.)

### Unchanged scope caveats
D6 runtime/sandbox, D7 host normalization, and the separate operational D9 signer remain out of my scope. Promotion of `main` stays approval-gated; `main` = `6ca1fcf2`, unmoved.
