# S01 findings

Findings recorded while building the determinism matrix. These are separate
from the matrix result itself (P1-P5 in PREDICTIONS.md).

---

## S01-F1: selfdev reload is broken on the remote builder (.git excluded)

**Status:** open, real defect, NOT ours, NOT fixed here.
**Severity:** affects remote test runs only; no end-user impact known.

### What happens

`server::debug_command_exec::tests::debug_tool_selfdev_reload_returns_promptly_for_direct_execution`
fails when cargo is routed to the remote builder, and passes locally, at the
**same commit with an identical working tree**.

Measured at `1e356391e`, working tree clean of crate changes:

| Build locus | Command | Result |
|---|---|---|
| remote (`JCODE_REMOTE_CARGO=1`, the machine default) | `dev_cargo.sh test -p jcode-app-core --lib debug_tool_selfdev_reload_...` | **FAILED** |
| local (`JCODE_REMOTE_CARGO=0`) | same | **ok** (1 passed) |

Panic text, captured verbatim:

```
thread '...debug_tool_selfdev_reload_returns_promptly_for_direct_execution'
panicked at crates/jcode-app-core/src/server/debug_command_exec.rs:834:10:
debug selfdev reload should succeed: Could not find jcode repository directory
```

### Why

Three facts compose:

1. `scripts/remote_build.sh:328` rsyncs the tree with `--exclude '.git'`.
2. `is_jcode_repo()` (`crates/jcode-build-support/src/paths.rs:711`) returns
   `false` unless `dir.join(".git").exists()`.
3. `get_repo_dir()` tries `JCODE_REPO_DIR`, then `CARGO_MANIFEST_DIR`
   ancestors, then `current_exe()` ancestors, then `current_dir()` ancestors.
   Every one of those paths is inside the rsynced tree on the remote, so all
   four fail the `.git` predicate and `get_repo_dir()` returns `None`.

The remote shell prints the corroborating banner `install-git-hooks: not in a
git repository` on every run, which is the same root cause observed from a
different consumer.

### Why this is not a code regression

The code is byte-identical in the passing and failing runs. Only the
execution locus differs. `git log` shows the test file was last touched by
`e1d17541e` (F20c), well before this work, and the working tree has no crate
changes.

### Corroboration from the F14 baseline

F14's accepted transcript (frozen at `specimen-f14.log`) contains this test
passing twice, at lines 187 and 744, and contains **zero** occurrences of
`Running on remote` or `not in a git repository`. F14 was a local run. So the
accepted baseline and the pinned S01 rounds share a locus; the pin reproduces
F14's conditions rather than relaxing them.

### Candidate fixes (not applied here, out of S01 scope)

- Set `JCODE_REPO_DIR` on the remote to the synced tree root, and relax
  `is_jcode_repo()` to accept an explicit override without the `.git` probe; or
- sync a minimal `.git` marker; or
- mark the test as requiring a real repo and skip it when `get_repo_dir()` is
  `None`, which trades coverage for green and is the weakest option.

Picking among these is a code change to a shipping path and belongs to its own
node, not to a verification node.

---

## S01-F2: the build locus was an uncontrolled experiment input

**Status:** fixed inside S01 by pinning.

`JCODE_REMOTE_CARGO` is read from `~/.config/jcode/remote-build.env`, a
machine-local file **outside the repository and outside its history**. On this
machine it is `1` (mtime 2026-08-01).

Neither `scripts/run_lifecycle_matrix.sh` nor the original `s01_matrix.sh`
mentioned the variable, so the matrix silently inherited whatever that file
said. Two rounds run either side of an edit to it would not be two rounds of
the same experiment, and no second party could reproduce `H` without also
holding a file that is not in the repo.

`s01_matrix.sh` now pins `export JCODE_REMOTE_CARGO=0` before running.
Precedence verified empirically through the real loader:

```
unset case -> JCODE_REMOTE_CARGO=1     # config file wins when env is unset
pinned case -> JCODE_REMOTE_CARGO=0    # explicit env wins over config file
```

and propagation verified through the `env -u IN_NIX_SHELL -u
DEV_CARGO_NIX_REEXEC` wrapper and a nested subshell (both report `0`).

**This pin narrows no gate.** All 9 matrix steps still run, unmodified. It
fixes *where* they run, not *what* runs. Excluding the step would have been
shrinking a boundary to make a check pass; pinning an environment variable
that should have been pinned from the start is the opposite.

