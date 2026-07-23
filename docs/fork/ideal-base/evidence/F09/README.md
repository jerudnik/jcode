# F09 implementation: pending-activation reconciliation

Implemented at `d5d388028` by a delegated worker from a scouted design.

## Changes

- `crates/jcode-build-support/src/lib.rs`:
  `reconcile_stale_pending_activation(min_age, session_is_alive)` +
  `PendingReconcileOutcome {NoPending, StillFresh, InitiatorAlive,
  Completed, RolledBack, Skipped}`. Uses the already-timestamped
  `PendingActivation.requested_at`; verifies candidate identity via the
  immutable version dir + `.source.json` sidecar (version_label +
  source_fingerprint, `dev_binary_matches_source` contract: missing/bad
  folds to invalid, never panics); guards live foreign canaries (clears
  only the record, returns Skipped); rollback strips `previous_*` targets
  that no longer point at the pending version so a newer publish is never
  clobbered.
- `crates/jcode-app-core/src/server.rs`: best-effort startup hook after
  headless recovery, min_age 10 minutes, liveness via
  `jcode_storage::active_pids::observe_session_pid_markers(sid).active_marker_is_live()`.

## Acceptance gates

1. Dead session + valid candidate completes safely: test 1 (Completed,
   canary promoted, record cleared).
2. Missing/bad candidate rolls back: tests 2-3 (missing version dir,
   fingerprint mismatch -> RolledBack with symlink restore).
3. Live canary preserved: tests 4 and 6 (live initiator untouched; live
   foreign canary_session -> Skipped with canary fields and symlinks
   unchanged). Plus tests 5, 7, 8 (fresh record untouched; no-pending
   no-op; newer-publish rollback guard).

## Validation

- `scripts/dev_cargo.sh test -p jcode-build-support`: 58 passed (7 new).
- `scripts/dev_cargo.sh test -p jcode-app-core --lib`: 1147 passed, 0
  failed (full suite; one pre-existing bash-test flake fixed separately
  at the same time, see `test(bash): poll for background output`).

## Known gaps

- Tiny TOCTOU window between the liveness check and manifest write if a
  restored headless session re-registers its pid marker mid-sweep; the
  canary guard covers the dangerous half. Follow-up candidate: manifest
  file lock.
- Hook not exercised in a live daemon boot yet (compiles, logged
  best-effort); the next daemon reload will run it.
- `selfdev status` opportunistic reconcile not wired (optional in design).
