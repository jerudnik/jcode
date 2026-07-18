# F01 design record: one shutdown coordinator, one activity-lease authority

Status: design only, revision 4. Revision 1 verified against
`c96c4b57de57438d63e23796e6b038027265fca4`; revisions 2-3 re-verified against
`398b51c07d1f0545bfdccd6a33e6ea9fd76b6574` (main). No source modified.
Revision 2 resolved the independent FAIL review
(`../../reviews/F01-architecture-critique.md`, commit `7563a1237`); the
finding-by-finding mapping is in `revision_response.md`. Revision 3 resolves
the re-review FAIL (`../../reviews/F01-architecture-re-review.md`, commit
`09f367098`): B-R1 (startup reload-recovery caller family added, section
3.3), B-R2 (termination ownership protocol, section 3.2.1), I-R1 (background
guard scope), I-R2 (two authorized termination sites enumerated), M-R1/M-R2
(citation and deadline-wording precision). Revision 4 resolves the round-2
FAIL (same review file, Round 2 section, commit `6e1c59f34`): R2-B1
(watchdog disarm-before-`Cleaned` atomic handoff, 3.2.4), R2-B2
(`src/cli/dispatch.rs` added to F02 ownership, 3.2.1), R2-I1 (executor
prose and lint rule), R2-I2 (honest C5 adopt/pruning boundaries).

Scope contract (A0/A1 of `ACCEPTANCE_STANDARD.md`):

- A0: every normal daemon exit path invokes ONE bounded shutdown authority;
  sidecars end live-and-coherent or fully removed.
- A1: idle exit requires zero clients AND zero active work leases; provider
  turns, headless/restored turns, debug jobs, background jobs, MCP calls, and
  swarm waiters are covered by the lease authority.

---

## 1. Source ownership census: who can exit the daemon today

Every process-exit authority in the daemon image, at the commit above:

| # | Exit authority | Location | Trigger | What it cleans up | What it ignores |
|---|----------------|----------|---------|-------------------|-----------------|
| E1 | Persistent idle monitor | `crates/jcode-app-core/src/server/lifecycle.rs:159-188`, decision at `lifecycle.rs:90-96` (`persistent_should_exit`), `std::process::exit(EXIT_IDLE_TIMEOUT)` at `lifecycle.rs:178` | `client_count == 0` for `IDLE_TIMEOUT_SECS` (300, `server.rs:518`), polled every 10s | Registry unregister only (`lifecycle.rs:177`) | Sockets, `.hash` sidecar (`server.rs:1691-1692`), all active work classes, MCP children |
| E2 | Temporary lifecycle monitor | `lifecycle.rs:190-244`, exit via `shutdown_temporary_server` at `lifecycle.rs:246-256` (`exit` at :255) | Owner PID dead (`lifecycle.rs:204-213`, `process_alive` at :270) or idle >= policy timeout | Registry, both sockets, temp metadata (`lifecycle.rs:251-254`) | `.hash` sidecar, all active work classes, MCP children |
| E3 | SIGTERM handler | `server.rs:1204-1226`, `exit(0)` at `server.rs:1222`; 3s watchdog thread `exit(0)` at `server.rs:1215-1219` | SIGTERM | Registry unregister only (`server.rs:1221`) | Sockets, metadata, hash, all work classes; watchdog can preempt even the registry cleanup |
| E4 | Exec-based reload | `crates/jcode-app-core/src/server/reload.rs:57-211`; `replace_process` around `reload.rs:180`; failure fallback `std::process::exit(42)` at `reload.rs:210` | Reload signal channel (`await_reload_signal`, wired at `server.rs:1187-1202`) | Persists recovery intents (`reload.rs:118`, `persist_reload_recovery_intents` at :214), graceful-shutdowns sessions (`reload.rs:130`, `graceful_shutdown_sessions` at :334), removes sockets pre-exec (`reload.rs:19`), marks listener FDs close-on-exec (`server.rs:2115-2121`) | Non-session work classes (debug jobs, MCP in-flight calls, swarm waiters are killed by exec without lease drain); exit(42) fallback path skips socket cleanup it already did but leaves no registry/metadata reconciliation |
| E5 | Accept-loop failure return | `server.rs:2181-2197`: either listener exiting cancels the runtime task scope (`runtime.rs:303-305` `tasks.shutdown()`) and returns `Ok(())` from `run()` | Listener error | Joins owned connection tasks | No socket/registry/metadata removal on this path; process exit code is the caller's |
| E6 | Parent SIGKILL / crash | (no code: involuntary) | External | Nothing | Everything; next boot relies on stale-socket reap (`reap_stale_socket_if_dead`, `socket.rs:76-126`), background orphan reconcile (`server.rs:1171-1186`), reload-marker stale clear (`server.rs:2126`), reload-recovery GC (`server.rs:2128-2139`), PID-marker sweep (`jcode-base/src/session.rs:66`) |

Confirmed absences (grounds for this design):

- `persistent_should_exit(client_count, idle_elapsed_secs, idle_timeout_secs)`
  at `lifecycle.rs:90-96` consults NOTHING but client count and idle clock.
- No `lease`/`Lease` abstraction exists anywhere under
  `crates/jcode-app-core/src/server*` or `crates/jcode-core/src/`.
- The five voluntary exits (E1-E5) each run their own ad-hoc cleanup subset;
  no shared shutdown function exists.

### 1.1 Work-class census: everything that is "active work" while clients may be zero

