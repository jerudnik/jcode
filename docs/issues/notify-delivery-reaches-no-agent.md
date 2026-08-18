---
title: DM delivery mode notify reaches no agent-visible surface
status: open
priority: high
owner: unassigned
opened: 2026-08-18
---

# DM delivery mode notify reaches no agent-visible surface

## Summary

A swarm message sent with `delivery: notify` performs **no delivery action
toward the recipient agent at all**. It emits one `ServerEvent::Notification`
on the recipient's client channel and returns success. Nothing is queued for
the recipient's model, and no turn is started.

For a **headless** swarm member that channel is drained by a discard loop, so
the send succeeds, the fanout counts a delivery, the sender's tool reports
success, and the message is destroyed. Headless and inline are the default
spawn modes, so this is the default worker shape.

This is the recurring shape: a successful `send()` counted as a delivery.

## Verified

Three facts, each read directly from the code under test.

- The delivery match in `crates/jcode-app-core/src/server/client_comm_message.rs`
  has an **empty arm** for notify. The interrupt arm queues a soft interrupt;
  the wake arm runs a live turn or queues one; the notify arm does nothing.
- The same function computes a `reminder` string telling the recipient it has
  received a message and should act on it. That reminder is passed only to the
  wake arm. Notify never sees it.
- `create_headless_session` in `crates/jcode-app-core/src/server/headless.rs`
  wires the member's `event_tx` to a task whose whole body discards what it
  receives, under the comment "Drain events to keep channel alive". A headless
  member has no attached client and no renderer, so the notification's only
  consumer throws it away.

`fanout_session_event` in `crates/jcode-app-core/src/server/state.rs` falls back
to that `event_tx` when `event_txs` is empty and returns a non-zero delivered
count when the channel accepts the value. Acceptance by a discard loop is
indistinguishable from delivery to a reader.

## Reproduction

`comm_message_notify_to_headless_member_reaches_no_agent_surface` in
`crates/jcode-app-core/src/server/client_comm_tests.rs` drives the real
`handle_comm_message` twice against one fixture whose recipient is headless.
Only the delivery mode differs between the two calls.

- Control arm, `Interrupt`: the body lands in the recipient's soft-interrupt
  queue.
- Treatment arm, `Notify`: the request completes with `Done`, a
  `ServerEvent::Notification` carrying the body really is emitted, and the
  recipient's queue is empty.

The control arm is what makes the empty queue meaningful. Without it, an empty
queue would be equally consistent with a harness that cannot observe delivery
at all.

Both mutations were checked and redden different halves:

- Making notify queue like interrupt fails the treatment assertion with
  `the queue held ["DM from falcon: notify body"]`.
- Making interrupt stop queueing fails the control assertion with
  `control arm: interrupt delivery must reach the recipient's queue`.

## Why it bites the careful sender

`resolve_comm_delivery_mode` defaults a DM with no explicit delivery to `Wake`,
which does reach the recipient. The failure needs a sender who **deliberately**
passes `notify` to avoid derailing a long-running turn. The considerate choice
is the one that silently does nothing, and the tool schema documents only the
enum, not what each value promises.

## Not established

- Whether a **TUI-attached** recipient's model later sees the notification. That
  depends on whether the client renders it into conversation history, which was
  not tested here. The headless case does not depend on that question.
- Whether any live incident was caused by this path. The mechanism is proven
  from the code and the test; no attempt was made to attribute a past run to it.

## Options

Not a recommendation, and the choice changes what notify means.

1. Report honestly. Have the send surface that the recipient had no reader, so
   a no-op stops looking like a success. Smallest change; leaves notify inert
   by design but no longer silent.
2. Deliver at the next turn boundary. Queue the body without interrupting, so
   notify becomes "guaranteed to be seen, never mid-turn". This is the promise
   most senders appear to assume.
3. Leave the behaviour and remove the mode. If notify cannot reach a headless
   worker, offering it for headless targets is a trap.

Option 1 does not fix the class; it makes one instance visible. Option 2 changes
delivery semantics and needs a decision about ordering against wake.
