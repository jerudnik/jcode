# Zero-loss daemon reload — design synthesis

> Synthesis of a 3-model design council (GLM-5.2, GPT-5.5, Sonnet-5), each proposing
> independently from the same brief. Raw proposals are archived alongside this doc.
> Goal: reload the daemon binary (selfdev hot-update, auth/config/logging changes)
> without losing in-flight work — running LLM turns, subagent processes, swarm progress.

## The one thing all three agree on (the hard constraint)

A live LLM stream (TCP + TLS + HTTP/SSE parser state + provider-side request id) and an
arbitrary child OS process **cannot be portably transplanted** into a new `execve`'d
process. `SCM_RIGHTS` can pass a *listening* socket fd (that's the nginx/Envoy/systemd
trick), but the daemon is the *client* of the LLM — there's no listener to duck-pass, and
the TLS/HTTP state lives in library memory. CRIU can checkpoint on Linux but is
Linux-only, heavy, and wrong as a default. **Conclusion: you don't move live state — you
either drain it before the swap or checkpoint-and-resume it. The only way to survive a
*mid-turn* stream is to keep it in a process that doesn't reload.**

What already survives: the fork's **W1/W2 control log** durably replays swarm
membership/task/artifact state. What's lost today: mid-turn LLM streams, running child
processes, live client UI state, and any in-memory work not yet in the control log.

## Where the council split — the cheap Stage-1

| Model | Stage-1 proposal | Trade |
|---|---|---|
| **Sonnet-5** | Formalize the existing checkpoint: replace the 2s `graceful_shutdown_sessions` timeout race with a real per-session **ack**, snapshot `(partial assistant msg, tool calls in flight, resume cursor)` into a new `TurnStashed` control-log event, `SIGSTOP`/replay children. | Smallest change, extends W1/W2, no new processes. Doesn't help clients stay connected. |
| **GLM-5.2** | Drop `execve`; spawn successor, pass client UDS sockets via `SCM_RIGHTS` so **clients never disconnect**, drain LLM turns on a timer, respawn children. | Clients stay attached; still drops mid-turn streams on timeout. |
| **GPT-5.5** | **Blue-green graceful drain**: gen N+1 starts, N stops taking new work and drains, clients migrate at idle, N exits on drain/timeout (nginx/Envoy semantics — connections finish in the old process). | Cheapest to reach zero *new* loss; two daemons need generation routing + no-pile-up. |

Sonnet explicitly **rejects full blue-green** as the default (dual state-locks on the
session store/control log, two socket paths) unless reload cadence is low vs session
length. GPT and GLM accept a second process but only transiently (drain, then exit).

## Recommendation: do Stage 1, stop there until it's measured insufficient

**Stage 1 — harden checkpoint-and-resume.** The only stage worth building right now.
It's a weekend of hardening code that already exists (`await_reload_signal` →
`graceful_shutdown_sessions` → `persist_reload_recovery_intents` → `replace_process` →
`--resume`/`JCODE_RESUMING`). Exact diff:

1. `crates/jcode-app-core/src/server/reload.rs`,
   `graceful_shutdown_sessions_with_timeout`: today the wait loop treats any
   `SwarmEventType::StatusChange` or `MemberChange{action:"left"}` as "this session is
   handled, stop waiting" — a bare liveness flip, not proof anything was saved. Change
   what it waits for: a session only counts as handled once it completed normally or
   appended the checkpoint event below — not merely because its status changed. Leave
   the 2s `RELOAD_GRACEFUL_SHUTDOWN_TIMEOUT` bound alone; a hard cutoff is fine. The bug
   is that "timed out" and "left with nothing saved" are currently indistinguishable.
2. `crates/jcode-swarm-core/src/control_log.rs`: add one `SwarmControlEvent` variant —
   `TurnStashed { session_id, partial_content, resume_request }` (same shape discipline
   as the existing `ArtifactFiled`: an event, not a blob). Emit it from wherever the
   agent turn loop reacts to `InterruptSignal::fire()`, before it tears down the
   in-flight request — `reload.rs` only fires the signal, it doesn't consume it.
3. On resume (`src/cli/hot_exec.rs`, unchanged), read back any `TurnStashed` event for
   the session and re-issue the request instead of silently dropping it. Lean on
   provider prompt-prefix caching for the resend; don't build a custom dedup layer.
4. Children: don't migrate them. `SIGTERM` + replay from control-log task state on
   resume — `persist_reload_recovery_intents` already recovers session/task role, it
   just doesn't resume a stashed turn yet. Reparenting live child stdio is out of scope.
5. Clients: already reconnect via `--resume`. No change needed beyond making sure a
   stashed turn's resume request actually reaches them.

That's the whole diff: one changed wait condition, one new control-log event, one
read-back on resume. No new process, no new supervision surface.

**Stage 2 — LLM-stream sidecar: don't build this yet.** All three models converge on a
broker process that owns the HTTP/TLS/SSE connection so mid-turn tokens survive a
reload (PgBouncer/tmux for LLM calls). That's real infrastructure — a new long-lived
process, a UDS protocol, its own crash/restart handling, generation handoff. It's YAGNI
until Stage 1 is shipped and measured:
- If checkpointing at turn boundaries plus prompt-prefix caching already recovers most
  of the loss, the sidecar buys shrinking returns for a permanent new failure domain.
- Reload cadence is the deciding number. Infrequent reloads (selfdev builds, occasional
  config changes) make "lose the tail of one turn, resend it" a non-problem. Only build
  the broker if measured post-Stage-1 loss is still real.
- If it does become worth it: reuse the host/worker split already designed for desktop
  UI reload (`docs/DESKTOP_STABLE_HOST_RELOAD_STARTUP.md`) rather than inventing a
  second supervision pattern — that doc solves a different problem (OS window/renderer
  continuity across a UI binary reload), not this one, but the shape (stable owner +
  reloadable client) is the same template. Reuse it, don't re-propose it.

**Not doing:**
- Full permanent blue-green — dual state-locks on the session store/control log for a
  cadence-dependent benefit. Revisit only if reload frequency approaches session length.
- Manual switchover — strictly worse than what exists today.
- CRIU — Linux-only, heavy, and doesn't touch the actual TLS/HTTP problem anyway.

## Next step
Build Stage 1. Measure token loss on reload before/after. Only then decide whether
Stage 2 is worth its supervision cost.
