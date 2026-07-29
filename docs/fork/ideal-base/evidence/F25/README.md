# F25 evidence: socket sidecars, malformed swarm state, control-log retention

## Implementation summary

- Centralized endpoint artifact inventory in `server/socket.rs`: main socket, debug socket, `.hash`, temporary `.server.json`, and daemon lock are derived from the main socket path.
- Preserved distinct cleanup modes:
  - reload and endpoint-only cleanup remove main/debug sockets only,
  - graceful owned shutdown removes endpoints and hash, plus temporary metadata only for temporary daemons,
  - Unix stale cleanup removes all endpoint artifacts only after listener-lock-listener proof, then asks the registry to drop stale metadata,
  - non-Unix stale cleanup remains endpoint-only to avoid widening without an ownership proof.
- Typed the swarm-state directory scan so snapshots, backups, control logs, temp files, quarantine files, and unrelated files are handled separately.
- Malformed snapshots and malformed complete control-log lines are reported and exact corrupt bytes are copied to collision-safe quarantine files. Torn final control-log lines remain unconsumed and resumable.
- Terminal/orphan control logs are pruned only as whole files after the retention window. Active logs with a snapshot, young orphan logs, and unrelated JSONL files are preserved so snapshot/await byte cursors are not invalidated.
- Cached control-log handles and append notifiers are reset before deletion/replacement paths so cached folds cannot diverge from replaced files.

## Final validation

All final validation used the repository remote cargo path through `scripts/dev_cargo.sh`.

```text
scripts/dev_cargo.sh test -p jcode-app-core socket_tests
# 22 passed; includes stale_reap_sidecar_matrix_removes_only_after_listener_lock_listener_proof

scripts/dev_cargo.sh test -p jcode-app-core swarm_persistence_tests
# 29 passed; includes malformed snapshot/quarantine, retention, and cache reset fixtures

scripts/dev_cargo.sh test -p jcode-swarm-core control_log::tests
# 6 passed; includes corrupt complete-line and torn final-line fixtures
```

## Non-vacuity / mutation evidence

Direct planted production defects were restored after each run.

- Sidecar deletion gate: disabled hash sidecar deletion in `cleanup_endpoint_artifacts`.

```text
scripts/dev_cargo.sh test -p jcode-app-core stale_reap_sidecar_matrix_removes_only_after_listener_lock_listener_proof
# FAILED as expected: stale endpoint artifact should be removed: .../jcode.sock.hash
```

- Retention cutoff gate: reversed the terminal control-log retention cutoff comparison.

```text
scripts/dev_cargo.sh test -p jcode-app-core terminal_control_log_retention_preserves_active_and_young_logs
# FAILED as expected: old terminal log must be pruned
```

- Snapshot quarantine exact-byte gate: wrote mutated quarantine bytes instead of the corrupt snapshot bytes.

```text
scripts/dev_cargo.sh test -p jcode-app-core malformed_snapshot_matrix_quarantines_exact_bytes_and_recovers_when_possible
# FAILED as expected: primary corrupt bytes preserved exactly
```

- Control-log corrupt complete-line gate: dropped corrupt complete-line diagnostics in `read_from`.

```text
scripts/dev_cargo.sh test -p jcode-swarm-core corrupt_complete_line_is_skipped_without_wedging_replay
# FAILED as expected: corrupt_lines length assertion left 0, right 1
```

## Known warnings / uncertainties

- `ShutdownConfig::debug_socket_path` is currently unused by shutdown cleanup because cleanup derives the debug socket from the centralized inventory. It is retained for the surrounding shutdown/reload config surface.
- No pushes, merges, coordinator state edits, or self-approval were performed.
