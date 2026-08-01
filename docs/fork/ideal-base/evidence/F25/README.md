# F25 evidence: socket sidecars, malformed swarm state, control-log retention

## Implementation summary

- Centralized endpoint artifact inventory in `server/socket.rs`: main socket, debug socket, `.hash`, temporary `.server.json`, and daemon lock are derived from the main socket path. Graceful shutdown supplies its exact configured debug socket path instead of assuming the naming-convention sibling; stale-path repair still derives the sibling because it begins from one discovered endpoint.
- Preserved distinct cleanup modes:
  - reload and endpoint-only cleanup remove main/debug sockets only,
  - graceful owned shutdown removes endpoints and hash, plus temporary metadata only for temporary daemons,
  - Unix stale cleanup removes all endpoint artifacts only after listener-lock-listener proof, then asks the registry to drop stale metadata,
  - non-Unix stale cleanup remains endpoint-only to avoid widening without an ownership proof.
- Typed the swarm-state directory scan so snapshots, backups, control logs, temp files, quarantine files, and unrelated files are handled separately.
- Malformed snapshots and malformed complete control-log lines are reported and exact corrupt bytes are copied to collision-safe quarantine files. Torn final control-log lines remain unconsumed and resumable.
- Terminal/orphan control logs are pruned only as whole files after the retention window. Active logs with a snapshot, young orphan logs, pending await cursor roots (including expired-but-not-stale awaits pending startup finalization), and unrelated JSONL files are preserved so persisted byte cursors are not invalidated.
- Cached control-log handles are reset before deletion/replacement paths so cached folds cannot diverge from replaced files. Live append subscribers remain connected across reset; notifier entries with no receivers are removed.
- Cleanup and recovery failures now retain a user-visible warning instead of adding new swallowed-error patterns. The eight F25 persistence hygiene fixtures live in a dedicated 411-line test module; the pre-existing test module remains at its 1368-line baseline.

## Final validation

The original hardened code commit is `593faf3e63a6acbd3340a2acd8800591d405b0a8`. Coordinator reconciliation is committed as `e2db6bdd03b8e767ac85aba34e09194ac8155896`: it closes the repository ratchet failures without baseline changes and fixes the exact-debug-path shutdown bug exposed by the zero-warning gate.

```text
nix develop . --command cargo fmt --all
git diff --check
# passed

scripts/remote_build.sh test -p jcode-app-core 'server::socket' --lib
# 23 passed; includes stale_reap_sidecar_matrix_removes_only_after_listener_lock_listener_proof
# and endpoint_cleanup_uses_the_explicit_debug_path

scripts/remote_build.sh --no-sync test -p jcode-app-core 'server::swarm_persistence' --lib
# 31 passed; includes collision-safe quarantine, pending-await retention, and live-notifier reset fixtures

scripts/remote_build.sh --no-sync test -p jcode-swarm-core control_log --lib
# 6 passed; includes corrupt complete-line and torn final-line fixtures

scripts/preflight.sh --ratchets-only
# all 14 ratchet/advisory gates passed; warning budget current=0 baseline=0
# swallowed-error total improved 3034 -> 3004; no baseline was updated

scripts/preflight.sh
# all 16 final gates passed in 595 seconds, including rustfmt and fork-touched clippy
```

The first broad `jcode-app-core --lib` offload reached 1158 passes and one unrelated harness failure: the remote source cache intentionally excludes the worktree `.git` file, so `debug_tool_selfdev_reload_returns_promptly_for_direct_execution` could not discover a repository. The exact F25 subsystem suites above all pass on the same SCO builder; the broad harness finding is not retried unchanged or misreported as an F25 product failure.

## Non-vacuity / mutation evidence

Direct planted production defects were restored after each run.

- Sidecar deletion gate: disabled hash sidecar deletion in `cleanup_endpoint_artifacts`.

```text
scripts/dev_cargo.sh test -p jcode-app-core stale_reap_sidecar_matrix_removes_only_after_listener_lock_listener_proof
# FAILED as expected: stale endpoint artifact should be removed: .../jcode.sock.hash
```

- Registry cleanup gate: skipped `ServerRegistry::cleanup_stale` after the Unix listener-lock-listener ownership proof.

```text
scripts/dev_cargo.sh test -p jcode-app-core stale_reap_sidecar_matrix_removes_only_after_listener_lock_listener_proof
# FAILED as expected: stale ownership proof must also remove the dead registry entry
```

- Exact debug-path gate: removed the assignment that overrides the naming-convention sibling with `ShutdownConfig`'s configured debug socket path.

```text
scripts/remote_build.sh test -p jcode-app-core endpoint_cleanup_uses_the_explicit_debug_path --lib
# FAILED as expected: the configured debug socket should be removed
# restored source then passed; restoration was byte-identical
```

- Retention cutoff gate: reversed the terminal control-log retention cutoff comparison.

```text
scripts/dev_cargo.sh test -p jcode-app-core terminal_control_log_retention_preserves_active_and_young_logs
# FAILED as expected: old terminal log must be pruned
```

- Pending-await cursor gate: removed the pending-await control-log retention guard.

```text
scripts/dev_cargo.sh test -p jcode-app-core terminal_control_log_retention_preserves_pending_await_cursor
# FAILED as expected: pending await log must survive retention (file was pruned)
```

- Snapshot quarantine exact-byte gate: wrote mutated quarantine bytes instead of the corrupt snapshot bytes.

```text
scripts/dev_cargo.sh test -p jcode-app-core malformed_snapshot_matrix_quarantines_exact_bytes_and_recovers_when_possible
# FAILED as expected: primary corrupt bytes preserved exactly
```

- Quarantine collision gate: replaced exclusive `create_new` with truncating creation so an occupied quarantine path was reused.

```text
scripts/dev_cargo.sh test -p jcode-app-core quarantine_collision_never_overwrites_existing_evidence
# FAILED as expected: a collision must choose another path
```

- Live append-notifier gate: restored unconditional notifier removal during cached-log reset.

The earlier `has_changed()` assertion remained vacuous even after marking the initial value seen: the planted sender removal still passed on the pinned Tokio watch implementation. The final fixture awaits `changed()` under a 10 ms timeout. A live idle sender keeps that future pending and reaches the timeout; an unconditionally removed sender closes the channel and resolves immediately.

```text
scripts/dev_cargo.sh test -p jcode-app-core deleting_swarm_state_resets_cached_control_log_before_replacement
# FAILED as expected: a live idle append notifier must remain pending, not close and hot-loop its await watcher
```

- Control-log corrupt complete-line gate: dropped corrupt complete-line diagnostics in `read_from`.

```text
scripts/dev_cargo.sh test -p jcode-swarm-core corrupt_complete_line_is_skipped_without_wedging_replay
# FAILED as expected: corrupt_lines length assertion left 0, right 1
```

## Known warnings / uncertainties

- Warning budget is back to zero. The previously unused `ShutdownConfig::debug_socket_path` was a real cleanup defect, not an annotation problem, and is now consumed by graceful shutdown.
- The remote full-suite repository-discovery harness needs a separate fixture or explicit repo-root contract if it is expected to run in `.git`-free remote source caches; this is outside F25's product scope.
- Protected-main landing, required checks, and acceptance remain coordinator-owned; this implementation does not self-approve or bypass branch protection.
