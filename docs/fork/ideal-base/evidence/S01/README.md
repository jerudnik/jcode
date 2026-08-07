# S01 deterministic signoff package

## Verdict

**IMPLEMENTED, ready for independent S02 review.** The complete deterministic
ideal-base matrix passed twice at one fixed source commit. Both normalized
transcripts are byte-identical under the normalizer frozen before the first
round, and both rounds left zero owned residue.

This node is not marked `accepted` yet. Railway acceptance is deferred until
S02 independently reviews this package and S03 publishes the reviewed commits
to authoritative `main`.

## Fixed identities

- Source/runtime commit: `356476265ad6164970d2753f24da4dce9bdc89d5`
- Branch: `automation/s01-fix-1`
- Host: `Darwin arm64`
- Dev shell: Rust `rustc 1.96.0 (ac68faa20 2026-05-25)`
- Build locus: local, `JCODE_REMOTE_CARGO=0`
- Test ordering: serial libtest, `RUST_TEST_THREADS=1`
- Provider credentials: stripped; `JCODE_NO_TELEMETRY=1`
- Normalizer contract: `NORMALIZER_SPEC.md`, frozen before execution

`prewarm.sh` completed at the exact source/runtime commit before either final
round. The final executions were originally labeled E and F while the harness
was being stabilized; they are preserved here canonically as `round-A.log` and
`round-B.log`. No commit or cache mutation occurred between them.

## Results

| Quantity | Round A | Round B |
| --- | ---: | ---: |
| Matrix steps | 18 | 18 |
| Failed steps | 0 | 0 |
| Transcript lines | 577 | 577 |
| Normalized SHA-256 | `e7da251556fb96a0151be959d9eb3b42cdebc71e3c1340deb8d826da6e9f5bef` | `e7da251556fb96a0151be959d9eb3b42cdebc71e3c1340deb8d826da6e9f5bef` |
| Owned fixture residue | none | none |
| F14 evidence restored | byte-identical | byte-identical |

The raw transcript hashes intentionally differ because timestamps and durations
are retained in the raw files. `SHA256SUMS` protects those exact bytes;
`NORMALIZED_SHA256SUMS` records the determinism comparison.

Every matrix category passed twice:

- A6 warning, panic, swallowed-error, advisory, instructions, and docs gates
- A4 code/test size, wildcard re-export, dependency, TUI render, and env-lease gates
- A7 ambient-root and real-home isolation gates
- A0-A3 one-round real-process lifecycle matrix
- F14 evidence restoration
- owned-process residue check

The frozen normalizer controls also passed: D1 detects an outcome-bearing
one-character change; D2 erases only listed legitimate variation; D3 accepts
the clean specimen; D4 refuses empty and truncated captures. See `controls.log`.

## Predictions scored

- P1 held: both final rounds passed with `N_FAIL=0`.
- P2 held for the final exact-HEAD, prewarmed pair: hashes are equal.
- P3 was confirmed by superseded setup pairs: differences were cargo compile
  chatter or test-result ordering, never verdicts, counts, errors, or test names.
- P4 was confirmed: the first clean passing pair differed by multithreaded
  libtest result ordering. The harness now pins `RUST_TEST_THREADS=1`; the
  frozen normalizer was not widened.
- P5 held: zero owned residue and byte-identical F14 restoration in both rounds.

`FINDINGS.md` records every superseded attempt and repair rather than hiding
setup failures. In particular, live jcode sessions legitimately contaminate A7
real-home isolation, so final rounds ran with a quiet `~/.jcode/sessions`.

## Validation

Reproduce the recorded checks from the repository root inside `nix develop`:

```bash
bash docs/fork/ideal-base/evidence/S01/prewarm.sh
bash docs/fork/ideal-base/evidence/S01/s01_matrix.sh A
bash docs/fork/ideal-base/evidence/S01/s01_matrix.sh B
python3 docs/fork/ideal-base/evidence/S01/normalize.py --hash \
  docs/fork/ideal-base/evidence/S01/round-A.log
python3 docs/fork/ideal-base/evidence/S01/normalize.py --hash \
  docs/fork/ideal-base/evidence/S01/round-B.log
python3 docs/fork/ideal-base/evidence/S01/controls.py
(cd docs/fork/ideal-base/evidence/S01 && shasum -a 256 -c SHA256SUMS)
```

The matrix itself re-runs active documentation and generated-instruction checks.
The post-round verification also confirmed the frozen F14 `SHA256SUMS` and no
owned fixture process remained.

## Edge cases considered

- cold versus warm Cargo caches
- multithreaded test completion order
- remote versus local build-locus drift
- missing Cargo/Python modules outside `nix develop`
- concurrent writes to the real home session directory
- GNU/BSD `mktemp` syntax divergence
- accidental mutation of F14's accepted evidence
- empty or truncated captures that would hash stably
- a normalizer widened after observing a mismatch

## Open questions and external boundaries

S01 is deterministic/local evidence only. It does not upgrade any external
gate disposition or claim network/provider/package-surface validation beyond
the accepted G01-G05 records. S02 must independently check those dispositions,
graph coverage, source/runtime identity, and this package before S03 synthesis.

## Confidence and what was not checked

Confidence: **high** for the local deterministic claim because the final pair
passed every step, produced equal normalized hashes, passed adversarial
normalizer controls, restored prior evidence, and left zero residue.

Not checked by S01 itself:

- independent review quality or omission detection (owned by S02)
- publication ancestry and final railway acceptance (owned by S03)
- credentials, spend, or network-dependent provider checks not already
  authorized and dispositioned by G01-G05