`client_count` is maintained solely by
`runtime.rs:307-315` (`increment_client_count`) / `runtime.rs:317-325`
(`decrement_client_count`), driven by the main accept loop
(`runtime.rs:164/169`), the gateway accept loop (`runtime.rs:230/238`), and
stream teardown (`runtime.rs:373`). Debug clients are explicitly excluded
(`runtime.rs:199`). Every class below can therefore be mid-work at
`client_count == 0` and be killed by E1 today:

| ID | Work class | Where it runs | Present accounting | Killable-at-idle evidence |
|----|-----------|----------------|--------------------|---------------------------|
| C1 | Client-initiated provider turns | `client_lifecycle.rs` message loop (per-connection); per-session agent mutex | Indirect: the owning connection holds a client count while attached | If the client disconnects mid-turn, the turn task may continue with `client_count` already decremented (`runtime.rs:373` decrements on stream end, not turn end) |
| C2 | Server-initiated ("wake") live turns | `live_turn.rs:93-172` `spawn_tracked_live_turn` (raw `tokio::spawn` at :115); callers: `background_tasks.rs:57,136`, `client_comm_message.rs:296`, `client_actions.rs:107,980` | None. Not in `client_count`, not in the runtime task scope | A wake turn on an otherwise idle daemon does not reset the idle clock |
| C3 | Headless / restored sessions and their turns | `headless.rs:38` `create_headless_session` (drain task spawned at `headless.rs:210`); startup restore `server.rs:721` `recover_headless_sessions_on_startup` | None. Headless members have no client connection by definition | The recovery path itself runs post-startup with zero clients; a long restored turn outlasting 300s idle is killed by E1 |
| C4 | Debug jobs | `debug_jobs.rs:72` `maybe_start_async_debug_job` (raw `tokio::spawn` at :92 and :121) | None. Debug connections deliberately do not count (`runtime.rs:199`) | A debug job started from a debug socket on an idle daemon has zero protection |
| C5 | Background tasks (non-detached) | `crates/jcode-base/src/background.rs` `BackgroundTaskManager` (`background.rs:36`); handles aborted on cancel (`background.rs:1126`) | Observable (`server.rs:289` samples count for memory logs) but not consulted by any exit decision | Orphan reconcile at `server.rs:1171-1186` exists precisely because these die with the process and leave `Running` status files |
| C6 | Background tasks (detached) | `background.rs:357` `register_detached_task`; own process group; survive reload (`background.rs:1172-1190`) | By design excluded from daemon lifetime | Not a lease holder: they must NOT keep the daemon alive; they only need status reconciliation |
| C7 | MCP calls in flight (pooled AND per-session owned) | `McpManager::call_tool` (`crates/jcode-base/src/mcp/manager.rs:342`): pooled fast path `manager.rs:348-355`, owned per-session path `manager.rs:357-364`, connect-on-first-call retries `manager.rs:367-388`; reached from the registered tool at `mcp/tool.rs:49-58`; direct pool surface `SharedMcpPool::call_tool` (`mcp/pool.rs:232`); children owned per `client.rs:171` with `OwnedChildPermit` (`client.rs:39`) | None at daemon level | E1/E3 exit mid-call kills the child via process death but never drains the call; exec reload (E4) leaks in-flight call state entirely. Both pooled and owned paths live in `jcode-base`, below the server crate |
| C8 | Swarm await waiters | `comm_await.rs:349` `spawn_or_resume_await_members` (raw `tokio::spawn` at :364); persisted (`await_members_state.rs:24` `PersistedAwaitMembersState`, background resume at `comm_await.rs:830` from `server.rs:1264`) | Durable state exists; live waiter is an untracked task | Background awaits are auto-resumed after reload, so E4 is survivable BY PERSISTENCE, but a foreground await on an idle daemon is killed by E1 with no persisted final response |
| C9 | Scheduled/ambient delivery loop | `AmbientRunnerHandle` initialized at `server.rs:617-624`; nudged on client disconnect (`runtime.rs:375-377`) | None | A due scheduled task delivering into C2 gets no lease today |

Note (revision 2): the former C10 "reload handoff" work class is deliberately
removed from the lease census. Reload is coordinator phase state, not
drain-blocking work (section 3.2). Treating it as a lease created the
self-block cycle identified by review finding B2.

Deliberate non-lease classes (must be documented, not leased):

- C6 detached background tasks (outlive the daemon by design).
- Debug *connections* (readonly inspection must never pin the daemon; only
  debug *jobs*, C4, hold leases).
- Headless session *existence* (revision 2): a registered idle headless
  session holds no lease. Its turns hold `ProviderTurn` leases; its restart
  durability comes from session persistence, not from pinning the daemon.
- Background persisted awaits whose live watcher is parked: durable state
  (`await_members_state.rs:24`) survives restart, so no lease (section 3.3.2).
- Registry metadata publisher, memory samplers, embedding preload, index
  warmup (`server.rs:1117-1170`): best-effort startup tasks, abandonable.

---

## 2. Exit-reason taxonomy

Every way the daemon process ends, normal or not:

