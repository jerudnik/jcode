# §4 maintenance window: PR #119 (railway verify projection)

Executed 2026-08-05, the sixth use of the procedure. Recorded because §4 makes
every write and read-back in a window evidence.

## Why a window was needed

PR #119 edits `scripts/ideal_base_railway.py` and `tests/test_ideal_base_railway.py`,
both on the protected list. `Governance Root` fails by design on a protected-path
change: it detects an unreviewed governance-path change on the PR that makes it,
so a legitimate change cannot make it green. `--admin` is refused because
`protect-fork-rails` has `bypass_actors: []`, which binds the owner-admin too.

CI named exactly those two paths and no others, which was checked rather than
assumed: earlier windows in this program have failed on paths belonging to
somebody else's work.

Authorization was given by the repository owner in session, after the failure was
diagnosed and the pre-window state was captured.

## Sequence executed

| # | step | result |
|---|---|---|
| 1 | confirm PR state | `mergeable=MERGEABLE state=BLOCKED`, sole failure `Governance Root` |
| 2 | capture pre-change ruleset | `protect-fork-rails`, `active`, `bypass_actors []`, 4 contexts |
| 3 | hash with the pinned encoder | `43ba61a7a57ffded7a4276917192cdd6028f79d58755cae870cbb2df07494f2b` |
| 4 | record pre-window main tip | `9ac36a8c7eb82237888570e797ced378843abdee` |
| 5 | pre-window `fork-health.sh --live` | all invariants hold, exit 0 |
| 6 | PUT dropping **only** `Governance Root` | 4 -> 3, read back `Fork CI Gate, Security Gate, Nix Gate` |
| 7 | SHA-conditioned merge to reviewed head | `merged=true`, merge commit `a5e1cb594` |
| 8 | PUT restoring the exact pre-change body | read back all 4 contexts |
| 9 | body equality proof | pre == post == `43ba61a7a5…`, **RESTORED EXACTLY** |
| 10 | merge-commit bound | 1 merge commit created in the window |
| 11 | two-parent proof | parent1 == pre-window tip, parent2 == reviewed head `c879419c3` |
| 12 | post-window `fork-health.sh --live` | all invariants hold, exit 0 |

Window open `06:11:57Z`, closed `06:12:26Z`: **29 seconds**.

## The hash matches #101 and #104, which is the expected result

This window's governed-body SHA-256 is `43ba61a7a5…`, byte-identical to the ones
recorded for #101 and #104. Nothing between those windows and this one changed
the protected set, so the governed body did not change either.

The rule those records established holds in both directions: equality across
windows is evidence only when the protected set was untouched in between, and
inequality is evidence only when it was. #100's `efc2323f16…` differs precisely
because that window changed the set.

The hash was NOT computed with a hand-rolled sanitizer. It uses the repository's
own `scripts/governance_compare.py` `sanitize()` and `canonical()`, imported
rather than reimplemented, because a hash from an invented encoder proves nothing
about the body the gate compares. That is a mistake made once in the #101 window
and not repeated. The ten volatile keys it drops are recorded here so the number
is reproducible: `_links, contexts_url, created_at, current_user_can_bypass, id,
node_id, source, source_type, updated_at, url`.

## Merge-count correction: this window is another control

The #101 record established that `git rev-list --count --merges <pre>..main`
over-reports when the reviewed head is itself a merge commit, and that the
correct bound adds `--not <reviewed-head>`. PR #119's branch was linear, so both
forms were run:

    git rev-list --count --merges <pre>..github/main              => 1
    git rev-list --count --merges <pre>..github/main --not <head> => 1

They agree here, as they did for #104, and disagreed in #101. A correction should
change the answer exactly in the case it was written for and leave every other
case alone.

## What landed, and why it needed the window at all

A defect in `ready_nodes`: it offered only `pending` children, so a node at
`implemented` or `verifying` was invisible in both directions, neither pending
nor dependency-complete. Two nodes were correctly parked at `implemented` (G02,
D01-FIX-2) with their dependents blocked behind them, so the projection went
EMPTY and the railway silently claimed it had no next action. CI caught it as
`[] is not true : railway must always offer some next action`.

The trigger was a checkpoint, but the defect was not the checkpoint: setting
D01-FIX-2 to `accepted` in simulation still returns `[]`.

The test was independently and latently broken. Its "every runnable node must be
pending" rule contradicts `synthesize`, which is only ever emitted for a
NON-pending root, so it would have failed the first time any wave closed. It was
passing by accident of timing.

Verified on merged main: `next` offers `G02 verify` and `D01-FIX-2 verify`, and
the railway suite is 26 tests, exit status read directly rather than through a
pipe, exit 0.
