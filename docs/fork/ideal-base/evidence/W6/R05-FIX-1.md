# R05-FIX-1 — cancel mislabeled as server reload

**State:** accepted. Landed in `9a34ff77b`.

## Re-verification before work

The node was written 2026-07-18. Confirmed still live against shipped code
before changing anything:

- `Request::Cancel { id: u64 }` (`jcode-protocol/src/wire.rs:143`) carries no
  cause. Unchanged.
- `SessionControlHandle::new` is constructed with
  `new_agent.graceful_shutdown_signal()` passed as its
  `stop_current_turn_signal` parameter
  (`server/client_lifecycle.rs:582-587`), and that same handle's signal is
  registered into the server's `shutdown_signals` map
  (`client_lifecycle.rs:591-595`).
- `Request::Cancel` -> `cancel_processing_message` ->
  `session_control.request_cancel()` -> `stop_current_turn_signal.fire()`
  (`server/state.rs:619`).
- Therefore a cancel sets the agent's `graceful_shutdown` bit, and every
  reader could only ask `is_graceful_shutdown()`
  (`turn_streaming_mpsc.rs:599, 1267, 1489, 1608`).

## Reproduction

A probe asserting that a cancel must not set the shutdown bit FAILED against
pre-fix code:

```
panicked at crates/jcode-app-core/src/agent_tests.rs:2422:
R05: a user cancel set the graceful-shutdown bit, so downstream code
will label this cancel as a server reload
```

## Impact

The turn was always correctly aborted; only the explanation was false. The
model was told `[Tool 'bash' interrupted by server reload after 1.2s]` and,
for wait-like tools, invited to "Resume the wait" after a restart that was
never going to happen.

## Fix

`InterruptCause::{Cancel, ServerReload}` stored beside the flag in
`InterruptSignal`, so a reader of `is_set()` can also learn why. `fire()`
remains a cancel; reload fires explicitly at `server/reload.rs:452` and
`agent/interrupts.rs:171`. `reset()` clears the cause. Abort behavior is
unchanged.

## Control

Fix reverted in place, tests byte-identical:

```
against fix      : 8 passed
against pre-fix  : 4 FAILED
  cancel must not claim a server reload:
    [Tool 'bash' interrupted by server reload after 1.2s]
```

Full `jcode-app-core --lib`: 1179 passed, 1 failed. The failure
(`debug_tool_selfdev_reload_returns_promptly_for_direct_execution`) was
re-run on a stashed pristine tree and confirmed pre-existing.

## Note on method

The first probe asserted the *implementation* (two separate bits) rather than
the node's *contract* (honest labeling), so it kept failing after the fix was
already correct. A control that tests your chosen mechanism instead of the
required behavior is a mirror. Rewritten at the behavior level.
