# F01 revision response: mapping every review finding to a correction

Review: `docs/fork/ideal-base/reviews/F01-architecture-critique.md`, commit
`7563a1237`, reviewer OpenAI `gpt-5.6-sol` at high effort. Verdict FAIL with
blocking findings B1-B3, important findings I1-I5, and a ten-point revision
gate.

Revised design: `design.md` (revision 4, this directory). Source facts
re-verified at commit `398b51c07d1f0545bfdccd6a33e6ea9fd76b6574`.

NOTE (historical layering): this file is cumulative. The "Blocking findings"
and "Important findings" sections below record the revision-2 responses to
the ORIGINAL review and are historical; where later rounds refined them
(deadline rule wording, executor termination ownership, `Exited` ->
`Cleaned`), the later sections and the current `design.md` govern. Each
subsequent round appends its own section.

Inputs beyond the review itself: seven typed worker artifacts preserved under
`../F01-R/worker-artifacts/` (nodes `b1`, `i1`, `i2`,
`F01-R-watchdog-review-lines`, `F01-R-source-seam`, `F01-R-entry-families`,
`F01-R-reloadhandoff`), each produced read-only against the same tree by
OpenAI-routed workers per D009/D009a.

## Blocking findings

### B1 (authority placement cannot cover production MCP calls) -> RESOLVED

Review requirement (`F01-architecture-critique.md:84-89`): an implementable
inversion seam plus expanded MCP ownership, covering pooled AND non-shared
calls.

Correction (design section 3.0): the lease interface moves to the neutral
bottom crate `jcode-core` as `crates/jcode-core/src/activity.rs`.
Verified dependency directions at `398b51c07`:

- `crates/jcode-base/Cargo.toml:104` — `jcode-base` depends on `jcode-core`;
- `crates/jcode-app-core/Cargo.toml:88-89` — `jcode-app-core` depends on both;
- `jcode-core` depends on neither (zero matches for `jcode-app-core` or
  `jcode-base` in `crates/jcode-core/Cargo.toml`).

So `jcode-base` MCP/background code can acquire leases through the
`ActivityLeaseAuthority` trait object injected at the composition root, with no
dependency cycle. The concrete authority stays in app-core
(`ServerActivityLeaseAuthority`).

C7 is re-censused as two acquisition surfaces, both in `jcode-base`:

- `McpManager::call_tool` (`crates/jcode-base/src/mcp/manager.rs:342`) — one
  guard at entry covers the pooled fast path (`manager.rs:348-355`), the
  owned per-session path (`manager.rs:357-364`), and connect-on-first-call
  retries (`manager.rs:367-388`), reached from the registered tool at
  `crates/jcode-base/src/mcp/tool.rs:49-58`;
- `SharedMcpPool::call_tool` (`crates/jcode-base/src/mcp/pool.rs:232`) —
  wrapped as well so direct pool callers are covered.

F02 ownership is amended in `WORK_GRAPH.json` (both `all_nodes` and the W1
expansion) to add `crates/jcode-core/src/activity.rs`,
`crates/jcode-core/src/lib.rs`, `crates/jcode-base/src/mcp/manager.rs`,
`crates/jcode-base/src/mcp/pool.rs`, and
`crates/jcode-app-core/src/tool/mod.rs` (constructor injection at
`tool/mod.rs:758-778`). Tests must cover pooled and non-shared MCP calls
separately (design 4.3).

### B2 (no complete concurrent coordinator protocol; reload self-block) -> RESOLVED

Corrections (design sections 3.2, 3.2.1-3.2.4):

- `ReloadHandoff` is REMOVED from the lease table. Reload is coordinator phase
  state, never a drain-blocking lease, adopting the `F01-R-reloadhandoff`
  artifact recommendation. The self-wait cycle cannot exist because the
  coordinator never waits on its own state.
- One serialized executor: a single actor task owns all phase transitions;
  callers hold a cloneable `ShutdownHandle` with `begin(reason) ->
  BeginOutcome` and `begin_and_wait(reason) -> TerminalOutcome`.
