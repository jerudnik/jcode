---
title: Backgrounded swarm await results are delivered a turn late
status: open
priority: medium
owner: unassigned
opened: 2026-08-18
---

# Backgrounded swarm await results are delivered a turn late

## Summary

`swarm await_members` running in the background resolves promptly, but the
notification it produces is delivered as a **soft interrupt** and surfaces only
when the requesting agent's current turn ends. Observed lag on one live run was
almost three minutes.

The reading is correct at the moment it is taken. The problem is that the
requester sees it at some later, unbounded moment.

## Verified

The delivery path is queued, not immediate.

- `SwarmAwaitCompleted` is defined in `crates/jcode-base/src/bus.rs` and
  carries `session_id`, `completed`, `summary`, and a pre-rendered
  `notification` string.
- It is consumed in `crates/jcode-app-core/src/server/background_tasks.rs`,
  which attempts `run_live_turn_if_idle` and otherwise falls back to
  `queue_soft_interrupt_for_session`. That fallback is the lag: the payload
  waits for the requesting agent's turn boundary.

## Mitigated, not fixed

The payload originally carried no measurement time, so a reading taken at 02:03
and shown at 02:06 was indistinguishable from one taken at 02:06 — the lag was
not merely long, it was **invisible**.

`background_completion_notification` in
`crates/jcode-app-core/src/server/comm_await.rs` now takes a
`resolved_at: DateTime<Utc>`, stamped at the instant the await finalizes, so
the header records the resolution time regardless of how long delivery is
subsequently queued.

Two tests in
`crates/jcode-app-core/src/server/comm_control_tests/await_notification_dating.rs`
drive a real background await and read the published event off the bus:

| Test | Pins |
| --- | --- |
| `background_await_notification_is_dated` | the header carries a parseable RFC3339 stamp bounded by the wall-clock interval spanning the await |
| `the_stamp_is_the_resolution_time_not_the_read_time` | after holding the payload for 2s, it reads as at least 2s old |

The second is the direction test. Without it, an implementation that stamped at
*render* time would satisfy the first assertion while reproducing the exact
defect: a stale result wearing a fresh date.

Both were confirmed to fail when the stamp is removed from the formatter and to
pass when it is restored, so neither is vacuous. The test file was proved to be
compiled by planting a `compile_error!` and observing the build fail with that
marker before any test result was read from it.

## Still open

- **The lag itself.** Dating makes staleness visible; it does not make delivery
  prompt. A consumer that ignores the stamp is affected exactly as before.
- No claim is made about the maximum lag. The nearly-three-minute figure comes
  from a single live run and was not re-measured.
- Whether a backgrounded await *should* be able to interrupt an in-flight turn,
  or whether the requester should poll, is undecided.
