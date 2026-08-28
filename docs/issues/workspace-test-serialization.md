---
title: Full-workspace test runs are not serialized across concurrent swarm nodes
status: open
priority: medium
owner: unassigned
opened: 2026-08-28
related:
  - docs/agent-workflows.md
---

# Full-workspace test runs are not serialized across concurrent swarm nodes

## What happens

A plan running at concurrency 3 permits three agents to run
`cargo nextest run --profile ci --workspace` at the same time on one machine.
Nothing coordinates them. They do not fail on contention; they take turns
badly, and every agent pays for the queue while it waits.

Measured during a burndown run on 2026-08-28:

```
pid 38917   52 min   swarm-model-policy-enforcement   nextest --workspace
pid 49782   20 min   w3-budgets                       nextest --workspace  (cancelled)
load averages: 20.15 18.60 16.32
```

Two of the three active nodes were in a full-workspace run simultaneously.
One took 52 minutes and finished red. The other was cancelled after 20
minutes without completing.

## Why it costs more than the wall-clock time

The 52-minute run failed on two `jcode-tui` render tests:

```
test_full_redraw_clears_out_of_band_backend_artifacts_after_native_scroll_like_mutation
test_notification_file_activity_repaint_does_not_leave_trailing_digit_artifact
```

The branch under test touched no file under `jcode-tui`. Both tests pass on
that branch's own base with the change absent, in 2.08s and 1.87s. So the run
cost 52 minutes and produced a red that belonged to no change.

A red with no owner is worse than a slow build. It invites the agent to go
debug a defect that is not in its diff, and it teaches everyone that a red
result is worth re-running, which is the habit that lets a real failure
through.

## What is not established

The mechanism is unknown. The obvious reproduction does not work: the same
test binary passed 30 isolated runs, including 24-way parallel at load 14-15.
Whatever the failing run had, it is not merely CPU contention. Candidates
include memory pressure from concurrent `rustc`, `nextest` scheduling across
6656 tests, or interaction with another test in the same process group.

Anyone picking this up should not start with a parallel-copies harness. It
has already been tried and it comes back green.

## Suggested direction

Both workers that hit this arrived at the same shape independently: crate- or
filter-scoped tests while iterating, and one full-workspace run per PR in a
serialized lane rather than once per verification node.

That needs a real lock rather than a convention, since the failure mode is
two agents that each individually did the right thing.

## Evidence

- `target/nextest/ci/slow-tests.log` in the affected worktrees, which also
  recorded a separate test exceeding 60 seconds.
- Isolated runs via the prebuilt binary
  `target/debug/deps/jcode_tui-6510d58bcce2117c`, which reproduces neither
  failure.
