# §4 maintenance window: PR #132 (D02 expansion guard)

Prepared 2026-08-06. When executed this will be the eighth use of the procedure
under the ordinal series that restarted at PR #100, and the fifteenth recorded
window overall. Recorded because §4 makes every write and read-back in a window
evidence.

PR: <https://github.com/jerudnik/jcode/pull/132>, head branch
`automation/d02-expansion-guard`. The protected-path prediction below was
scored against head `eefc91a882ffef68fbd9ddb12f7669a61e5fc3dd`.

That SHA is a record of what was scored, **not the head SHA to use at window
time**: committing this file advances the branch head past it. Step 1 re-reads
the live head and stops on a mismatch, and that read is the authority. Any
later commit to this branch, including this one, also re-runs CI, so the
prediction is re-scored rather than inherited.

**Status: EXECUTED 2026-08-06.** Authorized by the repository owner, run at
02:42:32Z, closed 02:42:37Z, merge `73d8a640a7acbca3d6b7d16b0d2027e7eeb8542b`.
The pre-window sections below were derived and committed **before** the window
opened, so the prediction is scored against reality rather than written up
afterwards to match it.

## Why a window is needed

The PR edits `scripts/ideal_base_railway.py` and
`tests/test_ideal_base_railway.py`, both on the protected list. `Governance
Root` fails by design on a protected-path change: it detects an unreviewed
governance-path change on the PR that makes it, so a legitimate change cannot
make it green. `--admin` is refused because `protect-fork-rails` carries
`bypass_actors: []`, which binds the owner-admin too.

The trip is predicted before pushing, not discovered from CI. Parsing the inline
`protected=( ... )` array out of `.github/workflows/governance-root.yml` yields
**32 patterns**, and matching the 6 changed files against it:

    ok   docs/fork/ideal-base/STATE.json
    ok   docs/fork/ideal-base/WORK_GRAPH.json
    ok   docs/fork/ideal-base/evidence/R07/maintenance-window-d02-2026-08-06.md
    ok   scripts/d02_scoreboard.sh
    HIT  scripts/ideal_base_railway.py
    HIT  tests/test_ideal_base_railway.py

The parse asserts the pattern list is non-empty. A zero-pattern parse reads as
"no protected files touched", which is a false all-clear this program has
already produced: the PR #106 window records the predictor reporting **0
protected paths** because it read keys that do not exist in
`scripts/required-checks.json` instead of the inline list in
`governance-root.yml`. That record's own conclusion is that a predictor
reporting "no protected paths" on a repo with 32 of them is worse than none.

`scripts/d02_scoreboard.sh` was checked against the list rather than assumed
clean. It is not protected, so committing it does not change the prediction.

**The prediction is only tested when CI fails naming exactly those two files and
nothing belonging to anybody else's work.** That is read from the failure log,
not assumed. A window that trips on paths outside this change is a different
problem and must not be executed as if it were this one.

**Scored against PR #132, run 31064263271.** `Governance Root` failed in
`Detect governance-path changes`, and the log names:

    ##[error]governance paths changed; use the recorded ruleset maintenance
             procedure (design.md section 4)
    scripts/ideal_base_railway.py
    tests/test_ideal_base_railway.py

Two files, both predicted, nothing outside this change. The predicted set and
the observed set are identical, so the window is scoped to this PR's own work
and step 1 may proceed once authorized. Had CI named a third path, or a path
belonging to someone else's commit, this would be a different window and must
not be executed as this one.

Authorization is required from the repository owner. It was requested before any
write and **granted on 2026-08-06**, after which the window was executed. The
window is a write to a protected repository and is not self-authorized.

## Pre-flight, run before any write

Under the repository's own pinned `scripts/governance_compare.py` `sanitize()`
and `canonical()`, imported rather than reimplemented. A hash from an invented
encoder proves nothing about the body the gate compares.

    pre-window governed hash  : 43ba61a7a57ffded7a4276917192cdd6028f79d58755cae870cbb2df07494f2b
    restore body key sets     : equal, 6 keys, missing NONE
    restore differing values  : NONE            -> restore is exact
    drop differing values     : ['rules']       -> exactly the intended scope
    drop contexts             : 4 -> 3, removing only `Governance Root`
    remaining contexts        : Fork CI Gate, Security Gate, Nix Gate

The comparison is field-by-field on the sanitized bodies, not GET-hash against
PUT-hash. That comparison is invalid because the two shapes are not required to
be identical documents; the PR #130 window established this after a hash
mismatch that turned out to be a real defect in the restore body.

Two controls, each failing on a **different** assertion:

