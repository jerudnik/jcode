---
status: open
priority: low
owner: maintainers
opened: 2026-08-15
related:
  - .github/workflows/scheduled.yml
  - scripts/fork-health.sh
---

# Fork-health: residue of a fixed defect (decisions recorded)

The original defect is fixed and regression-pinned. History: the scheduled
`fork-health` workflow reported success while its guard exited 2, because the
guard was piped through `tee` without `pipefail`. Fixed in `074f50998`
(2026-08-17); `RULESET_AUDIT_TOKEN` is defined and the live comparison runs
green; PR #200 added the regression tests
(`test_live_without_credential_is_exit_two_end_to_end`, mutation-proven, and
`test_fork_health_workflow_step_fails_when_the_guard_fails`, which reds if the
pipefail line is ever removed).

## Decision 1 (answered 2026-08-20): the G10A reduction was intentional

Session-history archaeology settled it. `621f4d44d`'s 31→5 protected-path cut
executed node G10A of the modernization task graph
(`docs/modernization/TASK_GRAPH.json`), whose designed content read "Change
the local governance definition. Remove Governance Root from the required
checks, limit protected paths to long-lived rules...". The node was
adversarially reviewed by two verify swarms on 2026-08-07 before execution,
and its reversal of F23's earlier protected-path growth was consciously
recorded afterwards (PR #154, `docs/fork/ideal-base/DECISIONS.md`). The
five paths restored since (10 today) were evidence-driven post-audit
additions, consistent with "long-lived rules". The ~21-path residue is
therefore intentional by standing design; new additions go through the
manifest's `proposed_additions` adjudication flow on evidence, as the last
five did.

## Decision 2 (answered 2026-08-20): the tag-lag is accepted for now

The `fork-point` tag has no tag ruleset, so an unauthorized move is detected
only by the daily fork-health run (~24h lag). Accepted unless a finding says
otherwise; adding a tag ruleset would change the writable contract hash and
is a governance event to take deliberately, not as a side effect.

## Still unrecorded

The `RULESET_AUDIT_TOKEN`'s owner, scopes, and rotation story (it demonstrably
works; nothing in-repo documents its lifecycle).
