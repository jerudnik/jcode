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

## Known deviations from the design record

- The one serialized executor is spawned by the first accepted `begin`
  rather than being a standing actor task; upgrades mutate shared state the
  executor observes each poll. Equivalent serialization (only the first
  acceptance spawns), simpler lifecycle.
- `LeaseTable` uses `Instant` directly instead of an injected `Tick`; the
  pure tests inject instants explicitly.
- `client_count` is not yet replaced by `ClientConnection` leases (design
  3.1 replacement); the monitors consult `client_count` plus
  drain-blocking leases, which preserves the A1 invariant. Full C1
  unification is left to the F03-verified follow-up.