- Total reason priority lattice (design 3.2.2), covering all seven voluntary
  reasons, not just `SIGTERM > idle`.
- Exact deadline-update rule: an accepted stronger reason re-derives the drain
  deadline as `min(remaining, full_deadline(new_reason))` — upgrades can only
  shorten or preserve, never extend a running drain.
- Exactly-once cleanup and termination: the executor runs the cleanup list at
  most once behind a phase latch, and the process ends at exactly one
  `terminate(code)` call site inside the executor (plus the coordinator-armed
  watchdog, see I2).
- Pairwise reason-race tests for every ordered pair of reasons are required by
  design 4.3, not just random transitions.

### B3 (provider/headless wiring misses the actual turn boundary) -> RESOLVED

Corrections (design sections 3.3, 3.3.1):

- The `ProviderTurn` guard is acquired inside
  `process_message_streaming_mpsc` itself
  (`crates/jcode-app-core/src/server/client_lifecycle.rs:3179`), at the top of
  the future, so every caller family is covered by construction. Verified
  caller enumeration at `398b51c07`: client message tasks
  (`client_lifecycle.rs:2861`), client actions (`client_actions.rs:1101`),
  swarm assignment (`comm_control.rs:991`), spawned/headless initial turns
  (`comm_session.rs:886`), Jade relay (`jade_relay.rs:1211`, `jade_relay.rs:1242`),
  live wake turns (`live_turn.rs:120`). A wiring-census test greps for callers
  and fails when a new caller family appears without a fixture.
- `HeadlessTurn` as a session-existence lease is REMOVED. Headless/restored
  turns take `ProviderTurn` at the same common boundary. Startup restoration
  gets its own bounded `StartupRecovery` lease class acquired in
  `recover_headless_sessions_on_startup`
  (`crates/jcode-app-core/src/server.rs:721`) for the enumeration/scheduling
  window only, with a hard TTL (design 3.3.1), adopting the
  `F01-R-entry-families` artifact. Session existence is never activity.

## Important findings

### I1 (idle window semantics) -> RESOLVED

Design 4.2 defines one quiescence epoch: `idle_since: Option<Tick>` owned by
the lifecycle model. It is `None` whenever `clients > 0` or any drain-blocking
lease is held, and is set only on the transition into full quiescence. Idle
exit requires the system to still be quiescent and
`now - idle_since >= idle_timeout`. `should_exit` consumes `idle_since`, not an
externally integrated elapsed value. Required pure test: acquire, hold past
timeout, release, assert a full new window is required (design 4.3).

### I2 (watchdog contradicts single authority) -> RESOLVED

Design 3.2.4: the watchdog is an implementation detail owned, armed, re-armed,
and disarmed exclusively by the coordinator executor at `begin()` acceptance.
The independent SIGTERM-handler thread (`server.rs:1213-1219`) is removed by
F02; the handler's only action becomes `begin(SigTerm)`. Forced exit is a
distinct terminal outcome `ForcedExit` with its own exit code, never reported
as a successful `Exited`. Budgets satisfy
`drain_deadline(reason) + cleanup_budget < watchdog_deadline(reason)` by
construction, and the forced-path residue contract (design 3.2.4) is asserted
by an F03 fixture rather than excused. This satisfies review lines 195-213 as
identified by the `F01-R-watchdog-review-lines` artifact.

### I3 (nonexistent cleanup API; incomplete residue set) -> RESOLVED

Design 3.4 names only APIs verified to exist at `398b51c07` —
`registry::unregister_server_bounded` (`lifecycle.rs:177`),
`transport::remove_socket` (`server.rs:2109-2110`), the `.hash` sidecar path
(`server.rs:1691-1692`), temp metadata removal (`lifecycle.rs:151`), and
pool-level `SharedMcpPool::disconnect_all` (`pool.rs:145-164`) — or declares
the one new API F02 must add within its owned paths:
`BackgroundTaskManager::finalize_non_detached(reason)` in
`crates/jcode-base/src/background.rs` (owned by F02 already). The residue set
(design 3.4.1) now includes the daemon lock (`DaemonLockGuard`,
`socket.rs:160-199`, held at `server.rs:2099`) and the registry entry, with
explicit guard-lifetime ordering rules for direct-exit, forced-exit, and exec
paths.

