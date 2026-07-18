# Known-good baseline and remaining seams

Recorded: 2026-07-18

## Verdict

The canonical TUI/CLI checkout is suitable for regular feature development.
The promoted runtime remains the exact immutable `8962bccb3-release` binary.
No real-provider request was made during this checkpoint.

This is an honest **core-runtime validated** baseline, not an unqualified claim
that every provider, mobile, WebSocket, packaging, or unattended-swarm path is
closed. The remaining product risks are explicit below.

## Exact baseline

- Canonical checkout: `/Users/jrudnik/labs/jcode`, branch `main`.
- Post-promotion parent: `152ececcc57c153731685ff398352a4494bd679b`.
- Known-good baseline commit: `41e86f3c9f21d942d87161151f9cbe75077b2c6a`.
- Product/runtime commit: `8962bccb32eede3b6746c42bfe6d265df29e4471`.
- Runtime label: `8962bccb3-release`.
- Runtime SHA-256:
  `6cf81221e8c0cee86ae714d2f1fc9fb55fe8715f45ee8082dc2ecf034a2515fc`.
- `current`, `stable`, and `shared-server` select that exact release.
- Self-development manifest has no canary or pending activation.
- Exactly one Git worktree remains. Recovery refs, stashes, bundles, sealed
  evidence, and private archives remain preserved.
- `opencode.json` was removed. `ORCHESTRATOR_PROMPT.md` was restored to its
  tracked baseline and retained because dozens of documentation records
  reference it.

The sealed isolated campaign is at
`~/labs/.recovery/jcode/2026-07-17-runtime-stress/`. Its verdict was 321/321
checks passing in 110.257 seconds, with no provider requests, leaked sandbox
processes or sockets, live-runtime mutation, or build-channel mutation. The
checksum manifest SHA-256 is
`24366cbb5d58c22b5b3ef24ad19b434aa61cb1ddfda763770f823f6fd5c61ae4`.

## Fixed in this checkpoint

| Seam | Resolution | Validation |
|---|---|---|
| Debug command handler errors disconnected the caller | Pre-dispatch job, session-admin, swarm, server-state, and ambient failures now become `DebugResponse { ok: false }`. Transport failures still end the connection. | Focused unit test, 32 debug-server tests, strict `jcode-app-core` clippy, and isolated same-socket error-then-ping live test. |
| Destroying the default session left a stale selection | Active-session removal and deterministic replacement selection are one lock-protected transition. The last removal clears the default. | Unit test plus isolated live create/destroy/state sequence. |
| Self-development status could print `Stable: none` | Status reads the authoritative `stable-version` channel marker instead of stale manifest projection. | Focused status fixture and clean build. |
| Test-only canary and pending-activation residue | The leaking debug selfdev-reload test now uses a temporary `JCODE_HOME`; reproduced live residue was removed and the stable projection normalized. All channel targets and binaries remained unchanged. | Re-ran the test while hashing the live manifest before and after; hashes were identical. Reversible cleanup evidence is in `~/labs/.recovery/jcode/2026-07-18-known-good-baseline/`. |

## Ranked remaining product seams

### 1. Medium-high: server exit does not explicitly reap owned MCP children

**Current behavior.** Persistent idle exit, temporary owner/idle exit, reload,
and the SIGTERM handler call `std::process::exit`. That skips
`McpClient::Drop`, whose only fallback is `child.start_kill()`. A server
`SIGKILL` also cannot run any parent cleanup. The shared pool already has
`disconnect_all`, but server exit paths do not call it. Cooperative stdio
children normally observe pipe EOF and exit, so the practical orphan risk is
concentrated in children that ignore EOF or leave wrapper grandchildren. That
still matches the historical incident class.

**Evidence.** `server/lifecycle.rs:159-255`, `server.rs:1204-1223`,
`jcode-base/src/mcp/client.rs:364-412`, and
`docs/proposals/swarm-lifecycle-remediation.md`. This is the same class as the
historical 231-orphan `mcp-serve` incident. The 321-check campaign proved direct
MCP probes exit and left no sandbox residue, but it did not kill a daemon while
its shared MCP child was still live.

