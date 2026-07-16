# R04 Marker/Persistence Fix Review (Independent)

- **Verdict: PASS** for the A-E source fix and its deterministic fixtures. The unsafe `reconcile_dead_owner`/disconnect marker-before-persist defect identified by the Sol/Fable sign-offs is correctly remediated. Cancellation/reload/wait-like lifecycle widening remains correctly out of scope and separately gated.
- **Reviewer:** independent verify agent (Opus). Read-only. No worktree edits.
- **Worktree:** `/Users/jrudnik/labs/jcode-fix-r04-marker`
- **HEAD:** `0f8bd8d9f5556accfebf522577d40930ac9eac47` (clean) over base `1b9d6e09f`
- **Commits reviewed:** `e264340ad` (fix: persist before cleanup), `19a0fedad` (docs), `9620bda2d` (fix: reject unstable observations), `0f8bd8d9f` (docs)
- **Infra:** disk stayed 33-35Gi free throughout; no exhaustion. Toolchain reached via `scripts/dev_cargo.sh` re-entering the repo Nix dev shell (rustc 1.96.0). No network/daemon/credentials used.

## Invariant verification (responsibility, not just files)

### 1. Terminal state persisted before marker removal (PASS)
- `Session::persist_terminal_state_with_observed_markers` (`session.rs:1082-1088`) observes markers, then `save()?`, and only on `Ok` calls `remove_session_pid_markers_if_unchanged`. The `?` makes save failure abort before any removal.
- `reconcile_dead_owner` (`session.rs:1157-1167`) now returns `Result<bool>`; on `detect_crash()` it persists-then-conditionally-consumes, propagating save errors.
- Base `Session::mark_closed`/`mark_crashed` (`session.rs:1050-1058`) are now status-only; the unconditional `unregister_active_pid`-before-save that Fable flagged CRITICAL is removed. Save-aware variants `mark_closed_and_persist`/`mark_crashed_and_persist` replace them at every runtime caller.
- Agent wrappers (`agent.rs:971-982,1000-1009`) return `Result` and call the `_and_persist` variants after snapshotting soft interrupts.

### 2. Persistence failure is observable and retains the marker (PASS)
- `reconcile_active_sessions` (`session.rs:44-67`) tracks `persistence_failed`, logs a warning per failed session, and **defers `sweep_stale_pid_markers` entirely when any reconcile save failed**, so a failed transition cannot have its marker swept by the fallback path either. Verified by fixture `reconcile_save_failure_retains_marker_and_retry_consumes_after_save` (green): on forced failure status stays `Active` and marker survives; after guard drop, retry persists `Crashed` then consumes the marker.
- All 11 propagation callers (ambient runner x5, server x2, client_session x3, turn_execution, commands_review, conversation_state, cli/terminal x2) log the error instead of silently discarding. Grepped: no remaining non-test caller of the base status-only `mark_closed()/mark_crashed()` outside `session.rs` internals.

### 3. Conditional removal targets only the exact observed marker, never a successor (PASS)
- `remove_marker_if_unchanged` (`active_pids.rs:414-431`) re-reads contents AND file identity (`len`, `modified`, and on unix `dev`+`ino`) under the marker lock, refusing removal if either differs. Content-only matching is explicitly rejected because a successor may re-register the same PID bytes via atomic replace.
- Fixtures `stale_reconcile_success_preserves_replaced_live_successor_marker` and `..._failure_..._retry_skips_live_owner` (both green) prove a successor registered after observation survives on both save-success and save-failure paths, and that retry observes the live successor and skips stale crash classification (`reconcile_dead_owner` early-returns via `active_marker_is_live()`).

### 4. Observation rejects unstable content/identity (PASS)
- `observe_pid_marker` (`active_pids.rs:140-159`) captures metadata-before, reads content, re-reads metadata-after, and returns `None` if identity changed across the read (commit `9620bda2d`). `observe_session_pid_markers` now holds `PidMarkerLock` for the whole observation.
- Fixture `observation_rejects_marker_replaced_between_content_and_metadata_read` (green) forces a same-content atomic replacement between the content and metadata reads via a test-only seam and asserts the observation is discarded and the marker survives cleanup.

### 5. Lock timeout/failure fails closed (PASS)
- Every mutating helper (`register`, `unregister`, `observe`, `remove_*_if_unchanged`, `mark/unmark_streaming`, `sweep`) early-returns a default/`false` when `PidMarkerLock::acquire()` fails, removing/writing nothing. Fixture `lock_failure_leaves_marker_state_untouched` (green) makes the lock path unopenable and asserts no deletion and no uncoordinated write.
- Disconnect agent-lock timeout: `cleanup_client_connection` (`client_disconnect_cleanup.rs:128-209`) wraps `agent.lock()` in a 2s (test-overridable) timeout; on `Err` it logs "skipping graceful shutdown" and performs no terminal marking. Fixture `disconnect_agent_lock_timeout_is_observable_without_terminal_persistence` (green) holds the agent lock, forces a 0ms timeout, and asserts status stays `Active` and the marker still exists (no false terminal claim).

### 6. No replaced live marker deleted; guarded crash-scan intact (PASS)
- The pre-existing guarded scan `find_crashed_via_pid_files` (`session/crash.rs:356-388`) still saves before `remove_active_pid_marker_if_stale_and_matches`, unchanged and correct.
- `detect_crash` no longer has a marker side-effect. Its other callers (`crash.rs:57,82`, `safety.rs:480`, `restart_snapshot.rs`, `restart.rs`) previously relied on `mark_crashed` deleting the marker; they now leave the dead-PID marker for the guarded/liveness-checked sweep. This is strictly safer than the old unconditional unlink and cannot delete a live successor (sweep rejects live PIDs). Not a regression.

