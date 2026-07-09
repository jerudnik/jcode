# Zero-loss Daemon Reload

Goal: reload the daemon binary for selfdev hot-updates, auth/config changes, and
logging changes without losing in-flight work: running LLM turns, subagent
processes, swarm progress, and attached clients.

## Hard constraint

A live LLM stream and an arbitrary child OS process cannot be portably
transplanted into a new `execve`'d process. The stream includes TCP, TLS,
HTTP/SSE parser state, and provider-side request identity. `SCM_RIGHTS` can pass
a listening socket fd, which is the nginx/Envoy/systemd trick, but the daemon is
the client of the LLM provider; there is no listener to hand off, and the stream
state lives in library memory. CRIU can checkpoint on Linux, but it is
Linux-only, heavy, and not a default answer to the TLS/HTTP problem.

The reload design should not pretend live state can be moved. It must either
drain work before the swap or checkpoint semantic state and resume it after the
swap. The only way to preserve a mid-turn stream itself is to keep that stream
owned by a process that does not reload.

The fork already has durable W1/W2 control-log replay for swarm membership,
tasks, and artifacts. The current loss surface is mid-turn LLM streams, running
child processes, live client UI state, and in-memory work not yet reflected in
the control log.

## Recommendation

Build Stage 1 first and stop there until it is measured insufficient. Stage 1
extends the reload path that already exists:

`await_reload_signal` -> `graceful_shutdown_sessions` ->
`persist_reload_recovery_intents` -> `replace_process` -> `--resume` /
`JCODE_RESUMING`.

Stage 2 is a stable LLM-stream owner. It is the right shape if measured reload
loss remains meaningful after Stage 1, but it is a new service boundary and
should not be introduced preemptively.

## Stage 1: checkpoint-and-resume

Stage 1 makes reload acknowledge saved turn state instead of treating any
liveness update as proof that a session is safe.

1. In `crates/jcode-app-core/src/server/reload.rs`,
   `graceful_shutdown_sessions_with_timeout`, change the wait condition. Today
   the loop treats any `SwarmEventType::StatusChange` or
   `MemberChange { action: "left" }` as "this session is handled." That is only
   a liveness flip, not proof that work was saved. A session should count as
   handled only after it completed normally or appended the checkpoint event
   below. Keep the 2s `RELOAD_GRACEFUL_SHUTDOWN_TIMEOUT`; the bug is that
   "timed out" and "left with nothing saved" are currently indistinguishable.
2. In `crates/jcode-swarm-core/src/control_log.rs`, add a
   `SwarmControlEvent::TurnStashed { session_id, partial_content,
   resume_request }` variant. Keep it shaped like the existing `ArtifactFiled`:
   an event, not an opaque blob.
3. Emit `TurnStashed` from the agent turn loop when it reacts to
   `InterruptSignal::fire()`, before it tears down the in-flight request.
   `reload.rs` should continue to fire the signal; it should not own the turn
   checkpoint logic.
4. On resume, read any `TurnStashed` event for the session and re-issue the
   request instead of silently dropping it. Lean on provider prompt-prefix
   caching for the resend rather than adding a custom deduplication layer.
5. Do not migrate live child processes. Send `SIGTERM` and replay from
   control-log task state on resume. `persist_reload_recovery_intents` already
   recovers session/task role; it needs to resume a stashed turn as well.
   Reparenting live child stdio is out of scope.
6. Keep client behavior simple. Clients already reconnect via `--resume`; the
   missing piece is ensuring that a stashed turn's resume request reaches them.

This is a narrow diff: one changed wait condition, one new control-log event,
one read-back path on resume, and no new supervision surface.

## Stage 2: stable LLM-stream owner

Stage 2 introduces a broker process that owns HTTP/TLS/SSE provider connections
so mid-turn tokens can survive daemon reload. It is effectively a small
PgBouncer/tmux for LLM calls: a long-lived owner, a UDS protocol, crash/restart
handling, and generation handoff.

Do not build it until Stage 1 is shipped and measured:

- If checkpointing at turn boundaries plus prompt-prefix caching recovers most
  reload loss, the broker adds a permanent failure domain for shrinking returns.
- Reload cadence is the deciding number. Infrequent reloads make losing and
  resending the tail of one turn acceptable. The broker is justified only if
  post-Stage-1 loss remains common enough to matter.
- If the broker becomes worth it, reuse the stable-owner/reloadable-client shape
  from `docs/DESKTOP_STABLE_HOST_RELOAD_STARTUP.md`. That document solves UI
  process continuity rather than daemon LLM streams, but the ownership pattern
  is the right template.

## Out of scope

- Permanent blue-green daemon operation. Dual state-locks on the session store
  and control log are too much surface for a cadence-dependent benefit. Revisit
  only if reload frequency approaches session length.
- Manual switchover. It is less reliable than the current automatic reload path.
- CRIU. It is Linux-only, operationally heavy, and still does not solve the
  provider stream ownership problem.

## Next step

Build Stage 1. Measure token loss on reload before and after. Decide on Stage 2
only from those measurements.
