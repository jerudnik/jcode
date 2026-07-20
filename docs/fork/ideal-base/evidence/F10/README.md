# F10 implementation: durable disconnect-cleanup intent

Implemented at `c27ec5a07` (worker), review fixes at `276764206`, test
isolation fix at `4b66de27c`.

## Changes

- `crates/jcode-app-core/src/server/client_disconnect_cleanup.rs`:
  - `DisconnectCleanupRecord` written to
    `~/.jcode/state/disconnect-cleanup/<sid>.json` inside the sessions
    write lock, strictly BEFORE live-session removal, on every removal
    branch (successor-connected early-return writes nothing, correct).
    Deleted only on `TerminalPersistenceOutcome::Persisted`; Failed and
    SkippedLockTimeout leave it as durable evidence.
  - `reconcile_disconnect_cleanup_records(session_is_live)`: startup sweep
    that maps records reason-aware (review finding 1): clean
    `client_disconnected` -> Closed; interrupted/reload reasons ->
    Crashed. Live sessions untouched; persist failures keep the record
    for a later startup; unloadable sessions drop it with a warning.
  - `spawn_lock_timeout_retry` (review finding 2 contract ruling): one
    bounded in-process retry 30s after a lock-timeout abort; takes the
    lock untimed, re-checks on-disk status, persists Crashed if still
    Active, clears the record; failure leaves the record for startup.
    Test-gated (opt-in delay) so the background task cannot corrupt
    other tests' process-global env windows.
- `crates/jcode-app-core/src/server.rs`: best-effort startup hook next to
  the F09 reconcile, liveness via pid markers.

## Acceptance gates

1. Agent-lock timeout leaves a durable record:
   `disconnect_lock_timeout_leaves_durable_cleanup_record`.
2. Restart marks session terminal and clears the record:
   `startup_reconcile_marks_stale_session_terminal_and_deletes_record`
   (Crashed mapping) and
   `startup_reconcile_maps_clean_disconnect_reason_to_closed` (Closed
   mapping). Plus happy-path no-residue and live-session-guard tests.

## Validation

- `client_disconnect_cleanup` module: 16 passed.
- Full `server::` suite: 490 passed, 0 failed, three consecutive runs
  (flushed out and fixed a cross-test env-corruption flake caused by the
  first retry implementation, and an unserialized env-mutating lifecycle
  test).

## Review

First-round PASS (reviews/F10-implementation-review.md) with findings 1
and 2 fixed post-review as above; finding 3 (pid-marker lock contention
folding liveness to dead) remains a recorded follow-up shared with F09.