| control | mutation | caught by |
|---|---|---|
| A | replant the PR #130 defect: five keys, omitting `bypass_actors` | key-set check; reproduces mismatch hash `6f700457b6246b82` |
| B | six keys, but `enforcement` mutated to `disabled` | field-by-field value diff only |

Control B is the one that justifies keeping the value diff. The key-set check is
**blind** to it, so a preflight built on key sets alone would pass a body that
disables the ruleset outright.

Artifacts are in `/tmp/gwin`: `live_get.json`, `restore_body.json`,
`drop_body.json`, `expected_base_sha.txt`.

## Identity asserts: captured, and stale by default

Six asserts, all PASS, captured at main tip
`511064cd1771a2dffd2c9a8f58e1606991844960`:

    repo id                        1238606714
    required-check integration_id  {15368}
    ruleset                        protect-fork-rails / active
    bypass_actors                  []
    rulesets                       no-stray-branches:18509016, protect-fork-rails:18509013
    auth                           jerudnik, scopes: gist, read:org, repo, workflow

These are stale the moment main moves. `./scripts/d02_scoreboard.sh` prints the
captured base SHA next to current `github/main` and warns on divergence, so this
is checked rather than remembered. **They must be re-run against the actual PR
head immediately before the window opens.** Step 1's head-SHA precondition is a
STOP condition, not a retry.

## What lands

One `elif` branch in `expansion_violations` plus its tests: a root in
`DEPENDENCY_COMPLETE` with any child outside it is a violation, and the message
names the stranded children by id.

The stranding mechanism was demonstrated, not argued. `ready_nodes` skips any
root outside `{in_progress, implemented, verifying, blocked}`, so the same
pending child disappears purely because its root closed:

    root=in_progress  child C pending -> offered ['C']
    root=accepted     child C pending -> offered []

This is the mirror of the rule directly above it, which catches an incomplete
root under fully complete children. The gap was found by simulation during the
D01-FIX-3 window and written down as D02 before it had a patch.

Engineering evidence collapses to `./scripts/d02_scoreboard.sh`, exit 0 only
when every integer is at its required value:

    tests_exit 0 | tests_ran 29 | guard_fires_and_is_quiet 0
    railway_check_exit 0 | protected_patterns 32 | protected_hits 0

The missing reviewed-commit object (`b238d7034fdef981a2430224e71b9e6daed2cf23`) is
the separate validation failure that still trips `tests.test_ideal_base_railway`.
The scoreboard baselines above now track the live tree, so that failure is not
misread as a D02 baseline mismatch.

Counters proven responsive, each mutation confirmed on disk before any exit code
was read, each restored byte-identical via `diff -q`:

| mutation | counters | assertion that fails |
|---|---|---|
| `elif False:` | 2 | `did NOT fire on a complete root over a pending child` |
| `elif True:` | 3 | `fired on a fully complete wave` |

`railway_check_exit` correctly stays 0 under the first and moves 0 -> 1 under the
second. It should not move when the guard is merely inert, because main is not
in the forbidden state; it must move when the guard over-fires, because an
over-firing guard flags the live tree. The second is the acceptance-side
control: without it, a guard that flagged everything would pass a fires-only
check.

The window itself is deliberately absent from that scoreboard. It is blocked on
authorization, and scoring it would misreport "waiting on a human" as "work
unfinished".

## Sequence executed

Executed 2026-08-06 by `transcripts/window-d02.py`, adapted from
`transcripts/window-pr76.py`. Full output: `/tmp/d02_window_transcript.txt`.

The script defaults to a **dry run**; opening a real window requires typing
`--commit`. That default is inherited from the pr76 script, which records a
harness believed read-only performing a live governance write because
`--dry-run` was opt-in.

