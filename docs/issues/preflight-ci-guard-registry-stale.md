---
title: "Fork CI runs preflight, but 10 guard-registry entries still describe it as local-only"
status: open
priority: high
owner: maintainers
opened: 2026-08-28
related:
  - scripts/check_guard_nonvacuity.py
  - scripts/preflight.sh
  - .github/workflows/fork-ci.yml
  - docs/issues/undecided-bucket-wire-or-retire.md
  - https://github.com/jerudnik/jcode/pull/219
---

# Fork CI runs preflight, but the guard registry still calls its checks dormant

PR #219 made Fork CI run this blocking step:

```bash
scripts/preflight.sh --ratchets-only --no-branch-handoff
```

The step passed on PR head `73e8289e6` in Actions job `98873387034` and is now
on `main`. The registry in `scripts/check_guard_nonvacuity.py` still reports 17
gating and 16 dormant entries, including checks that the new step executes.
Several reasons now make false claims such as "nothing executes it" or
"preflight is a developer entry point that no workflow runs."

## Entries that now run in blocking CI

`--ratchets-only` runs these eight guards unconditionally, but the registry
still marks each one `DORMANT`:

- `scripts/check_agent_instructions.py`
- `scripts/check_ambient_roots.sh`
- `scripts/check_config_env_lease.py`
- `scripts/check_dependency_boundaries.py`
- `scripts/check_env_lease_drop_order.py`
- `scripts/check_tui_render_lock.py`
- `scripts/check_warning_budget.sh`
- `scripts/check_wildcard_reexport_budget.py`

These entries need `GATING` status and wiring evidence for both links in the
path: Fork CI runs preflight, and preflight runs the guard. A claim that proves
only one link can become false while the registry remains green.

## Entries whose reasons are stale but whose checks remain dormant

Two other entries mention preflight but are deliberately skipped by the hosted
command:

- `scripts/check_branch_handoff.py` is skipped by `--no-branch-handoff`.
- `scripts/check_real_home_isolation.sh` is outside `--ratchets-only` and also
  requires `PREFLIGHT_HOME_ISOLATION=1` during a full preflight run.

Their `DORMANT` status may remain correct, but their reasons must name the
actual skip conditions instead of claiming that no workflow runs preflight.

## Count correction

The audit in `docs/issues/undecided-bucket-wire-or-retire.md` cites 17 gating
and 16 dormant entries. Promoting the eight executed guards while leaving the
two skipped guards dormant would make the split 25 gating and 8 dormant. The
implementation must recompute the totals from `GUARDS` and update the audit in
the same commit, rather than preserving the old count by hand.

## Resolution criteria

- The eight CI-executed guards are registered as `GATING` with evidence for the
  complete workflow-to-preflight-to-guard path.
- Removing either the Fork CI preflight step or a guard invocation makes the
  registry test fail.
- The two skipped guards retain accurate reasons for remaining dormant.
- The section 2 totals in the undecided-bucket issue match the registry.
- `python3 -I scripts/check_guard_nonvacuity.py` and its tests pass.

## One reason string also cites a path that no longer exists

Separate from the gating and dormant question, the
`scripts/check_warning_budget.sh` entry cites its test as
`scripts/test_warning_budget.py`. PR #225 moved that module to
`tests/test_warning_budget.py` so the `tests/test_*.py` glob would collect it,
which makes the citation dangling and its "is itself unrun" clause false.

This was fixed once during the #225 merge and then deliberately reverted.
`scripts/check_guard_nonvacuity.py` is a governance path, so the one-line
correction turned an otherwise window-free PR into one needing a ruleset
maintenance window. The fix belongs in the change that rewrites these reason
strings anyway, rather than costing a separate window on its own.

Nothing breaks in the meantime. The path appears in a `reason` string, not in
executable logic, and `check_guard_nonvacuity.py` passes either way at 28
claims.

Add to the resolution criteria: no `reason` string cites a path that does not
exist. Checking this mechanically across `GUARDS` would prevent the next
instance, since a moved file is the same failure mode issue #224 addressed for
documentation citations.
