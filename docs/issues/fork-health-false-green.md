---
status: open
priority: low
owner: maintainers
opened: 2026-08-15
related:
  - .github/workflows/fork-health.yml
  - scripts/fork-health.sh
---

# Fork-health: two open maintainer decisions

The original defect here is fixed and regression-pinned; this file now holds
only the two decisions it surfaced. History: the scheduled `fork-health`
workflow reported success while its guard exited 2, because the guard was
piped through `tee` without `pipefail`. Fixed in `074f50998` (2026-08-17);
`RULESET_AUDIT_TOKEN` is defined and the live comparison runs green; PR #200
added the regression tests (`tests/test_governance_compare.py`:
`test_live_without_credential_is_exit_two_end_to_end`, mutation-proven, and
`test_fork_health_workflow_step_fails_when_the_guard_fails`, which reds if the
pipefail line is ever removed). The protected-path count history (7 → 32 → 5 →
10 across `621f4d44d`/`8907e568d`/`cb6edabf9`) is reconciled in the planning
records.

## Decision 1: is the G10A protected-path reduction's residue accepted?

Commit `621f4d44d` ("Change local governance definition, Modernization-Node:
G10A", 2026-08-08) cut the protected set from 31 paths to 5. Five governance
paths have since been restored on evidence (10 today). The residue — roughly
21 formerly protected `scripts/`/`tests/` files — remains unprotected. Confirm
that residue is intentional, or nominate additions via the manifest's
`proposed_additions` flow.

## Decision 2: fork-point tag detection lag

The `fork-point` tag has no tag ruleset (both live rulesets are
branch-target). An unauthorized tag move is detected only by the daily
fork-health run, a ~24-hour lag. Accept the lag, or add a tag ruleset — noting
that a ruleset change alters the writable contract hash and is a governance
event in its own right.

Also unrecorded anywhere in-repo: the `RULESET_AUDIT_TOKEN`'s owner, scopes,
and rotation story (it demonstrably works; nothing documents its lifecycle).