| # | step | result |
|---|---|---|
| 1 | confirm PR state and head SHA precondition | head `9635f94bd8` == reviewed head; live `main` == PR base `511064cd17`, no drift; `Governance Root` failure, other three success; 6 files, matching declared `changed_files` |
| 2 | capture pre-change ruleset | 4 contexts, `enforcement=active`, `target=branch`, `bypass_actors=[]`, strict policy on, all `integration_id=15368` |
| 3 | hash with the pinned encoder | `43ba61a7a5...94f2b`, equal to the known-good steady state |
| 4 | record pre-window main tip | `511064cd1771a2dffd2c9a8f58e1606991844960` |
| 5 | pre-window `fork-health.sh --live` | all invariants hold (captured pre-window) |
| 6 | PUT dropping **only** `Governance Root` | OPEN 02:42:32Z; read-back exact; contexts `['Fork CI Gate','Nix Gate','Security Gate']` |
| 7 | SHA-conditioned merge to reviewed head | merged `73d8a640a7acbca3d6b7d16b0d2027e7eeb8542b` (`sha` pinned to the reviewed head) |
| 8 | PUT restoring the exact pre-change body | CLOSED 02:42:37Z; **window open 5 seconds** |
| 9 | body equality proof | fresh GET re-hashed `43ba61a7a5...94f2b`, identical to step 3; 4 contexts, `active`, `bypass_actors=[]` |
| 10 | merge-commit bound | both forms return **1** (see below) |
| 11 | two-parent proof | parents exactly `[511064cd17, 9635f94bd8]`; one first-parent merge in the window, equal to the merge SHA |
| 12 | post-window `fork-health.sh --live` | all invariants hold; `railway check` at the new published ref exits 0 |

Steps 5 and 12 are the two independent comparators. Step 9's re-hash is a
**fresh GET decoded again**, not the script's own read-back trusted twice.

Post-merge verification on the merged commit, reading
`scripts/ideal_base_railway.py` out of `73d8a640a7` rather than the worktree:

    silent strand (root accepted, child in_progress) -> REJECTED, names c2
    fully complete wave                              -> quiet, no false positive

The suite passes on merged `main` (27 tests, exit 0).

### One counter is expected to read differently after the merge

`./scripts/d02_scoreboard.sh` now reports `protected_hits 0 (want 2)`. This is
the counter going **obsolete**, not a regression. It counts protected files in
`git diff github/main..HEAD`, and that diff is empty because the change landed:
both files are in the merge commit. The counter measured a pre-merge condition
and 2 was only ever the correct answer while the work was unlanded. Verified by
confirming both files are present in `73d8a640a7`, and by exercising the guard
directly against the merged source, which is the assertion that actually
matters.

### Merge-commit bound, both forms

    git rev-list --count --merges 511064cd17..github/main              => 1
    git rev-list --count --merges 511064cd17..github/main --not 9635f94bd8 => 1

Per the #101 correction, the `--not` form is required: without it a
merge-commit head returns 2 and reads as an unreviewed second write. Both are
recorded because they agree only when the branch is linear.

## Correction carried into this window's script

`window-pr76.py` read the protected-path list from
`scripts/required-checks.json`. **That file holds 31 entries; the inline
`protected=( ... )` array in `.github/workflows/governance-root.yml`, which is
what the gate actually runs, holds 32.** The extra entry is
`scripts/test_warning_budget.py`. Reading the JSON is the same defect class that
produced the PR #106 false all-clear.

`window-d02.py` parses the workflow array, asserts it is non-empty, and
additionally cross-checks it against the JSON, printing any divergence instead
of silently preferring one source. For this window both sources yield the same
two protected files, and that agreement is recorded rather than assumed.

The divergence is **not closed by this window** and is now a live finding:
`fork-health.sh --live` reports `enforcing 31 paths`, so the health check and
the gate disagree about how many paths are protected. A change touching only
`scripts/test_warning_budget.py` would be called clean by the health check and
tripped by the gate. That is a follow-up, not part of D02.

### Controls on the window script

Five controls, each planted on a copy so the executed script was never mutated,
each mutation asserted present on disk before its exit code was read, and each
failing on a **different** assertion:

| control | mutation | assertion that fails |
|---|---|---|
| 1 | `REVIEWED_HEAD` set to zeros | head identity: "branch was re-pushed since review" |
| 2 | `STEADY_STATE` corrupted | governed hash != known-good steady state |
| 3 | reviewed path expectation narrowed to one file | **acceptance side**: real protected path reads as unreviewed |
| 4 | protected parse forced to `[]` | zero-pattern artifact refused as an all-clear |
| 5 | `DROP` pointed at a passing context | gate state: "Nix Gate is success" |

Control 3 is the acceptance-side control. Without it, a script that ignored an
unexpected protected path would still pass a trips-only check. All five exited
1; the unmutated script was then confirmed byte-different from every copy and
re-passed its dry run before `--commit` was used.

## What this procedure does not close

The ruleset PUT has **no expected-main-SHA precondition**, so the assert and the
write cannot be made atomic. An ordinary PR can still merge during the window
through the three still-required contexts. Step 7 **detects** that; it does not
prevent it.

A step 7 mismatch is a governance incident requiring out-of-band investigation.
It is not closed by re-running step 6.

The §114 revert-between-samples gap means this record, like every prior one,
does not claim continuous coverage of the window interval.
