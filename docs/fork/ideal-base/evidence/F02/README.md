# F02 evidence: activity leases and bounded shutdown coordinator

Recorded: 2026-07-18. Implementation of the accepted F01 design revision 4
(`../F01/design.md`, accepted at review round 3, D013).

## Commits

- `9d9762e6f` — jcode-core activity-lease seam (design 3.0):
  `crates/jcode-core/src/activity.rs`.
- `e5059a3c2` — coordinator, lease authority, background finalize:
  `crates/jcode-app-core/src/server/shutdown.rs`,
  `BackgroundTaskManager::finalize_non_detached` (design 3.4 step 6).
- `2609e7a8d` — exit-path and lease-site wiring (design 3.3.3 / 3.5):
  idle monitors, SIGTERM, reload, accept-loop failure, `ServerExit`,
  `src/cli/dispatch.rs` termination site, ProviderTurn / StartupRecovery /
  DebugJob / SwarmWaiter / McpCall / BackgroundTask leases, composition-root
  injection.

## Acceptance-gate evidence

Gate 1: "Idle exit requires zero clients and zero active leases"

- `IdleClock` quiescence epoch (design 4.2) consulted by both lifecycle
  monitors over `clients == 0 && drain_blocking_leases == 0`.
- Pure test `idle_clock_quiescence_epoch` proves hold-past-timeout-then-
  release requires a full new window.
- `client_connections_are_not_drain_blocking` proves the C1 split.

Gate 2: "SIGTERM, reload, persistent idle, and temporary-owner exits invoke
bounded shutdown"

- All six termination reasons route through
  `ShutdownCoordinator::begin`; reload routes through `begin_reload_drain`
  with typed temporary refusal; reload exec failure re-enters via
  `reload_exec_failed` (the historic bare `exit(42)` is gone).
- Zero direct `std::process::exit` calls remain in the daemon image outside
  the two authorized sites (top-level runner in `src/cli/dispatch.rs`,
  coordinator-armed watchdog).

## Test results

- Pure/unit: `jcode-core` 34/34, `jcode-base --lib background` 26/26,
  `jcode-app-core --lib` 1113/1113 (455 in `server::`).
- Runtime process transcript (`exit_mode_fixtures.sh` against the selfdev
  binary in isolated `JCODE_RUNTIME_DIR` env, log
  `exit_mode_fixtures_run.log`):
  - temporary-idle: exit code 44 through the coordinator, zero
    socket/hash/metadata residue;
  - SIGTERM: exit code 0 through the coordinator, zero residue.

## Deferred to F03 (per design 4.3)

- One no-provider hold-release fixture per lease class.
- One fixture per ProviderTurn entry family (incl. startup reload-recovery).
- Forced-exit (watchdog) fixture asserting code 70, marker, and next-boot
  reconciliation.
- Pairwise reason-race runtime fixtures; parent-SIGKILL residue fixture.

## Review round 1 blocker fixes (F02-implementation-review.md, FAIL at ef67216ad)

- F02-B1: idle exit is claimed atomically. `LeaseTable::try_claim_idle_shutdown`
  verifies complete emptiness and closes acquisition in one critical section;
  `ClientConnection` leases now mirror `client_count` (one guard per counted
  connection in `ServerRuntime`), so clients and work live in the same table.
  Idle `begin` returns `Refused(NotQuiescent)` when the claim loses; the
  monitors restart the idle window. Non-idle begins call `refuse_new()` BEFORE
  the phase flips. New pure test `idle_claim_is_atomic_against_any_lease`.
- F02-B2: `ScheduledDelivery` lease wired in the ambient runner: acquired
  before the durable `take_ready_direct_items()` dequeue, held through
  delivery (covering the direct `run_once_capture` fallback paths), dropped
  after dispatch. Refusal skips the dequeue entirely: items stay durable for
  the successor daemon.
- F02-B3: `begin_reload_drain` now calls `cancel_intake()` like terminations.
- F02-B4: every refusal fails closed. Debug jobs acquire before spawn and
  error the request; await watchers do not spawn (durable state persists for
  the successor, the design's C8 "persist" semantics); startup recovery
  aborts (sessions recovered by the successor); `spawn_with_notify` never
  runs the task and writes a terminal refused status. Adoption keeps running
  (the future already exists, pre-adoption execution is the foreground
  owner's) but is logged and remains abortable by `finalize_non_detached`.
- F02-B5: watchdog thread-spawn failure falls back to `spawn_blocking`; only
  a no-runtime-and-no-thread double failure is left logged as unbounded.
- F02-I1: executor spawning uses `spawn_on_runtime` (tokio handle when
  present, dedicated thread with a one-shot runtime otherwise), so `begin`
  off-runtime cannot strand `Draining`.
- F02-I2: `StartupRecovery` guard is bounded by a 60s TTL task.
- F02-M1: watchdog cancellation rewrites the durable marker as `cancelled`.

## Review round 2 blocker fixes (Round 2 FAIL at 8a09a289d)

- F02-R2-B1: connection admission fails closed. `try_admit_client` makes the
  `ClientConnection` lease the admission gate: on ShuttingDown refusal the
  accepted stream / gateway client is dropped UNCOUNTED, preserving the
  strict one-guard-per-counted-connection pairing the atomic idle claim
  relies on. `increment_client_count` no longer exists.
- F02-R2-B2: `RunningTask` stores the ORIGINAL adopted future's
  `AbortHandle`; `finalize_non_detached` and `cancel_with_grace` abort the
  original before the wrapper, so cleanup actually cancels adopted work
  instead of drop-detaching it. New test
  `finalize_non_detached_aborts_adopted_original_future` proves the original
  future is cancelled (drop-flag assertion).
- F02-R2-I1: `begin_reload_drain` calls `refuse_new()` under the coordinator
  lock BEFORE publishing `Draining`, matching ordinary `begin`.
- F02-R2-I2: debug jobs acquire the lease before `create_job`, so a refusal
  leaves no permanently Queued record.

## Known deviations from the design record

- The one serialized executor is spawned by the first accepted `begin`
  rather than being a standing actor task; upgrades mutate shared state the
  executor observes each poll. Equivalent serialization (only the first
  acceptance spawns), simpler lifecycle.
- `LeaseTable` uses `Instant` directly instead of an injected `Tick`; the
  pure tests inject instants explicitly.
- `client_count` still exists alongside the lease table, but every counted
  connection now also holds a `ClientConnection` lease, and the idle claim
  consults ONLY the lease table (atomic). The redundant counter is kept for
  its many read-side consumers; removing it is cosmetic follow-up.