---

## S01-F3: F03 lease fixture passes a retired CLI flag (not ours, real)

**Status:** repaired under S01-FIX-1 (2026-08-06). A fixture defect in F03's
evidence, surfaced by S01.

Round A read `FAIL lease/exit/crash/restart matrix (F03)` with 8 fixture
failures, each `could not acquire fixture lease` followed by `Killed: 9`.

**The `Killed: 9` is a symptom, not the cause.** It is the fixture's own
cleanup on `lease_class_fixtures.sh:119`, which `kill -9`s the daemon after
the lease attempt returns empty. The daemon was healthy: it booted, created
both `jcode-debug.sock` and `jcode.sock`, and answered.

**Cause, read by running the command with stderr visible:**

    error: unexpected argument '--no-update' found

`lease_class_fixtures.sh:85` invokes `jcode debug --no-update --quiet ...`.
That flag was removed by `9238c4d86` *refactor(update): retire runtime binary
updater* (2026-07-27), which is an ancestor of HEAD. The fixture pipes stderr
to `/dev/null`, so a CLI incompatibility presents as a lease failure.

**Proven, not inferred.** Same daemon, same environment, flag dropped:

    $ jcode debug --quiet --socket "$DIR/jcode.sock" shutdown:hold_lease:client-connection
    {"token":2}

**Timeline.** F14's accepted baseline shows `PASS lease/exit/crash/restart
matrix (F03)` at specimen lines 48 and 611, recorded 2026-07-26. The breaking
commit landed 2026-07-27. The fixture has been broken since, and nothing
re-ran it until S01.

**This is not nondeterminism.** It fails identically every time, which is why
it cannot be what P2 is about.

---

## S01-F4: F09 matrix step targets a relocated test suite (not ours, real)

**Status:** repaired under S01-FIX-1 (2026-08-06). A defect in
`scripts/run_lifecycle_matrix.sh`, surfaced by S01.

Round A read `FAIL pending-activation reconcile suite (F09) (exit 97)` with:

    running 0 tests ... 40 filtered out
    dev_cargo: explicit cargo test filter matched zero tests

`run_lifecycle_matrix.sh:85` ran `cargo test -p jcode-build-support reconcile`.
The matching tests left that crate in `e1d17541e` *F20c: retire the dead
distribution surface (#31)* (2026-07-26), an ancestor of HEAD.

**Correction, 2026-08-06.** This finding first said the subsystem was "retired,
not moved". That is wrong, and the error was caught by a control rather than by
re-reading. `e1d17541e` *added* `crates/jcode-app-core/src/tool/selfdev/
reconcile_tests.rs` in the same commit that removed the build-support copy:
`git log --diff-filter=A` on that path returns `e1d17541e`. The tests MOVED.
The original wording came from grepping one retired symbol name
(`pending_activation`) and generalizing from its absence, which is the
empty-result-as-answer trap. Re-derived at HEAD: `grep -c reconcile
crates/jcode-build-support/src/` is 0, while `-p jcode-app-core --lib
selfdev::reconcile` runs 4 tests. So the step was repaired by retargeting to
the tests' real home, not by deleting a step whose subject no longer existed.

Exit 97 is `dev_cargo.sh` refusing a zero-match filter. That guard is correct
and is the only reason this was visible at all; a bare `cargo test` would have
exited 0 on zero tests and the dead step would have read as PASS forever.

**Second-order finding: the guard is locus-dependent.** The first attempt to
control this repair ran `dev_cargo.sh` without pinning the locus, and the dead
filter exited **0**, contradicting the round-A observation of 97. The remote
path prints `running 0 tests ... 40 filtered out` and returns success; only the
local path applies the zero-match refusal. So the same dead step reads FAIL
locally and PASS remotely at one commit. This is a third instance of the S01-F1
pattern (remote and local are not the same environment) and is recorded here
rather than absorbed: had the matrix run remotely, this dead step would never
have been caught.

**Not nondeterminism.** Deterministic failure, same cause every run at a fixed
locus.

---

## S01-F5: my own harness silently failed to back up F14 (mine, fixed)

**Status:** fixed in `s01_matrix.sh`, controlled 3/3.

Round A also read `FAIL F14 evidence NOT restored`. That assertion was added
precisely to catch a scope violation, and it caught a real one.

`mktemp -t s01f14` is the BSD spelling. Under the GNU coreutils on the dev
shell PATH the argument is read as a *template* and rejected:

    mktemp: too few X's in template 's01f14'

so `F14BAK` was empty, `cp "$F14LOG" ""` failed, the run continued under
`set -uo pipefail` (no `-e`), and the restore `cp` silently no-oped. F14's
log was left holding S01's output: 453 lines stamped 20:44-20:46 in place of
the 1119-line accepted baseline.

**Damage was fully recoverable and is repaired.** F14 pins its log hash in its
own `SHA256SUMS`, and the frozen specimen matches it
(`6f2613f87bb54988...`). Restored from the specimen; `shasum -a 256 -c
SHA256SUMS` now reports both files OK.

**Fix:** portable `mktemp "${TMPDIR:-/tmp}/s01f14.XXXXXX"`, a *fatal* exit if
the backup cannot be taken (a missing backup must never again present as a
completed restore), and the post-restore assertion now additionally verifies
against F14's own pinned manifest, since a byte-identical copy of a *wrong*
backup would still satisfy a bare `diff -q`.

**Controlled, not assumed.** C1 portable mktemp succeeds under the dev shell
coreutils. C2 acceptance-side: a corrupted log is rejected by the manifest
check, with the mutation asserted present on disk first. C3 the restored good
log verifies. 3/3.

## S01-F6: real nondeterminism in the F03 client-connection lease step

**Status:** open, product-side (not S01's to fix). This is the first genuinely
nondeterministic failure S01 has produced, and the whole point of the study.

The S01-FIX-1 sweep ran two rounds at one commit against one binary
(`v0.46.0-dev (6fb703745, dirty)`). Round 1: 9/9 steps PASS. Round 2:
`FAIL lease/exit/crash/restart matrix (F03)`, a single fixture assertion,

```
FAIL: [client-connection] daemon exited within 4s of release
      (idle window not restarted)
