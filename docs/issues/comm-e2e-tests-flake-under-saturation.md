---
status: open
priority: low
owner: unassigned
opened: 2026-08-20
related:
  - crates/jcode-app-core/src/tool/communicate_tests/end_to_end.rs
  - crates/jcode-app-core/src/tool/communicate_tests/e2e_support.rs
---

# Communicate end-to-end tests time out under full-suite concurrency

## Symptom

Three to four of the eight tests in
`crates/jcode-app-core/src/tool/communicate_tests/end_to_end.rs` intermittently
time out during a full `scripts/test_fast.sh` run and pass when rerun
serially. Observed on three independent full-suite runs on 2026-08-19 (6,614/
6,618, 6,618/6,621, and one clean 6,626 pass), always the same test family:

- `communicate_list_and_await_members_work_end_to_end`
- `communicate_message_routes_as_dm_while_broadcast_targets_swarm`
- `communicate_spawn_reports_completion_back_to_spawner`

## Mechanism

Each test takes `crate::storage::lock_test_env()` — a process-global lease
with ~246 use sites across the test binary. The lease is acquired before any
inner timeout starts, so lock contention does not consume the active timeout
budget. The remaining saturation exposure was setup performed inside timed
waits: server startup before socket readiness, per-connection registry and
agent construction before subscribe completion, and repeated worker startup
inside the run-plan churn operation.

## Decision

Keep the environment lock at the top of each test. Complete setup before
starting flow timeout clocks where possible. When setup is inseparable from the
awaited operation, give that setup-bearing section separate headroom instead of
lengthening message, status, or notification flow budgets.

## Update 2026-08-20

Kept the 30-second budget for message, status, notification, and other awaited
flows after setup. Server readiness and subscribe completion now use a separate
60-second setup budget because their required startup work cannot be hoisted
out of the wait. The run-plan churn test uses the same setup budget because
worker creation is the behavior under test and cannot happen before that timed
operation. Assertions are unchanged. The issue remains open until future full
suite runs provide repeated evidence that the saturation flake is gone. Shared
end-to-end fixtures now live in `e2e_support.rs` so the parent test module stays
below its size ratchet.

## Related

- `docs/issues/batch-recursion-test-flake.md` — the same shape (order/timing
  dependence in the full-suite run) with the diagnosis still pending; this one
  has its mechanism localized.