## Fixtures cross the real seam and cover every claimed case (PASS)
All executed green in the Nix dev shell:
- `jcode-storage --lib active_pids`: 8/8 (sweep live-preserve, conditional replaced-live, observation race, lock failure, temp residue, explicit sweep, counts, streaming guard).
- `jcode-base --lib reconcile stale_reconcile`: 8/8 (save-failure retry, replaced-successor success+failure, dead-pid crashed, sweep-without-session-data, plus background orphan reconciliation).
- `jcode-app-core --lib client_disconnect_cleanup`: 8/8 (crashed save-failure successor survival, reloading save-failure successor survival, idle-closed ordering, lock-timeout observability, and 4 disposition-classification unit tests).

Matrix coverage claimed by A-E: save failure (B) ✔, successor replacement (C) ✔, disconnect crash/reload (D) ✔, disconnect closed + lock-timeout (E) ✔, observation race (remediation) ✔. Dead-owner reconciliation (A) traced in source and covered by base reconcile fixtures ✔.

## API propagation and session-replacement behavior (PASS)
- `Result` propagation is consistent across all 11 runtime callers; storage `lib.rs` re-exports the two new public types and two new functions.
- Session replacement paths (`turn_execution.rs` pre-clear, `client_session.rs` clear/resume/detached-source) all persist-close the old session before creating the replacement, and log on failure. No path deletes the new session's marker: conditional removal is keyed to the observed old-marker identity.

## Docs / R11 append-only (PASS)
- Both `19a0fedad` and `0f8bd8d9f` ledger diffs are pure additions (zero content-line deletions), append-only per R11.
- Preserved reviews unchanged: `opus-review.md` = `56d7cd1e...d7149fa` and `grok-review.md` = `9113aab3...81b6906d`, matching the ledger's recorded hashes exactly. The Sol/Fable sign-off and rereview artifacts are present and untouched by these commits.
- No `QUALITY_GATES`/ratchet/budget/baseline file touched (no `--update`), consistent with the ledger's R09 posture.

## Adversarial findings (non-blocking)

- **[LOW / out of scope] Reloading disposition persists as `Crashed` status.** `DisconnectDisposition::Reloading` maps to `agent.mark_crashed("Server reload interrupted processing")` (`client_disconnect_cleanup.rs:141-150`). This is **pre-existing** behavior (identical at base `1b9d6e09f`), not introduced here, and the swarm member status is separately set to `stopped`/"server reload in progress". The resumable-wait/reload semantic correction is explicitly deferred to the ledger's widening slice 4 (`resumable interrupted wait`), so it is correctly not a blocker for this marker/persistence fix. Flagging only so it is not mistaken as newly-correct terminal semantics.
- **[INFO] Test-only seams compiled behind `#[cfg(test)]`/`#[cfg(debug_assertions)]`.** The failure/replacement injection env vars (`JCODE_TEST_FAIL_TERMINAL_SAVE_FOR_SESSION`, `JCODE_TEST_REPLACE_TERMINAL_MARKER_AFTER_OBSERVE...`, `JCODE_TEST_REPLACE_MARKER_AFTER_OBSERVE_CONTENT_PATH`) are gated so they cannot alter release behavior. `maybe_force_terminal_save_failure`/`maybe_replace_terminal_marker_after_observe` are `#[cfg(debug_assertions)]` no-ops in release. Acceptable.
- **[INFO] Lock re-entrancy.** `mark_*_and_persist` and `reconcile_dead_owner` acquire the marker lock in `observe_session_pid_markers`, release it, then re-acquire in `remove_session_pid_markers_if_unchanged`. No nested acquisition of the exclusive `fs2` lock within one call, so no self-deadlock. The save between them is not lock-held, which is fine (removal re-validates identity under lock).

## Commands run (key)
```
git log --oneline 1b9d6e09f..0f8bd8d9f        # 4 commits, 2 fix + 2 docs
git show e264340ad / 9620bda2d / <docs>        # inspected all four
shasum -a 256 .../opus-review.md .../grok-review.md   # match ledger hashes
git show <docs> | grep '^-[^-]'                # zero deletions -> append-only
dev_cargo.sh test -p jcode-storage --lib -- active_pids          # 8/8 pass
dev_cargo.sh test -p jcode-base --lib -- reconcile stale_reconcile # 8/8 pass
dev_cargo.sh test -p jcode-app-core --lib -- client_disconnect_cleanup # 8/8 pass
grep for residual base mark_closed()/mark_crashed() runtime callers  # none
git diff --name-only | grep -iE 'quality_gate|ratchet|budget'   # none
```

## Confidence and gaps
- **High** confidence: all six responsibility invariants hold in source and are backed by green deterministic fixtures I executed myself in an isolated dev shell (no network/daemon). API propagation is complete. Docs are append-only and preserved review hashes reproduce.
- **Medium** confidence limited to concurrency edges the fixtures do not model: true multi-process interleaving of observe/replace/remove is simulated via single-process test seams, not real concurrent daemons (the ledger already records this as a widening risk, and multi-daemon concurrency is out of the A-E scope).
- **No blocking gap found.** The two remaining items (Reloading->Crashed semantics; multi-daemon concurrency) are pre-existing and explicitly gated by the ledger's lifecycle-widening contract, not by this fix.

**Bottom line:** The fix moves durable terminal persistence ahead of PID-marker removal on the previously-unsafe `reconcile_dead_owner` and disconnect paths, makes persistence failure observable and marker-retaining, and removes markers only by exact content+identity match under lock so no live successor is deleted. Observation itself rejects unstable content/identity, and lock failure/timeout fails closed. Every claimed A-E fixture plus the observation-race remediation exists, crosses the real storage/session/disconnect seam, and passes. Docs are append-only with intact preserved reviews. PASS.
