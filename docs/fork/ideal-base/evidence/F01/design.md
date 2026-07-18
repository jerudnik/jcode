# F01 design record: one shutdown coordinator, one activity-lease authority

Status: design only. Verified against commit
`c96c4b57de57438d63e23796e6b038027265fca4` (main). No source modified.

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
| E6 | Parent SIGKILL / crash | (no code: involuntary) | External | Nothing | Everything; next boot relies on stale-socket reap (`socket.rs:71` + documented rationale), background orphan reconcile (`server.rs:1171-1186`), reload-marker stale clear (`server.rs:2126`), reload-recovery GC (`server.rs:2128-2139`), PID-marker sweep (`jcode-base/src/session.rs:66`) |

Confirmed absences (grounds for this design):

- `persistent_should_exit(client_count, idle_elapsed_secs, idle_timeout_secs)`
  at `lifecycle.rs:90-96` consults NOTHING but client count and idle clock.
- No `lease`/`Lease` abstraction exists anywhere under
  `crates/jcode-app-core/src/server*` (re-verified; only
  `release_retained_heap_if_excessive` at `server.rs:1378` matches).
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
| C7 | Pooled MCP calls in flight | `mcp/pool.rs:232` `call_tool` -> `mcp/client.rs:108/400`; children owned per `client.rs:171` with `OwnedChildPermit` (`client.rs:39`) | None at daemon level | E1/E3 exit mid-call kills the child via process death but never drains the call; exec reload (E4) leaks in-flight call state entirely |
| C8 | Swarm await waiters | `comm_await.rs:349` `spawn_or_resume_await_members` (raw `tokio::spawn` at :364); persisted (`await_members_state.rs:24` `PersistedAwaitMembersState`, background resume at `comm_await.rs:830` from `server.rs:1264`) | Durable state exists; live waiter is an untracked task | Background awaits are auto-resumed after reload, so E4 is survivable BY PERSISTENCE, but a foreground await on an idle daemon is killed by E1 with no persisted final response |
| C9 | Scheduled/ambient delivery loop | `AmbientRunnerHandle` initialized at `server.rs:617-624`; nudged on client disconnect (`runtime.rs:375-377`) | None | A due scheduled task delivering into C2 gets no lease today |
| C10 | Reload handoff in progress | `reload.rs:57-211` between signal receipt and exec | Implicit (the reload task IS the exit path) | E1 could fire between intent persistence and exec; nothing serializes E1 against E4 |

Deliberate non-lease classes (must be documented, not leased):

- C6 detached background tasks (outlive the daemon by design).
- Debug *connections* (readonly inspection must never pin the daemon; only
  debug *jobs*, C4, hold leases).
- Registry metadata publisher, memory samplers, embedding preload, index
  warmup (`server.rs:1117-1170`): best-effort startup tasks, abandonable.

---

## 2. Exit-reason taxonomy

Every way the daemon process ends, normal or not:

| Reason | Class | Initiator | Required guarantees |
|--------|-------|-----------|---------------------|
| `SigTerm` | normal, external | OS/user/supervisor | Bounded: drain-or-abandon within grace (current watchdog: 3s, `server.rs:1215`); registry + sockets + metadata removed; exit 0 |
| `PersistentIdle` | normal, internal | idle monitor | Only when zero clients AND zero leases for the full idle window; full sidecar cleanup; exit `EXIT_IDLE_TIMEOUT` (44, `server.rs:527`) |
| `TemporaryIdle` | normal, internal | temp monitor | Same as `PersistentIdle` plus temp metadata removal (`lifecycle.rs:151`); exit 44 (`lifecycle.rs:12`) |
| `TemporaryOwnerExit` | normal, internal | temp monitor observing dead owner PID (`lifecycle.rs:204-213`) | Leases get a bounded drain (owner is gone; work is abandoned after grace), then same cleanup as `TemporaryIdle` |
| `Reload` | normal, internal | reload signal (`reload.rs:57`) | NOT a cleanup exit: persists recovery intents, drains/interrupts sessions, removes sockets, exec-replaces image; leases must be drained or persisted-for-recovery, never silently dropped |
| `ReloadExecFailed` | abnormal-but-voluntary | reload path fallback (`reload.rs:210`, exit 42) | Must record failed phase (`ReloadPhase::Failed`, done today) AND perform the same sidecar cleanup as a normal exit (missing today) |
| `AcceptLoopFailure` | abnormal-but-voluntary | listener error (`server.rs:2181-2197`) | Cancel+join owned tasks (exists via `RuntimeTaskScope`), then the same bounded cleanup as `SigTerm` (missing today) |
| `ParentSigkill` / crash / OOM | involuntary | external | Cannot run code. Guarantee shifts to (a) next-boot reconciliation (already partially present, census E6) and (b) children not outliving the parent (A0 owner-identity requirement, F06's seam) |

Design rule: `Reload` is a **handoff**, all others are **terminations**. The
coordinator models both, but only terminations end in `Exited`; reload ends in
`Handoff` (exec) and its failure re-enters the termination path with reason
`ReloadExecFailed`.

---

## 3. The single authorities

Two new pure-core components, both in `crates/jcode-app-core/src/server/`
(F02's owned paths), each unit-testable without tokio time or real processes:

### 3.1 `ActivityLeaseAuthority` (the lease table)

A handle-based registry of active work:

```rust
pub enum LeaseClass {
    ClientConnection,   // C1 carrier: one lease per counted connection
    ProviderTurn,       // C1/C2: any streaming turn, client- or server-initiated
    HeadlessTurn,       // C3: restored/headless turn (incl. startup recovery window)
    DebugJob,           // C4
    BackgroundTask,     // C5 (non-detached only; C6 never takes a lease)
    McpCall,            // C7: in-flight pooled call
    SwarmWaiter,        // C8: live await watcher (foreground or background)
    ScheduledDelivery,  // C9: due scheduled/ambient delivery in progress
    ReloadHandoff,      // C10: from signal receipt to exec/failure
}

pub struct LeaseId(u64);

impl LeaseTable {
    // Pure state transitions. No clock reads: callers pass `now`.
    fn acquire(&mut self, class: LeaseClass, label: &str, now: Tick) -> LeaseId;
    fn release(&mut self, id: LeaseId, now: Tick);
    fn active(&self) -> LeaseSnapshot;         // counts per class + oldest age
    fn is_idle(&self) -> bool;                 // zero leases of every class
}
```

Runtime wrapper: an RAII `LeaseGuard` (release-on-drop, like the existing
`OwnedChildPermit` at `mcp/client.rs:39`) so a panicked/aborted task can never
leak a lease. Leases carry a label (session id, job id, task id) so the
lifecycle log and `debug_socket` can attribute what is pinning the daemon.
Lease age is exposed so a future watchdog can flag stuck leases; F01 does NOT
give leases expiry (a stuck turn keeping the daemon alive is the safe failure
mode; the existing per-turn timeouts bound it).

Replacement, not parallel bookkeeping: `client_count`
(`runtime.rs:307-325`) becomes the count of `ClientConnection` leases;
`persistent_should_exit` gains the lease dimension (section 5) instead of a
second counter growing beside it.

### 3.2 `ShutdownCoordinator` (the only exit path)

A pure phase machine plus one runtime executor. All voluntary exits (E1-E5)
converge on `ShutdownCoordinator::begin(reason)`; direct `std::process::exit`
outside the coordinator becomes a lint/review violation in F02.

Pure phases:

```text
Running
  -> Draining(reason, deadline)     // stop intake, wait for leases
  -> CleaningUp(reason)             // leases drained OR deadline hit
  -> Handoff(reason=Reload)         // exec path only, replaces CleaningUp
  -> Exited(reason, code)
```

Phase rules (pure, testable):

- `begin(reason)` is idempotent-with-priority: a second `begin` with a
  stronger reason (SIGTERM > idle) upgrades the deadline; a weaker one is
  ignored. This serializes E1 vs E4 (census C10) by construction.
- `Draining` stops intake first: accept loops stop accepting (the existing
  `RuntimeTaskScope` cancellation token, `runtime.rs:303-305`, is the
  mechanism), new lease acquisition fails with a typed refusal, and the
  registry entry is marked draining.
- Drain deadline per reason: `SigTerm` short (keep current 3s watchdog as the
  hard bound), `PersistentIdle`/`TemporaryIdle` zero (by definition no leases
  are active, else the exit would not have been chosen), `TemporaryOwnerExit`
  short bounded grace, `Reload` bounded by the existing graceful-shutdown
  timeout (`reload.rs:334-342`).
- `CleaningUp` runs ONE ordered cleanup list (superset of E1-E5's current
  ad-hoc subsets): unregister registry (`registry::unregister_server_bounded`),
  remove socket + debug socket (`transport::remove_socket`), remove `.hash`
  sidecar (`server.rs:1691`), remove temp metadata (`lifecycle.rs:151`) when
  temporary, shut down MCP pool children (`mcp/client.rs:374`), finalize
  non-detached background statuses (avoiding the next-boot orphan reconcile
  for voluntary exits), flush lifecycle log. Each step is individually bounded;
  the whole phase is covered by the OS-thread watchdog (pattern already at
  `server.rs:1215-1219`) so a hung cleanup can never prevent exit.
- `Handoff` (reload): performs the reload-specific persistence
  (`persist_reload_recovery_intents`, graceful session shutdown, socket
  removal, exec). On exec failure it transitions to
  `Draining(ReloadExecFailed)` so the fallback `exit(42)` finally gets the
  same sidecar cleanup as every other exit.
- `Exited(reason, code)` fixes the exit code table: 0 for `SigTerm`, 44 for
  idle/owner exits, 42 for `ReloadExecFailed`, nonzero-distinct for
  `AcceptLoopFailure`.

### 3.3 Wiring census (which call sites route where)

| Current site | Becomes |
|--------------|---------|
| `lifecycle.rs:178` `exit(EXIT_IDLE_TIMEOUT)` | `coordinator.begin(PersistentIdle)` |
| `lifecycle.rs:246-256` `shutdown_temporary_server` | `coordinator.begin(TemporaryIdle | TemporaryOwnerExit)` |
| `server.rs:1212-1222` SIGTERM handler | `coordinator.begin(SigTerm)`; watchdog stays as the hard bound |
| `reload.rs:57-211` reload task | `coordinator.begin(Reload)` -> `Handoff`; `reload.rs:210` fallback -> `begin(ReloadExecFailed)` |
| `server.rs:2181-2197` accept-loop failure arms | `coordinator.begin(AcceptLoopFailure)` before returning |
| Lease acquisition points | C1: `runtime.rs:164/230` connection accept; C2: `live_turn.rs:93` entry; C3: `headless.rs:38` + `server.rs:721` recovery window; C4: `debug_jobs.rs:72`; C5: `BackgroundTaskManager` spawn/registration (non-detached branches, e.g. `background.rs:454/529/656/740`); C7: `pool.rs:232` around the call; C8: `comm_await.rs:364` watcher body; C9: ambient delivery dispatch; C10: `reload.rs` signal receipt |

---

## 4. Pure lifecycle state model

The model is three pure pieces composed by a thin runtime shell:

1. `LeaseTable` (3.1): map of live leases; pure `acquire`/`release`/`is_idle`.
2. `ExitDecision`: the successor of `persistent_should_exit`:

   ```rust
   fn should_exit(
       clients: usize,
       leases: &LeaseSnapshot,
       idle_elapsed_secs: u64,
       idle_timeout_secs: u64,
   ) -> bool {
       clients == 0 && leases.is_idle() && idle_elapsed_secs >= idle_timeout_secs
   }
   ```

   with the idle clock defined to RESET whenever `clients > 0` or any lease is
   acquired (the current monitor resets only on clients, `lifecycle.rs:180-185`).
   Temporary variant adds `owner_alive: bool` as an input, keeping the
   owner-PID probe (`lifecycle.rs:270`) outside the pure core.
3. `ShutdownCoordinator` phase machine (3.2): `(Phase, Reason, Tick) -> Phase`
   transitions with injected deadlines.

### 4.1 Full coverage matrix: exit reason x work class

Behavior of each normal exit for each active work class. `drain` = wait up to
the reason's deadline for lease release; `persist` = write durable recovery
state then release; `abandon` = bounded abandonment after deadline (logged with
lease labels); `n/a` = the class cannot be active when the reason fires (the
exit decision refuses while its lease is held).

| Reason \ class | C1 conn | C1/C2 turn | C3 headless | C4 debug job | C5 bg task | C7 MCP call | C8 waiter | C9 sched | C10 reload |
|---|---|---|---|---|---|---|---|---|---|
| SigTerm | abandon | drain->abandon | drain->abandon | abandon | finalize status then abandon | drain->kill child | persist (durable state exists, `await_members_state.rs:24`) | abandon | coordinator serializes: reload loses to SIGTERM |
| PersistentIdle | n/a (clients=0 required) | n/a (lease blocks) | n/a | n/a | n/a | n/a | n/a (live waiter holds lease; background waiter persisted+leased while live) | n/a | n/a |
| TemporaryIdle | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| TemporaryOwnerExit | abandon (owner gone) | drain(grace)->abandon | drain(grace)->abandon | abandon | finalize->abandon | drain->kill child | persist | abandon | n/a (temp servers do not reload) |
| Reload (Handoff) | close (client reconnects to successor) | persist intent (`reload.rs:118`) + graceful shutdown (`reload.rs:130`) | persist intent (headless restore path, `server.rs:721`) | abandon (documented loss; F03 fixture asserts job marked failed, not silently lost) | finalize non-detached as failed-by-reload (today: next-boot reconcile, `server.rs:1171`) | drain briefly->kill (children die at exec; successor pool restarts) | persist (`background` awaits auto-resume, `comm_await.rs:830`) | abandon (runner reinitialized on successor, `server.rs:617`) | is the reason |
| ReloadExecFailed | as SigTerm | as SigTerm | as SigTerm | as SigTerm | as SigTerm | as SigTerm | persist | abandon | terminal |
| AcceptLoopFailure | join (existing `tasks.shutdown()`, `runtime.rs:303`) | drain->abandon | drain->abandon | abandon | finalize->abandon | drain->kill | persist | abandon | n/a |

Involuntary exits (parent SIGKILL, crash): covered not by the coordinator but
by the recovery ledger the coordinator makes redundant on voluntary paths:
stale-socket reap (`socket.rs:71`), orphan background reconcile
(`background.rs:317` via `server.rs:1171`), reload-marker stale clear
(`server.rs:2126`), recovery GC (`server.rs:2128`), PID sweep
(`session.rs:66`), plus F06's owner-identity work for MCP children. The F03
parent-SIGKILL fixture asserts zero owned descendants and clean next boot.

### 4.2 Invariants (the testable spec)

- I1 exit-requires-idle: `Exited(PersistentIdle|TemporaryIdle)` is reachable
  only through states where `clients == 0 && leases.is_idle()` held for the
  entire idle window.
- I2 single-authority: every `Exited` is preceded by `Draining` and
  `CleaningUp` (or `Handoff` for reload) of the same coordinator instance.
- I3 no lease leak: for every `acquire` there is a `release` on every task
  outcome (success, error, panic->guard drop, abort->guard drop).
- I4 monotone phases: `Running -> Draining -> {CleaningUp|Handoff} -> Exited`
  with no backward edges; reason upgrades never regress the phase.
- I5 bounded: from `begin(reason)` to `Exited` is bounded by
  `drain_deadline(reason) + cleanup_deadline` in coordinator ticks, enforced
  at runtime by the OS-thread watchdog.
- I6 no intake after drain: `acquire` fails in every phase except `Running`.
- I7 sidecar coherence (A0): after `Exited`, the cleanup list is fully
  executed or each skipped step is logged with its reason (watchdog preemption
  is the only allowed skip).

### 4.3 Test plan shape (for F02/F03)

- Pure: exhaustive small-state enumeration over
  (phase x reason x lease-multiset x clients x tick) asserting I1-I6, plus
  property tests driving random acquire/release/begin sequences. No tokio, no
  sleeps: ticks and deadlines are injected.
- Runtime (F03): one no-provider fixture per lease class (C1-C5, C7-C9)
  holding the lease past a short idle timeout, asserting the daemon stays
  alive, then releasing and asserting exit + zero residue (sockets, metadata,
  hash, PID markers, processes). Reason fixtures: SIGTERM, idle, temp-owner
  exit, reload handoff, reload-exec-failure, parent SIGKILL.

---

## 5. Explicit non-goals of F01

- No MCP child owner-identity protocol (F06 owns it; the coordinator only
  calls pool shutdown).
- No atomic background-status writes (F04/F05); the coordinator only invokes
  the finalize hook the manager already exposes.
- No lease expiry/preemption policy: stuck-work detection is surfaced via
  lease age in `debug_socket`, not auto-broken.
- No change to detached-task semantics (C6 stays outside daemon lifetime).
