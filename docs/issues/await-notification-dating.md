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

## What sets the lag, since established

The queued fallback was already named above. What was not named is the lever
that decides how long that queue holds, and it is a single boolean.

`dispatch_swarm_await_completion` queues the payload with `urgent: false`.
Urgency is not a display hint — it is the gate on the skip-remaining-tools
path:

- `crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs`, **injection point
  C**, runs before each tool after the first and aborts the rest of the batch
  only `if tool_index > 0 && self.has_urgent_interrupt()`.
- **Injection point D**, "all tools done, before next API call", is where a
  non-urgent interrupt lands. The comment there calls it "the safest point for
  non-urgent injection since all tool_results have been added".

So an await result cannot cut into a tool batch that is already running. It
waits for the batch to drain. **The lag is bounded by the length of the
in-flight tool batch, not by anything in the await machinery** — which is why a
single live observation of nearly three minutes is unremarkable rather than
alarming: it measures the requester's turn, not the await.

`backgrounded_await_completion_is_queued_non_urgently_while_busy` in
`crates/jcode-app-core/src/server/queue_tests.rs` pins the queued half by
calling `dispatch_swarm_await_completion` with the agent lock held, then
asserting the queued interrupt is non-urgent and sourced from
`BackgroundTask`. Flipping that `false` to `true` in production turns the test
red, so it covers the line it names.

Injection point C is now covered too.
`urgent_interrupt_skips_the_rest_of_an_in_flight_tool_batch` and
`non_urgent_interrupt_lets_the_in_flight_tool_batch_finish` in
`crates/jcode-app-core/src/agent_tests.rs` drive `run_once_streaming_mpsc`
against a provider that emits a two-tool batch and assert, respectively, that
an urgent interrupt replaces the second tool result with the skip marker and
that a non-urgent one does not. Disabling the guard turns the first red;
dropping the urgency discrimination from it turns the second red. Injection
point D remains read from the source and is labelled as such: no test
exercises it.

## Still open

- **The lag itself.** Dating makes staleness visible; it does not make delivery
  prompt. A consumer that ignores the stamp is affected exactly as before.
- The nearly-three-minute figure still comes from a single live run and was not
  re-measured. The mechanism above explains what such a number is made of; it
  does not turn one observation into a bound.
- Whether an await completion *should* be urgent is undecided. It is now a
  decision about one argument with a known effect, rather than an open question
  about the delivery design.
- **Injection point D is still untested.** The non-urgent landing point is
  described from the source only.

## Correction

An earlier revision of this entry listed "the skip path itself is untested" as
an open item and said both injection points were read from the source. That was
accurate when written and is no longer: injection point C now has the pair of
tests described above. The claim is corrected in place rather than removed, and
the correction is recorded here, because it was published in that form.

The correction narrows the open set; it does not close it. Injection point D is
still uncovered, and covering C says nothing about the lag itself, which remains
the open item below.