| Reason | Class | Initiator | Required guarantees |
|--------|-------|-----------|---------------------|
| `SigTerm` | normal, external | OS/user/supervisor | Bounded: drain-or-abandon within grace; registry + sockets + metadata removed; exit 0 |
| `PersistentIdle` | normal, internal | idle monitor | Only after a continuous quiescence epoch (zero clients AND zero leases for the full idle window, section 4.2); full sidecar cleanup; exit `EXIT_IDLE_TIMEOUT` (44, `server.rs:527`) |
| `TemporaryIdle` | normal, internal | temp monitor | Same as `PersistentIdle` plus temp metadata removal (`lifecycle.rs:151`); exit 44 (`lifecycle.rs:12`) |
| `TemporaryOwnerExit` | normal, internal | temp monitor observing dead owner PID (`lifecycle.rs:204-213`) | Leases get a bounded drain (owner is gone; work is abandoned after grace), then same cleanup as `TemporaryIdle` |
| `Reload` | normal, internal | reload signal (`reload.rs:57`) | NOT a cleanup exit: persists recovery intents, drains/interrupts sessions, removes sockets, exec-replaces image; refused with a typed outcome on temporary servers (section 3.2.3) |
| `ReloadExecFailed` | abnormal-but-voluntary | reload path fallback (`reload.rs:210`, exit 42) | Must record failed phase (`ReloadPhase::Failed`, done today) AND perform the same sidecar cleanup as a normal exit (missing today) |
| `AcceptLoopFailure` | abnormal-but-voluntary | listener error (`server.rs:2181-2197`) | Cancel+join owned tasks (exists via `RuntimeTaskScope`), then bounded cleanup, then `run()` returns a distinct error only after the coordinator reaches a terminal outcome (section 3.2.3) |
| `ParentSigkill` / crash / OOM | involuntary | external | Cannot run code. Guarantee shifts to (a) next-boot reconciliation (already partially present, census E6) and (b) children not outliving the parent (A0 owner-identity requirement, F06's seam) |

Design rule: `Reload` is a **handoff**, all others are **terminations**. The
coordinator models both, but only terminations end in a termination outcome;
reload ends in `Handoff` (exec) and its failure re-enters the termination path
with reason `ReloadExecFailed`.

---

## 3. The single authorities

### 3.0 Crate placement: the inversion seam (resolves B1)

Verified dependency directions at `398b51c07`:

- `jcode-base` depends on `jcode-core` (`crates/jcode-base/Cargo.toml:104`);
- `jcode-app-core` depends on both (`crates/jcode-app-core/Cargo.toml:88-89`);
- `jcode-core` depends on neither jcode crate (no `jcode-app-core` or
  `jcode-base` entries in `crates/jcode-core/Cargo.toml`).

The lease **interface** therefore lives in the neutral bottom crate:
`crates/jcode-core/src/activity.rs`, exported via `pub mod activity;` from
`crates/jcode-core/src/lib.rs`:

```rust
pub enum ActivityClass {
    ClientConnection,   // C1 carrier: one lease per counted connection
    ProviderTurn,       // C1/C2/C3 turns: any streaming turn, any initiator
    StartupRecovery,    // bounded startup restoration window (3.3.1)
    DebugJob,           // C4
    BackgroundTask,     // C5 (non-detached only; C6 never takes a lease)
    McpCall,            // C7: any in-flight MCP call, pooled or owned
    SwarmWaiter,        // C8: live await watcher task
    ScheduledDelivery,  // C9: due scheduled/ambient delivery dispatch
}

pub struct ActivityLeaseToken(u64);

pub enum ActivityLeaseError {
    ShuttingDown,       // typed refusal: acquisition after drain began
}

/// Object-safe, sync, std-only: release must be callable from Drop.
pub trait ActivityLeaseAuthority: Send + Sync + 'static {
    fn acquire(&self, class: ActivityClass, label: &str)
        -> Result<ActivityLeaseToken, ActivityLeaseError>;
    fn release(&self, token: ActivityLeaseToken);
}

/// RAII guard, mirrors `OwnedChildPermit` (`mcp/client.rs:39`): release on
/// drop, so panicked/aborted tasks can never leak a lease.
pub struct ActivityLeaseGuard { /* Arc<dyn ActivityLeaseAuthority> + token */ }

/// No-op authority for tests and non-daemon binaries.
pub fn noop_activity_authority() -> Arc<dyn ActivityLeaseAuthority>;
```

The concrete implementation (`ServerActivityLeaseAuthority`, wrapping the pure
`LeaseTable` of 3.1 in a `std::sync::Mutex`) lives in
`crates/jcode-app-core/src/server/activity.rs`. The `Server` owns the `Arc`
and injects the trait object downward at composition roots:

- `get_shared_mcp_pool` (`crates/jcode-app-core/src/server/util.rs:39-44`)
  passes it into `SharedMcpPool` construction;
- `Registry::register_mcp_tools_for_dir`
  (`crates/jcode-app-core/src/tool/mod.rs:758-778`) passes it into
  `McpManager` construction;
- `BackgroundTaskManager` construction gains an
  `with_output_dir_and_activity` variant; existing constructors default to
  `noop_activity_authority()` for tests and back-compat.

This is composition-root injection, not a reversed dependency edge. Lower
crates never import server types.

#### 3.0.1 C7 acquisition surfaces (both required)

- `McpManager::call_tool` (`manager.rs:342`): one `McpCall` guard acquired at
  entry, labeled `{session_id}/{server}/{tool}`, held across the pooled fast
  path (`manager.rs:348-355`), the owned per-session path
  (`manager.rs:357-364`), and connect-on-first-call retries
  (`manager.rs:367-388`).
- `SharedMcpPool::call_tool` (`pool.rs:232`): wrapped the same way so direct
  pool callers are covered. Nested acquisition (manager path reaching pool) is
  permitted; leases are counted, not exclusive.

F02 ownership consequences (WORK_GRAPH amendment, applied to both `all_nodes`
and the W1 expansion): add `crates/jcode-core/src/activity.rs`,
`crates/jcode-core/src/lib.rs`, `crates/jcode-base/src/mcp/manager.rs`,
`crates/jcode-base/src/mcp/pool.rs`, and
`crates/jcode-app-core/src/tool/mod.rs` to F02 `owned_paths`
(`crates/jcode-base/src/background.rs` is already owned).

### 3.1 `LeaseTable` (pure lease state)

```rust
impl LeaseTable {
    // Pure state transitions. No clock reads: callers pass `now`.
    fn acquire(&mut self, class: ActivityClass, label: &str, now: Tick)
        -> Result<ActivityLeaseToken, ActivityLeaseError>;
    fn release(&mut self, id: ActivityLeaseToken, now: Tick);
    fn active(&self) -> LeaseSnapshot;   // counts per class + oldest age
    fn is_idle(&self) -> bool;           // zero leases of every class
    fn refuse_new(&mut self);            // drain began: acquire now errors
}
```

Leases carry a label (session id, job id, task id) so the lifecycle log and
`debug_socket` can attribute what is pinning the daemon. Lease age is exposed
so stuck leases can be flagged; F01 does NOT give leases expiry (a stuck turn
keeping the daemon alive is the safe failure mode; existing per-turn timeouts
bound it).

Replacement, not parallel bookkeeping: `client_count`
(`runtime.rs:307-325`) becomes the count of `ClientConnection` leases. There
is exactly one source of truth for "clients present": the lease table's
`ClientConnection` count (resolves the review's C1 duplication note).

### 3.2 `ShutdownCoordinator` (the only exit path)

All voluntary exits (E1-E5) converge on the coordinator. The F02 lint/review
rule: any `std::process::exit` in the daemon image outside the two authorized
termination sites of 3.2.1 (the top-level runner and the coordinator-armed
watchdog) is a violation.
Reload handoff is coordinator **phase state**, never a lease (resolves B2's
self-block): the coordinator does not wait on anything it owns.

#### 3.2.1 One serialized executor

The coordinator is an actor: one dedicated task owns the phase variable and
performs every transition, every cleanup step, and the final
terminal-publication transition (it does not itself exit the process; see
termination ownership below). Callers interact only through a cloneable
`ShutdownHandle`:

```rust
impl ShutdownHandle {
    /// Non-blocking request. Never waits.
    fn begin(&self, reason: Reason) -> BeginOutcome;
    /// Request and await the terminal outcome of the whole shutdown.
    async fn begin_and_wait(&self, reason: Reason) -> TerminalOutcome;
    /// Observe without requesting.
    async fn wait_terminal(&self) -> TerminalOutcome;
}

enum BeginOutcome {
    Accepted,                        // reason now drives (first, or upgrade)
    SupersededBy(Reason),            // a stronger reason already drives
    Refused(RefusalReason),          // e.g. TemporaryServerNoReload
}

enum TerminalOutcome {
    Cleaned { reason: Reason, code: i32 },   // full cleanup ran; process NOT yet exited
    ForcedExit { reason: Reason, code: i32 }, // watchdog preempted cleanup
    Handoff,                                 // exec replaced the image
}
```

Because exactly one task executes transitions, cleanup runs at most once (a
phase latch makes re-entry impossible). Concurrent callers cannot both run
cleanup; they observe `SupersededBy` and may `wait_terminal`.

Termination ownership (revision 3, resolves re-review B-R2): the executor
NEVER calls `std::process::exit`. It finishes cleanup, publishes
`Cleaned { reason, code }` to all waiters, and stops. `Server::run` awaits the
coordinator's terminal state alongside its listener handles and returns a
typed `ServerExit { reason, code }` value (or the accept-loop `Err`); its
local resource guards, including the daemon lock, drop as `run()`'s scope
unwinds, AFTER every cleanup step. The binary's top-level runner (the sole
caller of `run()`) then performs the one normal process-termination call,
`std::process::exit(code)`. There are exactly two authorized termination
sites in the whole image:

1. the top-level runner, after `run()` returns with a `Cleaned` outcome
   (normal path);
2. the coordinator-armed watchdog thread (`ForcedExit`, 3.2.4).

This makes `begin_and_wait` coherent: waiters receive `Cleaned` before the
process exits, so paths like accept-loop failure can await full cleanup and
still return through `run()`. `Handoff` remains special: exec replaces the
process image inside the executor's `Handoff` phase and is not a termination.

Ownership consequence: the sole normal termination site is
`src/cli/dispatch.rs` (the `serve` arm constructs `Server` at
`dispatch.rs:106` and calls `run` at `dispatch.rs:114`; verified sole
production caller). Implementing the outcome-to-code mapping there requires
changing that file, so `src/cli/dispatch.rs` is added to F02 `owned_paths`
in `WORK_GRAPH.json` (both copies). F06, which also lists the file for its
later MCP owner-identity work, depends on F02, so ownership is sequential,
never concurrent. F02 acceptance evidence must cover normal outcome-to-code
mapping and the accept-loop code 45 at this site.

#### 3.2.2 Total reason priority lattice

Strongest first:

```text
SigTerm > ReloadExecFailed > AcceptLoopFailure > Reload
        > TemporaryOwnerExit > TemporaryIdle = PersistentIdle
```

Rules:

- A `begin` with a strictly stronger reason than the driving one is an
  upgrade: it is `Accepted`, replaces the driving reason, and re-derives the
  absolute drain deadline as
  `new_deadline = min(current_absolute_deadline, now + full_budget(new_reason))`
  — upgrades can only shorten or preserve a running drain, never extend it.
- Equal or weaker reasons return `SupersededBy(driving)` and change nothing.
- The two idle reasons are mutually exclusive by construction (a server is
  persistent xor temporary), so their equal ranking is unreachable in
  practice; the pure model still defines it (tie = not an upgrade).
- Upgrades never regress the phase (monotonicity, invariant I4). An upgrade
  during `CleaningUp` only changes the recorded reason/exit code if cleanup
  has not yet passed its point of no return (the first destructive step);
  afterwards it is `SupersededBy`.
- `Reload -> ReloadExecFailed` is the designed failure edge: exec failure
  re-enters the termination path as an upgrade (it is stronger), so the
  `exit(42)` fallback finally gets full sidecar cleanup.

Pairwise race tests for every ordered pair of reasons are mandatory (4.3).

#### 3.2.3 Phases

```text
Running
  -> Draining(reason, deadline)   // intake stopped, waiting for lease table
  -> CleaningUp(reason)           // leases drained OR deadline hit
  -> Cleaned                      // publish TerminalOutcome::Cleaned; runner exits
Running
  -> Draining(Reload, deadline)
  -> Handoff                      // reload persistence + exec
  -> (exec failure) Draining(ReloadExecFailed) -> CleaningUp -> Cleaned
```

- `Draining` stops intake first: accept loops stop accepting (the existing
  `RuntimeTaskScope` cancellation token, `runtime.rs:303-305`), the lease
  table starts refusing acquisition with `ActivityLeaseError::ShuttingDown`,
  and the registry entry is marked draining. The executor then awaits
  lease-table emptiness or the deadline. The coordinator holds no lease, so
  nothing it waits on is its own.
- Drain deadlines per reason: `SigTerm` short (bounded by the watchdog
  budget, 3.2.4); `PersistentIdle`/`TemporaryIdle` zero (quiescence was the
  precondition, 4.2); `TemporaryOwnerExit` short bounded grace; `Reload`
  bounded by the existing graceful-shutdown timeout (`reload.rs:334-342`);
  `AcceptLoopFailure`/`ReloadExecFailed` same as `SigTerm`.
- `CleaningUp` runs the ONE ordered cleanup list (3.4).
- `Handoff` performs the reload-specific persistence
  (`persist_reload_recovery_intents` at `reload.rs:214`, graceful session
  shutdown at `reload.rs:334`, socket removal, exec). Reload waiters await
  reload phase/marker state, not drain emptiness.
- Temporary servers: a coordinator constructed with `mode: Temporary` returns
  `BeginOutcome::Refused(RefusalReason::TemporaryServerNoReload)` for
  `begin(Reload)`. This is required because `await_reload_signal` is wired
  unconditionally (`server.rs:1195`) before the temporary/persistent choice
  (`server.rs:1668-1685`), so temporary servers CAN receive reload signals
  today. The refusal is typed, logged, reported to the requester, and tested.
- Accept-loop failure: the failure arms in `Server::run`
  (`server.rs:2181-2196`) call `begin_and_wait(AcceptLoopFailure)`, receive
  `Cleaned { .., code: 45 }` (the executor does not exit the process, 3.2.1),
  and only then return `Err(AcceptLoopFailed)`. The top-level runner
  (`src/cli/dispatch.rs:114`) maps that to exit code 45. The local
  `_daemon_lock` guard (`server.rs:2099`) remains in scope across the await
  and drops during `run()` unwind, after every cleanup step.

#### 3.2.4 The watchdog is coordinator-owned (resolves I2)

The independent SIGTERM-handler watchdog thread (`server.rs:1213-1219`) is
removed by F02. Instead:

- The executor arms one OS-thread watchdog when it accepts a `begin` for a
  termination reason, with deadline `watchdog_deadline(reason)`; it re-arms on
  upgrade (never later than the previous deadline).
- Disarm rule (exactly two disarm cases, both executor-performed): (a) at
  `Handoff` exec success; (b) after the last cleanup step completes and
  BEFORE `Cleaned` is published. Disarm is synchronous and decisive: the
  executor cancels the watchdog through a shared atomic/flag the watchdog
  thread checks before its `process::exit` call, and confirms cancellation
  won (e.g. a CAS on a two-state `Armed/Cancelled` cell) before publishing
  `Cleaned`. If the CAS loses — the watchdog already committed to firing —
  the executor does NOT publish `Cleaned`; the outcome is `ForcedExit`.
  Waiters therefore never observe `Cleaned` in an execution where the
  watchdog fires: the two terminal outcomes are mutually exclusive by the
  atomic handoff. The pure-model test drives the race between cleanup
  completion, watchdog deadline, waiter notification, guard unwind, and
  runner exit (4.3).
- Budgets are chosen so that
  `drain_deadline(reason) + cleanup_budget < watchdog_deadline(reason)`.
  For `SigTerm` the total stays within the current 3s envelope.
- If the watchdog fires, the outcome is `ForcedExit`, a distinct terminal
  outcome with its own exit code, never reported as `Cleaned`. Before
  sleeping, the watchdog thread records a durable "forced-exit armed" marker
  (reason, phase, timestamp) so a post-mortem can see cleanup was preempted
  even though the dying process cannot log afterwards.
- Forced-exit residue contract: anything the cleanup list had not reached may
  remain, and the next boot's reconciliation (E6 census machinery) must
  handle exactly that residue set. The F03 forced-path fixture kills cleanup
  deliberately (injected hang), asserts the forced exit code, then asserts
  next-boot reconciliation restores coherence. Skipped cleanup is asserted
  residue, never treated as success (satisfies A0's coherent-or-removed rule
  through the reconciliation path, and review lines 195-213).

`SIGTERM` handler after F02: its only action is `handle.begin(SigTerm)`.

#### 3.2.5 Exit-code table

| Outcome | Code |
|---|---|
| `Cleaned(SigTerm)` | 0 |
| `Cleaned(PersistentIdle | TemporaryIdle | TemporaryOwnerExit)` | 44 (`EXIT_IDLE_TIMEOUT`, `server.rs:527`) |
| `Cleaned(ReloadExecFailed)` | 42 (kept, `reload.rs:210`) |
| `Cleaned(AcceptLoopFailure)` | 45 (new, distinct) |
| (codes applied by the top-level runner in `src/cli/dispatch.rs:114`, the sole caller of `Server::run`) | |
| `ForcedExit(any reason)` | 70 (new, distinct; EX_SOFTWARE convention) |
| `Handoff` | (no exit: exec) |

### 3.3 Provider-turn wiring (resolves B3)

The `ProviderTurn` guard is acquired INSIDE
`process_message_streaming_mpsc`
(`crates/jcode-app-core/src/server/client_lifecycle.rs:3179`), at the top of
the future and held to completion, so every caller family is covered by
construction. Enumerated caller families at `398b51c07`:

| Caller family | Site |
|---|---|
| Client message tasks | `client_lifecycle.rs:2861` |
| Client actions | `client_actions.rs:1101` |
| Swarm task assignment | `comm_control.rs:991` |
| Spawned/headless initial turns | `comm_session.rs:886` |
| Jade relay | `jade_relay.rs:1211`, `jade_relay.rs:1242` |
| Live wake turns | `live_turn.rs:120` |
| Startup headless reload-recovery continuation | `server.rs:1009` (inside `recover_headless_sessions_on_startup`) |

A wiring-census test enumerates `process_message_streaming_mpsc` callers
(grep-based over production sources, excluding definitions, imports, and test
files) and fails when a new family appears without a corresponding F03
fixture entry. Every family in the table above, including the startup
reload-recovery continuation at `server.rs:1009`, requires its own F03
runtime fixture. Acquisition failure (`ShuttingDown`) surfaces as a typed
turn-refused error to the caller; no turn starts silently during drain.

#### 3.3.1 Startup recovery is its own bounded lease

`HeadlessTurn` is removed. Headless session existence is never activity
(census note, section 1.1). Instead:

- `recover_headless_sessions_on_startup` (`server.rs:721`) acquires ONE
  `StartupRecovery` lease when it finds a non-empty candidate set, holds it
  for the enumeration/scheduling window only, and releases it when scheduling
  completes; a hard TTL (default 60s) bounds it against hangs.
- Restored turns that actually run acquire `ProviderTurn` at the common
  boundary like every other turn. They do not inherit the recovery lease.

#### 3.3.2 Waiters and scheduled delivery

- C8: a live await watcher task (`comm_await.rs:364`) holds a `SwarmWaiter`
  lease for its own lifetime. A background persisted await whose watcher is
  not currently running holds NO lease: its durable state
  (`await_members_state.rs:24`) survives restart and is resumed at boot
  (`comm_await.rs:830` from `server.rs:1264`). Explicit policy: persistence,
  not daemon-pinning, is the durability mechanism for parked awaits.
- C9: the ambient runner acquires `ScheduledDelivery` when it dequeues a due
  task and begins dispatch, and releases it once the delivered turn has
  acquired its own `ProviderTurn` lease (or dispatch failed). The lease
  covers exactly the dispatch gap so the daemon cannot idle-exit between
  dequeue and turn start.

#### 3.3.3 Remaining acquisition census

| Class | Acquire site |
|---|---|
| C1 `ClientConnection` | connection accept `runtime.rs:164/230`, release at stream teardown `runtime.rs:373` |
| C1/C2/C3 `ProviderTurn` | inside `process_message_streaming_mpsc` (`client_lifecycle.rs:3179`) |
| `StartupRecovery` | `server.rs:721` recovery window (3.3.1) |
| C4 `DebugJob` | `debug_jobs.rs:72`, wrapping both spawned job tasks (`:92`, `:121`) |
| C5 `BackgroundTask` | `BackgroundTaskManager` non-detached branches, via the injected authority (3.0). Guard scope: for `spawn_with_notify`, acquired at method entry before the `tokio::spawn` at `background.rs:483-484`, so it exists before the future can execute; for the adopt path (`background.rs:628-686`, wrapping an already-running `JoinHandle`), acquired at adoption — execution before adoption belongs to the foreground owner that started the work and is covered by that owner's own lease. In both branches the guard is moved into the `RunningTask` record inserted into the live map (`background.rs:584-600`) and dropped at terminal pruning (`background.rs:551-552`, adopt: `:754-758`) after terminal status persistence; post-pruning wrapper work (output preview, bus publication) is not covered by this lease and is handed off to the scheduled-delivery/turn policy (3.3.2) |
| C7 `McpCall` | `manager.rs:342` and `pool.rs:232` (3.0.1) |
| C8 `SwarmWaiter` | `comm_await.rs:364` watcher body (3.3.2) |
| C9 `ScheduledDelivery` | ambient runner dispatch (3.3.2) |

### 3.4 Cleanup list (only real or explicitly-new APIs; resolves I3)

Ordered steps of `CleaningUp`, each individually bounded. Every named API is
verified present at `398b51c07`, except one explicitly declared new API:

1. Unregister registry: `registry::unregister_server_bounded`
   (used today at `lifecycle.rs:177`, `lifecycle.rs:251`, `server.rs:1221`).
2. Remove main + debug sockets: `transport::remove_socket`
   (used today at `server.rs:2109-2110`).
3. Remove the `.hash` sidecar (written at `server.rs:1691-1692`).
4. Remove temporary metadata when temporary:
   `lifecycle::cleanup_temporary_metadata` (`lifecycle.rs:151`).
5. Shut down MCP children pool-wide: `SharedMcpPool::disconnect_all`
   (`pool.rs:145-164`), which drains owned clients via per-client
   `shutdown`.
6. Finalize non-detached background statuses:
   **new API** `BackgroundTaskManager::finalize_non_detached(reason)` in
   `crates/jcode-base/src/background.rs` (an F02 owned path). It atomically
   marks every live non-detached task's status as failed-by-shutdown, making
   the next-boot orphan reconcile (`background.rs:317`, called from
   `server.rs:1171-1186`) unnecessary for voluntary exits. No such
   manager-wide API exists today (`reconcile_orphaned_tasks` is the only
   related surface); F02 must add it, F04/F05 later make its writes atomic.
7. Flush the lifecycle log.

Error semantics: each step logs-and-continues; a failed step never blocks
later steps. The whole phase fits the cleanup budget (3.2.4).

#### 3.4.1 Residue contract (complete set)

After the process exits following `Cleaned`, all of the following are gone
or coherent (A0):

main socket, debug socket, `.hash` sidecar, temporary metadata, registry
entry, **daemon lock file**, reload marker/recovery state, PID markers, and
owned child processes.

Daemon-lock ordering: the lock is held by the `DaemonLockGuard` local in
`Server::run` (`server.rs:2099`; guard type `socket.rs:160-199`, file removed
on drop `socket.rs:166-197`). Rules:

- Termination via the executor: the executor signals `run()`'s scope after
  cleanup; the guard drops last, after every cleanup step (the lock is the
  final residue to disappear).
- `Handoff` exec: the lock file intentionally survives into the successor
  image; the successor re-acquires or replaces it (existing reload
  behavior).
- `ForcedExit`: the guard cannot drop (process dies mid-flight); the stale
  lock is E6-class residue handled by next-boot stale-lock reaping.

### 3.5 Wiring census (which call sites route where)

| Current site | Becomes |
|--------------|---------|
| `lifecycle.rs:178` `exit(EXIT_IDLE_TIMEOUT)` | `handle.begin(PersistentIdle)` |
| `lifecycle.rs:246-256` `shutdown_temporary_server` | `handle.begin(TemporaryIdle \| TemporaryOwnerExit)` |
| `server.rs:1212-1222` SIGTERM handler (incl. watchdog thread `:1213-1219`) | `handle.begin(SigTerm)`; handler-local watchdog removed (3.2.4) |
| `reload.rs:57-211` reload task | `handle.begin_and_wait(Reload)` -> `Handoff`; exec failure -> executor-internal upgrade to `ReloadExecFailed` |
| `server.rs:2181-2196` accept-loop failure arms | `handle.begin_and_wait(AcceptLoopFailure)`, then return `Err` |
| Lease acquisition points | section 3.3.3 census |

---

## 4. Pure lifecycle state model

The model is three pure pieces composed by a thin runtime shell:

1. `LeaseTable` (3.1): pure `acquire`/`release`/`is_idle`/`refuse_new`.
2. `IdleClock` + `ExitDecision` (4.2): quiescence-epoch idle tracking.
3. `ShutdownCoordinator` phase machine (3.2): `(Phase, Reason, Tick) ->
   Phase` transitions with injected deadlines, executed by one actor.

### 4.1 Full coverage matrix: exit reason x work class

`drain` = wait up to the reason's deadline for lease release; `persist` =
write durable recovery state then release; `abandon` = bounded abandonment
after deadline (logged with lease labels); `refuse` = typed refusal outcome;
`n/a` = the class cannot be active when the reason fires (quiescence
precondition).

| Reason \ class | C1 conn | Turn (C1/C2/C3) | StartupRecovery | C4 debug job | C5 bg task | C7 MCP call | C8 waiter | C9 sched |
|---|---|---|---|---|---|---|---|---|
| SigTerm | abandon | drain->abandon | abandon | abandon | finalize then abandon (3.4 step 6) | drain->kill child | persist (`await_members_state.rs:24`) | abandon |
| PersistentIdle | n/a | n/a | n/a | n/a | n/a | n/a | n/a (live waiter leased; parked persisted) | n/a |
| TemporaryIdle | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| TemporaryOwnerExit | abandon (owner gone) | drain(grace)->abandon | abandon | abandon | finalize->abandon | drain->kill child | persist | abandon |
| Reload (persistent) | close (client reconnects to successor) | persist intent (`reload.rs:118`) + graceful shutdown (`reload.rs:130`) | drain(TTL)->abandon | abandon (job marked failed, F03 fixture) | finalize as failed-by-reload | drain briefly->kill (children die at exec) | persist (auto-resume, `comm_await.rs:830`) | abandon (runner reinit on successor, `server.rs:617`) |
| Reload (temporary) | refuse (`TemporaryServerNoReload`, 3.2.3) | — | — | — | — | — | — | — |
| ReloadExecFailed | as SigTerm | as SigTerm | as SigTerm | as SigTerm | as SigTerm | as SigTerm | persist | abandon |
| AcceptLoopFailure | join (existing `tasks.shutdown()`, `runtime.rs:303`) | drain->abandon | abandon | abandon | finalize->abandon | drain->kill | persist | abandon |

Involuntary exits (parent SIGKILL, crash, ForcedExit residue): covered not by
the coordinator but by the recovery ledger the coordinator makes redundant on
voluntary paths: stale-socket reap (`reap_stale_socket_if_dead`,
`socket.rs:76-126`), orphan background
reconcile (`background.rs:317` via `server.rs:1171`), reload-marker stale
clear (`server.rs:2126`), recovery GC (`server.rs:2128`), PID sweep
(`session.rs:66`), stale daemon-lock reap, plus F06's owner-identity work for
MCP children. The F03 parent-SIGKILL fixture asserts zero owned descendants
and clean next boot.

### 4.2 Quiescence epoch and invariants (resolves I1)

Idle state is one explicit epoch, not an externally integrated elapsed value:

```rust
struct IdleClock { idle_since: Option<Tick> }

// Maintained by the lifecycle model on every lease/client transition:
//   quiescent(now)  := clients == 0 && leases.is_idle()
//   on transition into quiescent:  idle_since = Some(now)
//   whenever !quiescent:           idle_since = None

fn should_exit(idle_since: Option<Tick>, now: Tick, timeout: Ticks) -> bool {
    matches!(idle_since, Some(t) if now - t >= timeout)
}
```

`idle_since` is `None` for the entire interval any drain-blocking lease is
held. A lease acquired, held past the timeout, then released starts a FULL
new window (the successor of `persistent_should_exit`, `lifecycle.rs:90-96`).
The temporary variant adds `owner_alive: bool` as an input, keeping the
owner-PID probe (`lifecycle.rs:270`) outside the pure core.

Invariants (the testable spec):

- I1 exit-requires-idle: `Cleaned(PersistentIdle|TemporaryIdle)` is reachable
  only when `quiescent` held continuously for the entire idle window
  (`idle_since` epoch semantics above).
- I2 single-authority: every `Cleaned` is produced by the one executor after
  `Draining` and `CleaningUp` (or `Handoff` for reload), and the process then
  exits only through the top-level runner (3.2.1). `ForcedExit` via the
  coordinator-armed watchdog is the only other voluntary process end; the two
  authorized termination sites are enumerated in 3.2.1.
- I3 no lease leak: for every `acquire` there is a `release` on every task
  outcome (success, error, panic->guard drop, abort->guard drop).
- I4 monotone phases: `Running -> Draining -> {CleaningUp|Handoff} ->
  terminal` with no backward edges; reason upgrades never regress the phase.
- I5 bounded: from accepted `begin(reason)` to a terminal outcome is bounded
  by `drain_deadline(reason) + cleanup_budget`, and that sum is strictly
  below `watchdog_deadline(reason)` (3.2.4).
- I6 no intake after drain: `acquire` fails with a typed error in every phase
  except `Running`.
- I7 outcome honesty: `Cleaned` implies the cleanup list fully executed
  (failed steps logged); `ForcedExit` implies the forced-exit marker was
  recorded and next-boot reconciliation owns the residue. There is no third
  category and no silent skip.

### 4.3 Test plan shape (for F02/F03)

- Pure: exhaustive small-state enumeration over
  (phase x reason x lease-multiset x clients x tick) asserting I1-I7;
  property tests driving random acquire/release/begin sequences; the
  quiescence-epoch hold-past-timeout-then-release case (I1); and pairwise
  begin-race tests for EVERY ordered pair of reasons (B2). No tokio, no
  sleeps: ticks and deadlines are injected.
- Runtime (F03): one no-provider fixture per lease class in 3.3.3 holding the
  lease past a short idle timeout, asserting the daemon stays alive, then
  releasing and asserting exit + zero residue per 3.4.1. One fixture per
  distinct `ProviderTurn` entry family from the 3.3 caller census, kept
  honest by the wiring-census test. MCP fixtures cover pooled AND non-shared
  owned-client calls separately (B1). Reason fixtures: SIGTERM, idle,
  temp-owner exit, reload handoff, reload-exec-failure, temporary-reload
  refusal, accept-loop failure (asserting the distinct exit code and awaited
  cleanup), forced-exit (injected cleanup hang, asserting code 70, the
  forced-exit marker, and next-boot reconciliation), parent SIGKILL.

---

## 5. Explicit non-goals of F01

- No MCP child owner-identity protocol (F06 owns it; the coordinator only
  calls pool shutdown).
- No atomic background-status writes (F04/F05). F02 adds the
  `finalize_non_detached` API (3.4 step 6); F04/F05 harden its write
  semantics.
- No lease expiry/preemption policy: stuck-work detection is surfaced via
  lease age in `debug_socket`, not auto-broken.
- No change to detached-task semantics (C6 stays outside daemon lifetime).
