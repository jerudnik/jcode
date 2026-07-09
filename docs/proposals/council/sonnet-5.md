# Zero-loss binary reload for jcode — Sonnet-5 proposal

## Grounding in current code
Today, `await_reload_signal` (reload.rs) already does the right *sequencing* — it fires
`InterruptSignal`s at running sessions, waits up to 2s draining via the swarm-event
broadcast, persists `ReloadRecoveryRole`-tagged intents (Initiator/Headless/InterruptedPeer)
through `reload_recovery::persist_intent`, then calls `platform::replace_process` (`execve`).
On the client side, `hot_reload`/`hot_restart` (src/cli/hot_exec.rs) re-exec with
`--resume <session_id>` and `JCODE_RESUMING=1`. So the skeleton for "checkpoint intent,
exec, resume" already exists — what's missing is that the *intent* captured today is just
"this session needs to resume", not a byte-accurate stream/process handoff. The W1/W2
control log (`control_log.rs`) durably folds swarm membership/task/artifact events and
replays them, so the *task graph* survives; it was never designed to carry live HTTP/SSE
state or child-process handles, and it shouldn't be stretched to.

## Architecture 1 — Blue-green daemon (maintainer seed 1)
New binary starts as daemon-2 on a fresh socket, both daemons run concurrently. Health-checks
before advertising. Clients migrate at their next idle boundary (ACP/TUI clients get a
"switch-when-idle" signal analogous to rust-analyzer's restart+reconnect, where the editor
keeps the old LSP alive until the new one is warm, then swaps and drops the old process).
Headless swarm sessions are the hard case: they don't have a natural idle point mid-turn.
**Preserves**: attached client UX, warm caches/model connections in daemon-2, zero downtime
for idle clients. **Preserves nothing for in-flight**: an LLM stream open on daemon-1 cannot
be hot-migrated to daemon-2 — HTTP/SSE connections are bound to a TCP socket + TLS session +
provider-side request id that daemon-2 has no handle to (unlike Envoy hot restart, which
works because the *listener* fd is shared via SCM_RIGHTS while upstream connections are
independently re-dialable; here the daemon *is* the client of the LLM, so there's no fd to
duck-pass — the socket lives in daemon-1's process). Child tool/MCP processes are worse:
they're subprocesses of daemon-1's PID, so daemon-2 has no ownership path short of re-parenting
via `PR_SET_CHILD_SUBREAPER` + `SO_REUSEPORT`-style fd donation over a UDS (systemd
socket-activation's trick, doable for *listening* sockets, not for arbitrary child stdio pipes).
**Effort/risk**: high — two daemons means two socket paths, a proxy/redirect layer, dual
lock/lease semantics on shared state (session store, control log) if both are ever live
against the same files. Real payoff only if reload cadence is low relative to session length.

## Architecture 2 — Work-stashing / tread-water (maintainer seed 3, recommended core)
Formalize what's implicit today: turn the 2s `graceful_shutdown_sessions` deadline into an
explicit **checkpoint protocol** with a real ack, not a timeout race. On reload signal:
(a) each running agent turn gets an interrupt that causes it to stop *cleanly at the next
token boundary*, snapshotting `(conversation state, partial assistant message so far, tool
calls in flight, retry cursor)` into the control log as a new `SwarmControlEvent` variant —
e.g. `TurnStashed { session_id, partial_content, resume_request }` — analogous to how CRIU
checkpoints process memory, except here it's an *application-level* snapshot (much cheaper
and more portable than CRIU, which can't checkpoint open sockets/TLS state anyway).
(b) child tool processes get SIGSTOP instead of SIGKILL where the tool protocol tolerates
pausing (most MCP servers over stdio can be frozen and later either resumed if the new
daemon re-execs into the *same* PID namespace — impossible across `execve` since exec
preserves PID and open fds! — or, more realistically, cleanly terminated with their
last-known-good output already flushed to the transcript, then **replayed as a fresh
subprocess call after resume**, since most tool invocations are idempotent/reproducible).
(c) attached client sockets: reuse the systemd/nginx pattern — `execve` **inherits open file
descriptors by default** in POSIX; jcode already relies on this implicitly for `--socket`, but
should explicitly `SO_REUSEADDR`+pass the *listening* UDS fd across exec (already true — the
new process binds the same socket path) while existing *accepted* client connections on the
old process die at exec and must reconnect (ACP/TUI clients already need a reconnect loop —
mosh/tmux detach-reattach model: client buffers pending input, redraws from a resync message
after reconnecting, rather than assuming the pipe survives).
**Preserves**: LLM turn *content* generated so far (not the live stream itself — the stream
dies, but its output up to the last flushed token is not lost), full task graph (already
true today via W1/W2), tool-call intent (replay, not migrate).
**Hard parts, honestly**: the *live* SSE connection to Anthropic/OpenAI cannot be moved
process-to-process — no OSS system does this for outbound HTTP client streams; the closest
analogy (PgBouncer) works because Postgres' wire protocol lets a *proxy* sit between client
and server indefinitely, decoupling the two ends' lifecycles. jcode has no such proxy today.
Two options: (i) accept the stream is severed, resume the turn as a *new* API call seeded
with the partial output + "continue" framing (cheap, some token waste, no new infra); or
(ii) introduce a thin **LLM-call sidecar process** (long-lived, outlives daemon reloads,
holds the actual HTTP connections, talks to the daemon over a UDS) — this is the only way
to truly survive reload with zero LLM-side interruption, at the cost of another process
boundary and a small latency tax per token relayed through it.
**Effort/risk**: medium for (i), the sidecar is a bigger lift but is the "fuller version."

## Architecture 3 — Proxy-fronted daemon (Envoy/HAProxy-style, hybrid)
A cheap, always-on **connection-broker** process (like Envoy's hot-restart parent or
HAProxy's master process) owns the listening UDS and all outbound LLM HTTP clients; the
"daemon" behind it becomes disposable and stateless-ish, restarted freely because the broker
buffers in-flight SSE chunks and re-attaches them to whichever daemon generation is current.
This subsumes both blue-green and the sidecar idea from Architecture 2(ii) into one
long-lived component. **Preserves** almost everything, including live streams, at the cost
of building and hardening a new always-on component that becomes a new single point of
failure — arguably not worth it unless reload frequency is extreme (which selfdev dogfooding
suggests it might be).

## Recommendation (staged)
**Stage 1 (cheap, ship now)**: Harden the existing tread-water path. Replace the 2s fixed
timeout with a real per-session ack (`TurnStashed`/`ToolPending` control-log events), extend
`ReloadRecoveryRole` semantics so *every* interrupted turn — not just headless — gets a
structured resume directive with partial content preserved, and standardize the "replay,
don't migrate" contract for child tool processes (SIGTERM with grace, log last output,
mark task `TaskStatusChanged{status: "interrupted"}` for W2 replay to retry). Client sockets
already survive via exec + inherited listening fd; formalize client-side reconnect-and-resync
(mosh-style) instead of relying on clients tolerating a hard disconnect.
**Stage 2 (fuller)**: Build the LLM-call sidecar (Architecture 2ii / Architecture 3's
outbound half) so mid-turn streams survive reload without resend cost — this is the one
piece of "moving live state across a process boundary" that's actually worth solving
properly, since it's the highest-value, most-frequently-severed resource. Leave child
subprocess and client-socket handling on the cheaper replay/reconnect model indefinitely —
migrating those is high effort for low marginal value once turn continuity is solved.
