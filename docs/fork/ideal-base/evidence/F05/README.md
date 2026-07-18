# F05 evidence: durable background status store verification

Recorded: 2026-07-18. Scope is bounded to `TaskStatusStore`, background manager reconciliation/terminal construction, and the F05 regression fixtures.

## Implementation

- **Crash durability**: `write_atomic` uses a unique same-directory `*.json.tmp.<pid>.<uuid>` file, writes and `sync_all`s it, atomically renames it over the destination, then opens and `sync_all`s the parent directory. Temp creation uses `create_new` so a stale name is never truncated.
- **Cross-instance serialization**: path-keyed process-wide mutexes serialize read-modify-write cycles from independent `TaskStatusStore` and `BackgroundTaskManager` instances sharing a directory.
- **Stale temp cleanup**: startup reconciliation removes `*.json.tmp.*` files whose encoded PID is dead or whose mtime is at least 24 hours old. Unknown suffixes require the age rule.
- **Task-ID collision policy**: `write_initial` is create-once. Any existing valid status, including `Running`, is rejected without clobbering it. Existing malformed/unreadable files surface as errors.
- **Delivery/completion race**: terminal construction preserves delivery flags from freshly loaded persisted state, preventing a stale terminal snapshot from overwriting a concurrent delivery update.

## Verification gates

| Gate | Named test(s) | Assertion |
|---|---|---|
| Cross-instance race matrix | `cross_instance_concurrency_never_tears_json_or_overwrites_terminal_truth` | Three managers share one directory while progress, delivery, and completion race; a concurrent raw reader always parses complete JSON and final terminal truth is `Completed`, exit 0. |
| Crash interruption | `crash_interruption_preserves_old_status_and_reconcile_cleans_temp` | A truncated dead-PID temp sibling never hides the valid old status; startup cleanup removes the temp and preserves the old file. |
| Age cleanup | `reconcile_cleans_age_expired_temp_even_when_pid_is_live` | A 25-hour-old temp is removed through the age fallback even when its encoded PID is live. |
| Malformed matrix | `malformed_status_matrix_surfaces_without_false_success_or_crash` | Truncated JSON, an empty file, and wrong-schema JSON return strict read errors; reconcile/list/status do not crash or fabricate success, and preserve the corrupt evidence files. |
| Running-ID collision | `write_initial_rejects_existing_running_collision_without_clobbering` | A second store cannot replace an existing `Running` file or its session metadata. |
| Orphan reconciliation | `reconcile_marks_orphan_from_reloaded_process_failed`, `reconcile_marks_orphan_from_dead_process_failed`, `reconcile_leaves_non_orphans_alone`, `status_read_self_heals_orphaned_task` | Reload/dead-owner orphans still finalize as failed while live/current/legacy/detached states remain conservative. |
| Terminal and cleanup regression | `concurrent_writers_never_tear_json_or_lose_terminal`, `progress_and_delivery_survive_concurrent_terminal_completion`, `update_delivery_applies_to_running_task_completion`, `terminal_persistence_failure_retains_tombstone_then_recovers`, `cancel_retains_tombstone_until_terminal_persistence_recovers` | Existing F04 precedence, delivery, retry, cancel, and pruning invariants remain green after durability changes. |

## Source anchors

- `crates/jcode-base/src/background/store.rs:68-145`: shared path locks and fsync ordering.
- `crates/jcode-base/src/background/store.rs:148-192`: stale temp eligibility and removal.
- `crates/jcode-base/src/background/store.rs:228-245`: create-once initial status policy.
- `crates/jcode-base/src/background.rs:422-456`: startup temp cleanup plus orphan sweep.
- `crates/jcode-base/src/background.rs:1672-1711`: persisted delivery flags win during terminal construction.
- `crates/jcode-base/src/background/tests.rs:633-868`: F05 manager verification matrix.
- `crates/jcode-base/src/background/store.rs:585-604`: existing-Running collision fixture.

## Validation

1. `scripts/dev_cargo.sh test -p jcode-base --lib background`
   - **43 passed; 0 failed; 0 ignored; 1146 filtered out**.
2. `scripts/dev_cargo.sh test -p jcode-base --lib`
   - **1186 passed; 0 failed; 3 ignored; 0 filtered out**.
3. `scripts/dev_cargo.sh test -p jcode-app-core --lib server::`
   - First run: **467 passed; 1 failed; 681 filtered out** due to `dead_pid_sweep_marks_swarm_member_crashed_without_picker`.
   - Isolated rerun of that exact test: **1 passed; 0 failed**.
   - Full server-filter rerun: **468 passed; 0 failed; 681 filtered out**.
4. `git diff --check`
   - Exit 0.

## Boundaries

This verifies crash-interruption behavior with deterministic filesystem fixtures, not a real power cut. The shared mutex registry coordinates independent stores inside one process; true cross-process same-task writers and Windows replace semantics remain outside this node.
