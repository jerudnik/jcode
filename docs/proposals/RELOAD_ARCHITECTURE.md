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

## Synthesized recommendation (staged)

**Stage 1 — Harden checkpoint-and-resume (do first; ~a weekend; highest value/effort).**
This is Sonnet's core and it's the lazy win: you already have the skeleton
(`await_reload_signal` → interrupt → `persist_intent` → `replace_process` → `--resume`).
Make the intent *byte-meaningful* instead of "please resume":
- Replace the 2s drain timeout with an explicit **per-session checkpoint ack**.
- Add a `TurnStashed { session_id, partial_content, resume_request, tool_cursor }`
  control-log event; on resume, re-issue the LLM request (lean on provider prompt-prefix
  caching so the re-send is cheap, not double-billed).
- Children: **replay from the control log, don't migrate** — `SIGTERM` + respawn from
  checkpointed task state. (Reparenting arbitrary tool stdio is not worth it.)
- Clients: mosh/rust-analyzer-style **reconnect** to the new daemon (the existing
  `--resume`/`JCODE_RESUMING` path, made robust).
This captures the large majority of today's token loss with no new supervision substrate.

**Stage 2 — LLM-stream sidecar (the endgame; do for expensive turns).**
All three converge here: a small, long-lived **stream-broker process** owns the
HTTP/TLS/SSE connection and appends token/tool-call deltas to disk; the daemon
observes/controls it over a UDS. On daemon reload the *new* generation reconnects to the
broker and resumes reading — the mid-turn stream survives because it never lived in the
reloadable process. This is the "PgBouncer/tmux for LLM calls" pattern. Scope it to
high-cost turns first; it's the only path to true zero-loss mid-turn.

**Deferred / rejected as defaults:**
- Full permanent blue-green (Sonnet's caution) — only if reload cadence justifies the
  dual-state complexity. But note GPT's **stable-host + reloadable-worker** long-term shape
  overlaps the existing `DESKTOP_STABLE_HOST_RELOAD_STARTUP.md` plan — worth reconciling
  those two docs before committing to a split.
- Manual switchover — rejected (too cumbersome for frequent reloads).
- CRIU / true process migration — rejected (Linux-only, heavy, can't do TLS sockets).

## Next step
Stage 1 is the actionable one and it's mostly *hardening code that exists*. Before building
it, reconcile this with `DESKTOP_STABLE_HOST_RELOAD_STARTUP.md` (does the fork already want
the stable-host split? if so, Stage 2's broker can live under that host).
