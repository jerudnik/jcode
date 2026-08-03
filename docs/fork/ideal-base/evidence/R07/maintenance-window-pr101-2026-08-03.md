# §4 maintenance window: PR #101 (D01-F12 stale-code-path ratchet)

Executed 2026-08-03, the second use of the procedure. Recorded because §4 makes
every write and read-back in a window evidence.

## Why a window was needed

PR #101 edits `scripts/check_docs_references.py` and
`scripts/test_docs_references.py`. Both entered the protected set in #100, so
this is the first change to land under that protection. `Governance Root` fails
by design on a protected-path change: it detects an unreviewed governance-path
change on the PR that makes it, so a legitimate change cannot make it green.

`gh pr merge --admin` is refused because `protect-fork-rails` has
`bypass_actors: []`, which binds the owner-admin too. Precedent: #49, #92, #100.

Authorization was given by the repository owner in session, after the blocker
was posted on the PR and work stopped.

## Sequence executed

| # | step | result |
|---|---|---|
| 1 | capture pre-change ruleset body | `protect-fork-rails`, enforcement `active`, `bypass_actors []`, 4 required contexts |
| 2 | hash with the **pinned** encoder | `43ba61a7a57ffded7a4276917192cdd6028f79d58755cae870cbb2df07494f2b` |
| 3 | record pre-window main tip | `04ece86fe6bb76de396421dbff37864907820b9c` |
| 4 | pre-window `fork-health.sh --live` | snapshot matches manifest, 31 protected paths |
| 5 | PUT dropping **only** `Governance Root` | 4 -> 3, read back `Fork CI Gate, Security Gate, Nix Gate` |
| 6 | SHA-conditioned merge to reviewed head | `merged=true`, merge commit `024b18701` |
| 7 | PUT restoring the exact pre-change body | read back all 4 contexts |
| 8 | body equality proof | pre == post == `43ba61a7a5…`, **RESTORED EXACTLY** |
| 9 | merge-commit bound | exactly 1 merge commit created in the window (see below) |
| 10 | two-parent proof | parent1 == pre-window tip, parent2 == reviewed head `ee6dcc0e7` |
| 11 | post-window `fork-health.sh --live` | snapshot matches manifest, 31 protected paths |

Window open `08:34:32Z`, closed `08:35:01Z`: 29 seconds.

## Two corrections to how the previous record was read

**The hash is not comparable across windows, and it should not be.** This
window's governed-body SHA-256 is `43ba61a7a5…` where #100 recorded
`efc2323f16…`. That is expected: #100 itself added two paths to the protected
set, so the governed body legitimately changed between the windows. The
invariant is pre == post *within* a window, not equality across windows. Anyone
reading these records in sequence should not treat a differing hash as drift.

**The first hash computed in this window was wrong and was discarded.** It was
produced by a hand-rolled sanitization that dropped a guessed key list. The
value only becomes evidence when it comes from the encoder the repository pins,
so it was recomputed by importing `sanitize` and `canonical` from
`scripts/governance_compare.py`. A hash from an invented encoder proves nothing
about the body the gate compares.

Related: the comparator is driven through `scripts/fork-health.sh --live`, whose
manifest is `scripts/required-checks.json`. Invoking `governance_compare.py`
directly with the R07 proposal JSON fails with `KeyError: 'target_branch'`,
because that file is a proposal artifact and not the live manifest.

## The merge count needed a correction, and the raw number was misleading

The naive check `git rev-list --count --merges <pre>..github/main` returned
**2**, not the expected 1. That looked like a second write inside the window.

It was not. The second merge is `ee6dcc0e7`, the reviewed head itself, created
when the branch was brought up to date with main after #102 merged. It was
committed at `04:24:01-04:00`, ten minutes before the window opened, and it
carries 21 completed check runs, so it is reviewed history that entered main as
part of the reviewed payload rather than an unreviewed write.

The correct bound excludes the reviewed head and its ancestors:

    git rev-list --merges --count <pre>..github/main --not <reviewed-head>
    => 1

This distinction matters for any future window opened on a branch that was
merged with main rather than rebased. The count-only form will over-report, and
reading it without the `--not` would either raise a false alarm or, worse,
teach the next reader to ignore the number.

## What this window does not prove

`design.md` §114 still applies: a change made and reverted between two `--live`
samples is not guaranteed to be caught by the audit boundary alone. This record
bounds the window by the merge-commit count and the parent identities and does
not claim continuous coverage.

## Post-merge state

`main` = `024b18701`. `d01_scoreboard.sh` TOTAL **0**. The docs gate reports
`137 active documents, 0 machine-local, 85 stale-code-path at baseline`, the
same numbers CI reported, and 31 tests pass.