### I4 (temporary servers can reload) -> RESOLVED

Verified: `await_reload_signal` is wired unconditionally
(`server.rs:1195`) before the temporary-vs-persistent choice
(`server.rs:1668-1685`). Design 3.2.3: a coordinator constructed in temporary
mode refuses `begin(Reload)` with the typed outcome
`BeginOutcome::Refused(RefusalReason::TemporaryServerNoReload)`; the reload
path reports the refusal to the requester. The coverage matrix row is `refuse`
(typed, tested), no longer `n/a`.

### I5 (accept-loop failure must await completion) -> RESOLVED

Design 3.2.3 and 3.5: the accept-loop failure arms in `Server::run`
(`server.rs:2181-2196`) call `begin_and_wait(AcceptLoopFailure)` and only then
return a distinct `Err`, mapping to its own nonzero exit code (design 3.2.5).
The `_daemon_lock` guard (`server.rs:2099`) lives until after the awaited
terminal outcome by function scoping, so lock release is ordered after
cleanup.

## Coverage-audit line items

- C1 duplication: resolved — `client_count` is replaced by the count of
  `ClientConnection` leases; one source of truth (design 3.1).
- C8 background persisted waits pinning the daemon: policy made explicit
  (design 3.3.2): live waiter tasks hold `SwarmWaiter` leases; a background
  persisted await whose watcher is parked holds NO lease because its durable
  state (`await_members_state.rs:24`) survives restart.
- C9: the acquire/release boundary is defined as the delivery dispatch in the
  ambient runner, ending when the spawned turn has acquired its own
  `ProviderTurn` lease (design 3.3.2).
- C10: removed from the lease table entirely (B2).

## Revision-gate checklist

| Gate item | Where resolved |
|---|---|
| 1. crate-safe shared activity interface + F02 ownership | design 3.0; WORK_GRAPH amendment |
| 2. pooled and non-shared MCP calls | design 3.0.1 |
| 3. provider-turn leasing at the common boundary | design 3.3 |
| 4. separate headless existence / recovery / turns | design 3.3.1 |
| 5. serialized executor, precedence, deadlines, awaitable completion | design 3.2.1-3.2.3 |
| 6. ReloadHandoff self-lease removed | design 3.2 (phase state only) |
| 7. continuous quiescence epoch | design 4.2 |
| 8. watchdog reconciled with I2/I5/I7/A0 | design 3.2.4 |
| 9. real cleanup APIs and complete residue set | design 3.4, 3.4.1 |
| 10. temporary reload disposition + pairwise races + per-entry fixtures | design 3.2.3, 4.3 |

---

## Revision 3: response to the re-review FAIL (`F01-architecture-re-review.md`, commit `09f367098`)

Re-reviewer: OpenAI `gpt-5.6-sol`, high effort. Verdict FAIL with two
blockers (B-R1, B-R2), two important findings (I-R1, I-R2), two minor
(M-R1, M-R2).

### B-R1 (caller census omits startup reload-recovery) -> RESOLVED

Verified: `crates/jcode-app-core/src/server.rs:1009-1016` calls
`process_message_streaming_mpsc` directly inside
`recover_headless_sessions_on_startup`. The caller-family table (design 3.3)
now includes it as its own family, with its own required F03 fixture, and the
wiring-census test is specified to scan production call sites only (excluding
definitions, imports, tests) so this class of omission fails loudly.

### B-R2 (awaitable completion vs executor-owned termination) -> RESOLVED

