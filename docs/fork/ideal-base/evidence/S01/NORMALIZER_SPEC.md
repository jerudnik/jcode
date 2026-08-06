# S01 normalizer specification (FROZEN BEFORE FIRST RUN)

This file is written and committed **before** the first S01 matrix round runs.
It is the contract for `normalize.py`.

## Why this is frozen first

The determinism claim is `H(round1) == H(round2)`, where `H` is the SHA-256 of a
normalized transcript. That claim is worthless if the normalizer may be widened
after a disagreement is observed, because a normalizer tuned until the hashes
agree is indistinguishable from one that emits the empty string. `sha256("")`
is perfectly stable and proves nothing.

Therefore:

- The erasure list below is closed. Adding a rule to it after a round has run
  **voids that run**. The correct response to an unexplained hash disagreement
  is to report the disagreement, not to erase the difference.
- Any change to this file must be committed before the round it governs, and
  the round record must cite the spec commit it ran under.

## Erasure list (closed)

Exactly these classes of legitimate run-to-run variation are erased. Nothing
else is touched.

| # | Class | Pattern intent | Replacement |
|---|-------|----------------|-------------|
| N1 | Wall-clock timestamps | `[HH:MM:SS]` log prefixes and ISO-8601 stamps | `[TS]` |
| N2 | Elapsed durations | `in 12.34s`, `finished in 0.42s`, `took 1m2s` | `in <DUR>` |
| N3 | Process IDs | `pid 12345`, `(pid=12345)`, PID columns in residue output | `pid <PID>` |
| N4 | Temp directory names | `/tmp/...`, `/var/folders/...` randomized segments, `.tmpXXXXXX` | `<TMP>` |
| N5 | Nix store hashes | `/nix/store/<32-char-hash>-` | `/nix/store/<HASH>-` |
| N6 | Absolute `$HOME` paths | the literal expansion of `$HOME` | `<HOME>` |
| N7 | Cargo target fingerprints | `target/debug/deps/<crate>-<16hex>` | `target/debug/deps/<crate>-<FP>` |

N7 is included because the binary fingerprint suffix is a build-input hash, not
a test outcome; it varies with incremental compilation state. It is listed here,
before any run, for the same reason as the others.

## Explicitly NOT erased

These are outcome-bearing and must survive normalization, because erasing them
would let a real regression hash as identical:

- `PASS` / `FAIL` / `ok` / `FAILED` verdict tokens
- test names and counts (`8 passed; 0 failed`)
- error messages, panics, tracebacks
- residue findings (orphan process names)
- step labels
- ordering of steps and of test result lines

## Two independent quantities

A round is summarized by two numbers, both read from files on disk, never
transcribed from scrollback:

- `N_FAIL` = count of failing steps. Must be `0`.
- `H` = SHA-256 of the normalized transcript. Must be equal across rounds.

Neither substitutes for the other. Two identical failures hash identically, so
`H` equality alone says nothing about passing. And `N_FAIL == 0` twice with
differing `H` means something non-deterministic moved that was not a verdict.
Both are required.

## Controls (each fails on a different assertion)

| ID | Plants | Asserts | Expected |
|----|--------|---------|----------|
| D1 | one-character diff into a captured transcript | the normalizer can still see a real change | `H` MUST move |
| D2 | a fake timestamp/pid/tmpdir difference | listed legitimate variation is erased | `H` MUST hold |
| D3 | nothing (acceptance side) | `N_FAIL` is actually `0`, not merely equal | exit 0, `N_FAIL == 0` |
| D4 | an empty and a 3-line transcript | short/empty capture is refused, not hashed | normalizer MUST exit non-zero |

D4 exists because an empty capture hashes stably and reads as perfect
determinism. Line count is asserted `> 0` (and above a floor) before any hash
is computed.

D1 is the primary control: a normalizer that cannot see a planted diff is
rejected outright, whatever the round hashes say.
