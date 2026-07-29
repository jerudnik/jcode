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
- Terminal/orphan control logs are pruned only as whole files after the retention window. Active logs with a snapshot, young orphan logs, pending await cursor roots (including expired-but-not-stale awaits pending startup finalization), and unrelated JSONL files are preserved so persisted byte cursors are not invalidated.
- Cached control-log handles are reset before deletion/replacement paths so cached folds cannot diverge from replaced files. Live append subscribers remain connected across reset; notifier entries with no receivers are removed.

## Final validation

The hardened code commit is `593faf3e63a6acbd3340a2acd8800591d405b0a8`. Final validation ran from a clean detached worktree at that exact SHA through a unique SCO remote cache, so concurrent work on the implementation worktree could not mutate the test subject.

```text
scripts/remote_build.sh --remote-dir .cache/remote-builds/jcode/f25-verify-593faf3 fmt --all --check
# passed

scripts/remote_build.sh --remote-dir .cache/remote-builds/jcode/f25-verify-593faf3 test -p jcode-app-core socket_tests
# 22 passed; includes stale_reap_sidecar_matrix_removes_only_after_listener_lock_listener_proof

scripts/remote_build.sh --remote-dir .cache/remote-builds/jcode/f25-verify-593faf3 test -p jcode-app-core swarm_persistence_tests
# 31 passed; includes collision-safe quarantine, pending-await retention, and live-notifier reset fixtures

scripts/remote_build.sh --remote-dir .cache/remote-builds/jcode/f25-verify-593faf3 test -p jcode-swarm-core control_log::tests
# 6 passed; includes corrupt complete-line and torn final-line fixtures
```

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

The first version of this assertion was vacuous because the receiver still had the watch channel's initial value unseen, so `has_changed()` returned success even after the sender closed. The fixture now marks that value seen with `borrow_and_update()` before reset. The same planted defect then failed on the intended assertion.

```text
scripts/dev_cargo.sh test -p jcode-app-core deleting_swarm_state_resets_cached_control_log_before_replacement
# FAILED as expected: reset must not close a live append notifier and hot-loop its await watcher
```

- Control-log corrupt complete-line gate: dropped corrupt complete-line diagnostics in `read_from`.

```text
scripts/dev_cargo.sh test -p jcode-swarm-core corrupt_complete_line_is_skipped_without_wedging_replay
# FAILED as expected: corrupt_lines length assertion left 0, right 1
```

## Known warnings / uncertainties

- `ShutdownConfig::debug_socket_path` is currently unused by shutdown cleanup because cleanup derives the debug socket from the centralized inventory. It is retained for the surrounding shutdown/reload config surface.
- Protected-main landing, required checks, and acceptance remain coordinator-owned; this implementation does not self-approve or bypass branch protection.