Adopted the re-review's first suggested protocol (design 3.2.1):
`TerminalOutcome::Exited` is renamed `Cleaned { reason, code }` and denotes
"cleanup fully ran, process not yet exited". The executor never calls
`std::process::exit`. `Server::run` awaits the coordinator terminal state,
returns typed exit information, and its resource guards (daemon lock) drop on
scope unwind after cleanup. The top-level runner — the sole `Server::run`
caller at `src/cli/dispatch.rs:114` — performs the one normal
process-termination call. Exactly two authorized termination sites exist:
the top-level runner (normal) and the coordinator-armed watchdog
(`ForcedExit`). This preserves exactly-once termination, lock ordering, and
lets accept-loop failure await `Cleaned` and still return `Err`.

### I-R1 (background acquisition branches) -> RESOLVED

The C5 wiring row (design 3.3.3) now cites the real boundaries: guard
acquired at method entry before the `tokio::spawn` at `background.rs:483-484`
(and before the adopt-path registration), moved into the `RunningTask` record
inserted at `background.rs:584-600`, dropped at terminal pruning. The guard
exists before the future can run and lives exactly as long as the tracked
task.

### I-R2 (termination-site ambiguity) -> RESOLVED

Design 3.2.1 enumerates the two authorized termination sites explicitly;
the "exactly one call site inside the executor" claim is removed.

### M-R1 (socket.rs:71 citations) -> RESOLVED

Both citations now name `reap_stale_socket_if_dead` at `socket.rs:76-126`.

### M-R2 (deadline wording) -> RESOLVED

The upgrade rule is stated over absolute deadlines:
`new_deadline = min(current_absolute_deadline, now + full_budget(new_reason))`.

---

## Revision 4: response to the round-2 FAIL (`F01-architecture-re-review.md` Round 2, commit `6e1c59f34`)

Re-reviewer: OpenAI `gpt-5.6-sol`, high effort. Verdict FAIL with blockers
R2-B1, R2-B2, important R2-I1, R2-I2, minor R2-M1.

### R2-B1 (watchdog can fire after `Cleaned`) -> RESOLVED

Design 3.2.4 now has an explicit disarm rule with exactly two
executor-performed disarm cases: `Handoff` exec success, and after the last
cleanup step BEFORE `Cleaned` is published. Disarm is an atomic CAS on a
two-state `Armed/Cancelled` cell that the watchdog thread checks before its
exit call; if the CAS loses, `Cleaned` is never published and the outcome is
`ForcedExit`. The two terminal outcomes are mutually exclusive by the atomic
handoff, restoring outcome honesty and exactly-once termination. The pure
test plan (4.3) drives the cleanup-completion / watchdog-deadline /
waiter-notification / guard-unwind / runner-exit race.

### R2-B2 (F02 does not own the termination site) -> RESOLVED

`src/cli/dispatch.rs` is added to both F02 `owned_paths` arrays in
`WORK_GRAPH.json`, and design 3.2.1 records the ownership consequence,
including that F06 (which also lists the file) depends on F02, so ownership
is sequential rather than concurrent, and that F02 acceptance evidence must
cover the outcome-to-code mapping and accept-loop code 45 at this site.

### R2-I1 (stale executor-termination prose) -> RESOLVED

Design 3.2.1 intro now says the actor performs the final
terminal-publication transition and does not itself exit the process. The
lint rule (3.2 intro) now permits exactly the two authorized termination
sites and rejects every other daemon-image exit site.

### R2-I2 (C5 guard-lifetime overstatement) -> RESOLVED

The C5 wiring row distinguishes the two branches honestly: `spawn_with_notify`
acquires before the spawn at `background.rs:483-484`; the adopt path
(`background.rs:628-686`) acquires at adoption, with pre-adoption execution
covered by the foreground owner's lease. Pruning boundaries are cited
(`background.rs:551-552`, adopt `:754-758`), and post-pruning wrapper work is
explicitly handed off to the scheduled-delivery/turn policy rather than
claimed as covered.

### R2-M1 (superseded revision-2 claims unmarked) -> RESOLVED

This file now carries a historical-layering note in its header; the current
`design.md` governs wherever earlier sections used superseded terminology.
