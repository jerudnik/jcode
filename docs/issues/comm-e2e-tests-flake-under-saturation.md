---
status: open
priority: low
owner: unassigned
opened: 2026-08-20
related:
  - crates/jcode-app-core/src/tool/communicate_tests/end_to_end.rs
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

## Mechanism (localized, not yet fixed)

Each test takes `crate::storage::lock_test_env()` — a process-global lease
with ~246 use sites across the test binary — and then runs multi-agent
socket-pair flows under **fixed 30-second inner timeouts**
(`tokio::time::timeout(Duration::from_secs(30), ...)` and
`.read_until(Duration::from_secs(30), ...)`). Under full-suite parallelism the
lease queue plus CPU saturation can consume most of that budget before the
flow even starts, so the timeout measures machine load, not the behavior under
test. Serial reruns pass because the lease is uncontended.

## Suggested direction

Either start the inner timeout clocks *after* the lease is acquired, or raise
the fixed 30s budgets for this family (a slow pass is cheaper than a flaky
fail; the long budget only costs time when the test is genuinely broken).
Marking the family serial-only would also work but hides the saturation class
instead of tolerating it.

## Related

- `docs/issues/batch-recursion-test-flake.md` — the same shape (order/timing
  dependence in the full-suite run) with the diagnosis still pending; this one
  has its mechanism localized.