```

on the assertion that read PASS in round 1. Same commit, same binary, same
harness, same machine, opposite verdicts. `N_FAIL` is not a function of the
tree.

**Why only client-connection.** The idle poller and the atomic idle claim use
two different definitions of quiescence:

| site | predicate |
| --- | --- |
| `lifecycle.rs:165-166` (poll) | `clients == 0 && drain_blocking_count() == 0` |
| `shutdown.rs:118` (claim) | `self.active.is_empty()` |

`drain_blocking_count` (`shutdown.rs:100-106`) deliberately excludes
`ClientConnection`, because design 4.1 C1 abandons connections rather than
waiting for them. So a held `client-connection` lease is invisible to the
poller but visible to the claim. For every other class the two agree, which is
exactly why only this one class flakes.

**The race.** With a client-connection lease held, the poller already sees
quiescence and starts the idle window immediately. Two orderings follow:

- A tick with `elapsed >= timeout` lands *while the lease is still held*. The
  claim refuses (`NotQuiescent`), the poller logs `claim lost to new activity`
  and resets the epoch, so the post-release window is genuinely full. PASS.
- No such tick lands before release. The epoch keeps running from before the
  release, `should_exit` is already satisfied, and the very next tick exits.
  The daemon dies ~1s after release. FAIL.

Confirmed in the preserved logs. Failing run: idle window opens 17:45:38.183
while the lease is held, release at 21:45:47.087 UTC, shutdown decided
21:45:48.185, i.e. **1.1s after release**, and `claim lost to new activity`
appears **0** times. Passing run: the same message appears **1** time and the
window restarts. That grep count is the mechanism, not an inference.

**Reproducer:** `repro-f03-cc.sh`, this directory. It replays only the
client-connection class and, unlike the F03 fixture, keeps the daemon log on
the failure path. Two independent runs at the fixture's 18s hold: 1/8 and 2/16
failures, so roughly a 1-in-10 flake. Note the
F03 fixture `rm -rf`s its runtime dir on exactly this branch, which is why the
sweep produced a verdict with no evidence behind it; that is a second, smaller
fixture defect worth fixing whoever owns F03.

**The mechanism, as a contingency table.** Over all 32 iterations run
(8 at HOLD=18, 16 at HOLD=18, 8 at HOLD=24), counting occurrences of
`claim lost to new activity` in each daemon's own log:

| | FAIL | PASS |
| --- | --- | --- |
| refusals = 0 | **3** | 0 |
| refusals >= 1 | 0 | **29** |

Perfect separation, no off-diagonal cell. The verdict is fully determined by
whether a poll tick with `elapsed >= timeout` landed while the lease was still
held. This is a measured property of the runs, not a reading of the source.

**Differential control.** The mechanism predicts that holding the lease long
enough to guarantee such a tick removes the failure without touching product
code. `HOLD=24` (one extra 10s poll interval past the fixture's 18s):
**8/8 PASS, refusals=1 on every iteration**. At HOLD=18 the same script fails
about 1 in 10. The knob moves the outcome in the predicted direction, and the
refusal count moves with it. Caveat, stated rather than glossed: 8 clean
iterations against a ~10% base rate is a weak bound on its own (~43% chance of
seeing zero failures by luck); the control's force comes from the refusal-count
mechanism being uniform across those 8, not from the pass count.

**Whether this is reachable in production.** `try_admit_client`
(`runtime.rs:334-366`) acquires the guard and *then* increments
`client_count`, so a real connection is only briefly lease-held-but-uncounted,
and on the next tick `clients` is nonzero. The wide-open version of this window
is the debug `hold_lease` path, which takes a client-connection lease with no
connection behind it. So the sharp edge is a test-surface asymmetry rather than
a user-facing hang. The inconsistency between the two quiescence predicates is
still real, and the fixture is a legitimate detector of it.

**Bearing on the S01 predictions.** P1 (`N_FAIL == 0` both rounds) is falsified
again, but for the first time by the phenomenon under study rather than by
harness drift. F3 and F4 were deterministic breakage that would fail identically
every run; this one flips at fixed tree state. It should be scored as a real P1
falsification.

### Where an S01-F6 repair can live (capacity derivation)

Re-derived from the files rather than carried over, because an earlier pass of
this derivation misread `expansions` as a list and got the owner set wrong.

Which node owns the two product files:

| file | owners | states |
| --- | --- | --- |
| `lifecycle.rs` | F02, R01, R05, F25 | all `accepted` |
| `shutdown.rs` | F03, R04, F25 | all `accepted` |

(An earlier note claimed F06 and F10 among these. That was wrong; the list
above is what `WORK_GRAPH.json` actually says.)

Capacity, counted from the graph:

| wave | children | incomplete |
| --- | --- | --- |
| W0-W4R, W6, W7 | 3,10,7,9,11,1,11,2 | 0 |
| W5 | 12 / 12 (full) | S01, S02, S03, S01-FIX-1 |

`MAX_EXPANSION_CHILDREN = 12` and roots are capped at 10, currently 9. So the
only structural room for a new node is the last root slot: W5 is full, and
every other wave is accepted.

**Ownership does NOT forbid it.** I predicted it would, and simulating the
mutation against `validate_ownership` falsified that prediction. A `W8` root
declaring `lifecycle.rs`, `shutdown.rs`, or both is **accepted**, because
`W8.depends_on = ['W7']` puts every earlier wave in its dependency closure and
`ordered()` is satisfied. The negative control confirms the check is live
rather than vacuous: the same node with `depends_on: []` is refused with
`unserialized ownership overlap: F02 ... and S01-FIX-2`. Recording this because
the structural objection was mine and it did not survive being run.

**Conclusion: do not spend the slot anyway.** Two reasons survive, and either
is sufficient:

1. **The design says the product is correct.** Design 4.1 marks `PersistentIdle`
   and `TemporaryIdle` as `n/a` for every class: idle shutdown presupposes
   quiescence. `drain_blocking_count` excluding `ClientConnection` is C1
   `abandon`, on purpose. The divergence between the poller and the claim is
   real but is not a defect the design wants closed by making idle wait on
   connections; that would contradict C1.
2. **Production is not affected.** `try_admit_client` increments `client_count`
   under the same guard, so the uncounted window is one tick wide. The wide
   version exists only behind debug `hold_lease`.

The defect that remains is in the fixture, not the product: the F03 step asserts
a full post-release idle window for a class the poller is documented not to
count, so it is asserting something design 4.1 does not promise. That is
S01-FIX-1's own territory (it already owns `lease_class_fixtures.sh`) and needs
no new node.

This is why S01-FIX-1 stays `blocked` rather than being closed. Gate 3 wants a
sweep at 0 FAIL with every step executing; reaching it means changing the
client-connection assertion to match design 4.1, which is a decision about the
fixture's contract and not a repair I should make silently while claiming the
gate was met on the original terms.

---

## S01-F7: first valid round pair — N_FAIL=0 twice, hashes unequal, cause = libtest interleaving (P4 confirmed)

The first pair where both rounds passed (rounds A/B at `b28bc9b7e`, both
`N_STEP=18 N_FAIL=0`) hashed unequal:

```
77355b302e0e86193736cdd8e1aa62cc842e852306d75907f5c21098e2adf3ee  577  (A)
898163015975f614bd6e743c9c150e191aced3846ab42533aaca369f14fa63bb  577  (B)
```

Localization: a line-sorted diff of the two NORMALIZED transcripts leaves only
the round-label lines (`round=A` vs `round=B`). Every other differing line in
the unsorted diff is a `test ... ok` line present in BOTH transcripts at a
different position, i.e. pure reordering of libtest output, which prints in
completion order across test threads. No verdict token, test name, count,
error, or step label differed. This is exactly prediction P4, and P3's "NOT in
a verdict token" clause held.

Response, per the pre-committed P4 remedy (harness fix, normalizer stays
frozen):

1. `RUST_TEST_THREADS=1` exported by `s01_matrix.sh`, serializing libtest so
   result lines print in a deterministic order.
2. The self-identifying round label removed from the transcript BODY (it made
   hash equality impossible by construction); the label lives in the log
   filename only. This was a harness authoring error in the original script,
   visible in the sorted diff above.

The A/B pair that observed the disagreement is thereby superseded; the
determinism claim rests on a fresh pair run under the pinned harness.

Also observed while stabilizing earlier attempts (environmental, not
determinism findings): the A7 real-home isolation gate legitimately FAILs
while any live jcode session is writing `~/.jcode/sessions/*`; rounds must run
on a quiet machine. And every gate must run inside `nix develop` — the bare
shell lacks cargo and python `tomllib`, which presented as A6 exit-127
failures in the first attempt.

### Superseded C/D pair: prewarm was not rerun after the harness commit

The first pair under the test-order pin also passed twice but hashed unequal:

```
d7822787a3493e113c6859fc61b72981e3cd43fde22d21107239c326ca6a5edd  580  (C)
6eb145b62be373db88c22193b2d68183abcf670bccd7c4b8dea91f89231e2d86  577  (D)
```

The normalized diff had exactly one hunk: C printed three `Compiling ...`
lines and D did not. Sorting both normalized transcripts produced no diff,
so again no outcome changed. C warmed the exact cache D consumed. This is P3's
other predeclared transcript artifact, but the experiment setup was wrong: the
prewarm had run before the final harness commit, not at the exact signoff HEAD.

Response: `prewarm.sh` now exports the same `RUST_TEST_THREADS=1` pin as the
matrix. After this finding is committed, prewarm and both replacement rounds
must run at that one unchanged HEAD. If those replacement hashes differ, the
harness premise is rejected rather than retried again.

---

## S01-F8: first packaged candidate rejected by full preflight and S02 review

Candidate `efb730a9a` passed its two recorded rounds, but it is **superseded**
and is not the final signoff subject. Two independent closeout surfaces found
four blockers before publication:

1. Full `scripts/preflight.sh` failed `rustfmt --check` in
   `tool/selfdev/reload.rs` and `jcode-core/src/env.rs`; Clippy passed.
2. Independent Opus S02 review `41bf14a6a` found
   `S01-FIX-1/gate3-sweep.log` cited by `STATE.json` but absent from Git because
   the operator's global `*.log` ignore swallowed it.
3. The same review found `F03/README.md` still claiming a full post-release
   idle window for all 8 classes after the fixture correctly narrowed that
   assertion to the 7 drain-blocking classes.
4. Branch handoff inspection found the already-reviewed
   `automation/protected-path-equality` governance fix (`1e356391e`) was not an
   ancestor of either `main` or the S01 branch, even though final signoff must
   include it.

Response before any replacement round: preserve the BLOCKED review, merge the
reviewed governance fix, apply only the two rustfmt changes, force-track the
cited sweep log with a checksum manifest, correct F03's prose and checksum,
then prewarm and rerun the pair at one new exact commit. The frozen normalizer
is unchanged.