**Bounded fix.** Add explicit child PID ownership and pre-exit reap, plus an
`mcp-serve --owner-pid` self-liveness fallback for `SIGKILL` survival.

**Acceptance gate.** Start a temporary daemon with N mock MCP children, exercise
SIGTERM, idle exit, owner death, and SIGKILL, and assert zero surviving child
PIDs after every case.

### 2. Medium: the 300-second daemon idle decision ignores active headless work

**Current behavior.** `persistent_should_exit` and the temporary monitor inspect
only connected client count. Headless sessions, asynchronous debug jobs, swarm
members, and provider work do not hold an explicit server activity lease. A
clientless persistent daemon can therefore exit after 300 seconds while
detached work is still running. Normal TUI-attached work keeps the client count
nonzero, and private temporary swarm servers default to 1,800 seconds, so the
risk is real but narrower than the ordinary attached path.

**Evidence.** `server/lifecycle.rs:81-95,159-188`, `server.rs:518,1668-1684`.
Existing tests cover only client-count and elapsed-time arithmetic, not active
headless work.

**Bounded fix.** Introduce one server activity/lease authority covering live
client turns, headless turns, debug jobs, background tasks, and MCP-owned work.
Idle exit requires both zero clients and zero active leases.

**Acceptance gate.** With a shortened timeout and a no-provider blocking
fixture, disconnect the last client while headless work runs. The daemon must
stay alive, then exit only after the work lease releases and the idle interval
elapses.

### 3. Medium: a dead pooled MCP child remains advertised as connected

`McpClient::is_running` exists, but pool handle lookup and tool calls do not use
it. If a pooled child dies unexpectedly, the stale handle remains until manual
reload or disconnect and calls can wait for request timeout. Evict dead clients
on lookup/call failure and allow bounded reconnect with cooldown. Validate by
killing a mock MCP child between two calls and asserting fast detection plus one
successful reconnect.

### 4. Low: abandoned self-development activation has no general liveness repair

The stale test residue was removed operationally, and production activation
normally references an installed binary. However, if an initiating session
dies between activation bookkeeping and completion, status can retain a pending
activation indefinitely. Add age/session-liveness reconciliation that never
removes a valid live canary. Validate with a dead initiating session and both
present and missing candidate binaries.

### 5. Low: dormant restored swarm members are metadata, not active sessions

`destroy_session` intentionally administers active agents only. A dormant
reload-restored member now returns a structured `Unknown session_id` response
and leaves the connection healthy. Retention GC is the current removal path.
Keep this behavior documented, or add a separately named metadata-prune command
if operator demand appears. Do not silently widen `destroy_session` semantics.

### 6. Low: observability and socket hygiene

- Malformed persisted swarm files are skipped fail-safe but without a warning.
- Persistent socket `.hash` and some metadata sidecars lack symmetric cleanup.
- Disconnect persistence can be skipped after the bounded agent-lock timeout;
  this is a deliberate deadlock guard, but the last turn is then exposed to a
  coincident hard crash.

These are bounded follow-ups, not blockers for regular feature development.

## Historical or trigger-gated work

- Recovery seam ledgers and `docs/archive/CODE_QUALITY_TODO.md` are historical
  evidence, not an active backlog.
- Expected-red panic, swallowed-error, and file-size ratchets remain normal
  owned debt under `QUALITY_DEBT.md`; touched paths must not grow them.
- R08A broader operator-input semantics reopens only when those command surfaces
  change.
- WebSocket/mobile attach, schema widening, commercial-tier catalog truth, real
  release/update networking, and real-provider turns require their own targeted
  authorization and validation. They do not block TUI/CLI feature work.

## Next engineering order

1. Fix MCP child ownership and server-exit reaping.
2. Add server activity leases and make idle exit work-aware.
3. Add pooled MCP liveness eviction/reconnect.
4. Take the low-severity observability and hygiene items opportunistically when
   adjacent code is touched.
