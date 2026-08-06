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

**Status: PREPARED, NOT EXECUTED.** Everything below the "Sequence executed"
heading is unfilled. The pre-window sections are complete and were derived
before the PR was opened, so the prediction can be scored against reality rather
than written up afterwards to match it.

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

Authorization is required from the repository owner and has not been given.
The window is a write to a protected repository and is not self-authorized.

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

    tests_exit 0 | tests_ran 27 | guard_fires_and_is_quiet 0
    railway_check_exit 0 | protected_patterns 32 | protected_hits 2

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

Not executed. To be filled from the run, following steps 1-8 of design.md §4:

| # | step | result |
|---|---|---|
| 1 | confirm PR state and head SHA precondition | |
| 2 | capture pre-change ruleset | |
| 3 | hash with the pinned encoder | |
| 4 | record pre-window main tip | |
| 5 | pre-window `fork-health.sh --live` | |
| 6 | PUT dropping **only** `Governance Root` | |
| 7 | SHA-conditioned merge to reviewed head | |
| 8 | PUT restoring the exact pre-change body | |
| 9 | body equality proof | |
| 10 | merge-commit bound | |
| 11 | two-parent proof | |
| 12 | post-window `fork-health.sh --live` | |

Step 8 runs `fork-health.sh --live` a second time, giving two independent
comparators rather than one reading trusted twice.

The merge-commit bound is run in both forms, per the correction established in
the #101 window:

    git rev-list --count --merges <pre>..github/main              => expect 1
    git rev-list --count --merges <pre>..github/main --not <head> => expect 1

The `--not` is required. Without it a merge-commit head returns 2 and reads as
an unreviewed second write. They agree only when the branch is linear, so both
are recorded.

## What this procedure does not close

The ruleset PUT has **no expected-main-SHA precondition**, so the assert and the
write cannot be made atomic. An ordinary PR can still merge during the window
through the three still-required contexts. Step 7 **detects** that; it does not
prevent it.

A step 7 mismatch is a governance incident requiring out-of-band investigation.
It is not closed by re-running step 6.

The §114 revert-between-samples gap means this record, like every prior one,
does not claim continuous coverage of the window interval.
