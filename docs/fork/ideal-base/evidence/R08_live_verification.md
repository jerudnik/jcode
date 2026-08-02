# R08 live verification

Run on the reloaded selfdev binary at `3c27ab5ff`, not in a test harness.

## Gate 1: the ownership fault names the group and the fault

Probe: a todo group labelled `r08 live probe` completed in the same write as a
goal deliberately filed under `r08-live-probe`, carrying
`end_to_end_ownership: 100`.

Observed on the previous binary, six times earlier in the same session, for
writes that were already persisted:

    Your hill-climbability is not high enough. First, improve the goal's
    objective and feedback loop ... Then call the todo tool again ...

False in two ways. The write had been saved, and (for the ownership variant)
the score existed and exceeded the threshold. Unactionable as well: re-scoring
cannot resolve a label mismatch.

Observed on `3c27ab5ff`:

    Your todo update was saved. This is a nudge about end-to-end ownership, not
    a rejection: nothing was discarded. The group "r08 live probe" is now
    complete, but no goal assessment carries that exact label, so its ownership
    was never assessed. Check that the goal's group matches the todos' group,
    or add a goal for "r08 live probe".

PASS. It names the group, identifies the fault as a missing assessment rather
than a low one, states that the write survived, and discloses no score or
threshold.

This is a real control rather than a re-reading: the identical write produced
the generic sentence on the prior binary.

## Gate 4: the unblocked poke text is unchanged

The reload poke arrived as:

    You have 4 incomplete todos. Continue working, or update the todo tool.

Byte-identical to the pinned string, confirming live that adding the blocked
disclosure above the trailing sentence did not disturb the ordinary path or
`is_auto_poke_message`.

## Not verified live

The blocked-todo branch itself. Reaching it requires a session whose only
outstanding todos carry `blocked_by`, which cannot be staged from inside the
session under test without ending the run. It is covered by the three
`r08_gate4_*` tests, each proven by a control that fails.
