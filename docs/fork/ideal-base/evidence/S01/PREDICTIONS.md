# S01 predictions (recorded BEFORE the two rounds run)

Written before round A executes. Scored afterward, unchanged.

A prediction recorded after the fact is not a prediction. These exist so the
run can come out wrong in a way that is visible.

## P1. Both rounds pass

`N_FAIL == 0` in round A and round B.

Falsified by: any step failing in either round.

Basis: the 15 quality gates are the same ones the Fork CI Gate enforces and
were measured 12/12 green under `nix develop` this session; the lifecycle
matrix was recorded 18 PASS / 0 FAIL at F14.

## P2. The two normalized hashes are equal

`H(round A) == H(round B)`.

Falsified by: any hash difference.

This is the actual determinism claim. It is the one most likely to fail, and
the most informative if it does.

## P3. Named residual non-determinism, if P2 fails

If P2 fails, I predict the difference will be in one of:

- cargo `Compiling`/`Fresh` lines (build-cache state, addressed by pre-warm)
- test execution ORDER within a suite (cargo test is multi-threaded)
- a duration or count format not matched by N2

I predict it will NOT be in a `PASS`/`FAIL` verdict token.

Falsified by: a hash difference located in a verdict token, which would mean
a genuinely flaky gate rather than a transcript artifact.

This prediction matters because the response differs. A transcript artifact is
a harness fix. A moving verdict is a real finding about the system and must be
reported as such, never normalized away.

## P4. Test ordering is the most likely single culprit

Of the P3 candidates, I name test execution order as most likely, because
`cargo test` runs test threads concurrently and prints in completion order.

Falsified by: hashes equal (P2 holds), or difference located elsewhere.

If P4 is the cause, the correct fix is a harness change that makes ordering
deterministic (for example `--test-threads=1` or sorting result lines), decided
and recorded as a harness change, NOT an addition to the normalizer erasure
list. Sorting result lines would be a normalizer change and is therefore
forbidden under the frozen spec; it would require a new spec version committed
before a fresh pair of rounds.

## P5. Zero owned residue

No orphaned MCP fixture children after either round, and the F14 log restored
byte-identical after each round.

Falsified by: `pgrep` finding fixture children, or `diff -q` reporting a
difference.
