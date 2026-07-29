# F26 evidence: PID-marker sweep, telemetry liveness, duplicate removal

Node: `F26` (implement), parent `W4`, base commit `eee5ccc71`.
Acceptance context: `ACCEPTANCE_STANDARD.md` A7.

Scope guard applied: `DECISIONS.md` D008 / finding GN-4 records that the startup
PID sweep pre-exists and that F26 must begin with a *verify* of it. That verify
was performed first and is reported below as a finding, not a change.

## Gate outcomes

| Gate | Statement | Outcome |
| --- | --- | --- |
| 1 | Dead markers without session JSON disappear at startup | **Already satisfied at `eee5ccc71`.** Verified by fixture, not by reading code. No production change was needed or made. |
| 2 | Only live PID telemetry markers count | **Changed.** The 24-hour mtime approximation was replaced with process liveness in `crates/jcode-telemetry-core/src/state_support.rs`. |
| 3 | Full build proves duplicate removal safe | **Changed.** `crates/jcode-app-core/src/telemetry_state.rs` deleted after equivalence review; `build --workspace` green. |

## Gate 1: verify-first, and the honest result

`reconcile_active_sessions` (`crates/jcode-base/src/session.rs:43`) reconciles
persisted sessions whose owner PID exited, then calls
`crate::storage::sweep_stale_pid_markers()` at `session.rs:66`. That sweep
(`crates/jcode-storage/src/active_pids.rs:362`) is deliberately independent of
session persistence, so a marker whose session JSON is missing or corrupt is
still removed. It is invoked at TUI startup from `src/cli/tui_launch.rs:425` and
periodically from the swarm sweep at
`crates/jcode-app-core/src/server/swarm.rs:283`, which satisfies "startup and
periodically".

Because the behavior pre-existed, passing tests alone would be a vacuous claim.
Non-vacuity was established by mutation: the `sweep_stale_pid_markers()` call in
`reconcile_active_sessions` was commented out, the same test filter re-run, and
the gate-1 test failed with `post-reconciliation sweep must remove markers whose
session data cannot be loaded`. The mutation was then reverted with
`git checkout --` and the filter re-run green. Both runs are in
`command-log.txt`.

**Finding: gate 1 required no production change.** F26 contributes verification
evidence for it, not code.

## Gate 2: process liveness instead of a 24-hour mtime approximation

Before: `prune_active_session_files` kept any marker whose mtime was under 24
hours old and wrote marker contents of the literal `"1"`. A session that crashed
kept inflating `active_sessions_at_start`, `other_active_sessions_at_start`, and
`max_concurrent_sessions` for up to a day.

After: `register_active_session` writes `pid=<pid>`, and
`active_session_marker_is_live` counts a PID-bearing marker only while
`jcode_core::process::is_running` reports its owner alive.

Two deliberate design choices:

1. **The `pid=` prefix is required, not cosmetic.** The legacy content is the
   literal `"1"`, which parses as PID 1 (`launchd`/`init`) and is therefore
   always alive. Reading legacy markers as PIDs would have made them immortal,
   turning a 24-hour bug into a permanent one. The prefix keeps legacy and
   unparseable markers on the original age bound so they still expire.
2. **The age bound is retained as the PID-reuse mitigation, and this is a
   documented limitation rather than a full solution.** A naive liveness probe
   reports "alive" for a PID the operating system recycled onto an unrelated
   process. Stronger options were considered and rejected for this node:
   - *Start-time cross-check* (`proc_pidinfo`/`KERN_PROC` on macOS, `/proc/<pid>/stat`
     field 22 on Linux) would be sound, but there is no existing cross-platform
     start-time helper in `jcode-core`; adding one is a `jcode-core/src/process.rs`
     change, and that path is **not owned by F26**. Introducing it here would
     breach the ownership boundary in `EXECUTION_PROTOCOL.md` section 4.
   - *Owner token* (random nonce written by the owner and re-verified) does not
     actually solve reuse on its own: nothing ties the nonce to the recycled
     process, so it detects a rewritten marker but not a recycled PID.

   The adopted behavior is therefore: liveness is the primary signal and fixes
   the crash case immediately; PID reuse degrades to the pre-existing 24-hour
   bound instead of being permanent. This is strictly better than the prior
   state and honestly narrower than "PID reuse is solved".

Non-vacuity: the liveness branch was mutated back to the pure mtime
approximation; 2 of 6 tests failed
(`dead_pid_marker_does_not_count_even_when_recently_written` and
`prune_active_session_files_removes_dead_pid_markers_and_counts_live`, the
latter counting 3 where 2 was expected). Reverted, 30/30 crate tests pass.

## Marker fixture matrix

Two distinct marker families exist and the matrix covers both: the
`~/.jcode/active_pids` + `~/.jcode/streaming_pids` session markers (gate 1) and
the `~/.jcode/telemetry_active_sessions` markers (gate 2).

| # | Fixture | Expected | Proven by |
| --- | --- | --- | --- |
| R1 | Dead PID, **no** session JSON | Marker removed at startup reconcile | `jcode-base` `session::tests::cases::reconcile_active_sessions_sweeps_dead_marker_without_session_data` |
| R2 | Dead PID, session JSON present | Session reconciled `Active` -> `Crashed`, marker consumed | `jcode-base` `session::tests::cases::reconcile_active_sessions_marks_dead_pid_crashed` |
| R3 | Live PID + marker | Marker preserved; sweep is idempotent | `jcode-storage` `active_pids::tests::stale_marker_sweep_removes_dead_and_invalid_but_preserves_live` |
| R4 | PID recycled onto an unrelated live process | Counts as live (documented limitation), bounded by the max age rather than permanent | `jcode-telemetry-core` `state_support::tests::pid_reuse_is_bounded_by_the_max_age` |
| R5 | Malformed marker file | Session markers: removed. Telemetry markers: fall back to the age bound, so a fresh one is kept and an aged one is removed | `jcode-storage` `active_pids::tests::stale_marker_sweep_removes_dead_and_invalid_but_preserves_live`; `jcode-telemetry-core` `state_support::tests::malformed_and_unreadable_markers_fall_back_to_age` |

R5's two behaviors are intentionally different. Session PID markers are pure
process bookkeeping, so an unparseable one is residue and is deleted. A
telemetry marker only feeds a concurrency counter; deleting an unreadable one
immediately would let a transient read error undercount live sessions, so it
keeps the bounded age fallback.

Additional telemetry rows proven beyond the required five:
`live_pid_marker_counts`, `dead_pid_marker_does_not_count_even_when_recently_written`,
`legacy_marker_keeps_age_based_treatment`,
`prune_active_session_files_removes_dead_pid_markers_and_counts_live`.

## Equivalence review of the app-core duplicate

Claim under test: `crates/jcode-app-core/src/telemetry_state.rs` is tracked but
uncompiled, and is a duplicate of
`crates/jcode-telemetry-core/src/state_support.rs`.

**Uncompiled: confirmed.** `crates/jcode-app-core/src/lib.rs` declares 28 `pub mod`
entries and none is `telemetry_state`; a repository-wide search for
`mod telemetry_state` returns zero declarations. The file is therefore not in any
module tree and never reaches `rustc`. Two corroborating signals: it begins
`use super::{SESSION_STATE, sanitize_telemetry_label};`, and `jcode-app-core` has
neither item, so it could not compile even if declared. Provenance:
`4dd91a9c6` (Phase A extraction) carried it into `jcode-app-core`; `b7b09ee55`
extracted telemetry into `jcode-telemetry-core`, and `4aec863e2` (Phase B split)
left the original behind.

**Equivalence: confirmed, with two intended and behavior-preserving differences.**
`diff` reports 54 changed lines, of which only 4 are lines unique to the app-core
copy:

| app-core line | telemetry-core replacement | Assessment |
| --- | --- | --- |
| `use crate::storage;` | `use jcode_storage as storage;` | Import-path only. Same crate, same functions. |
| `use std::path::PathBuf;` | `use std::path::{Path, PathBuf};` | Consequence of the row below. |
| `if crate::build::get_repo_dir().is_some()` (in `build_channel`) | `if telemetry_jcode_repo_dir().is_some()` | Behaviorally equivalent; see below. |
| `crate::build::get_repo_dir().is_some()` (in `is_git_checkout`) | `telemetry_jcode_repo_dir().is_some()` | Same. |

The remaining ~50 diff lines are the added local
`is_jcode_repo_dir`/`find_jcode_repo_in_ancestors`/`telemetry_jcode_repo_dir`
block in telemetry-core, which exists because the leaf crate cannot depend on
`jcode-app-core`. Comparing it against
`crates/jcode-build-support/src/paths.rs:10` (`get_repo_dir`, re-exported by
`crates/jcode-app-core/src/build.rs:1`), the resolution order is identical:
`JCODE_REPO_DIR` -> `CARGO_MANIFEST_DIR` ancestors -> `current_exe()` three
parents up -> `current_dir()` ancestors. The predicate is also identical:
`is_jcode_repo` (`paths.rs:711`) requires `Cargo.toml`, `.git`, and
`name = "jcode"` in the manifest, matching `is_jcode_repo_dir`
(`state_support.rs:266`). The only reachable divergence is which crate's
`CARGO_MANIFEST_DIR` is baked in at compile time, and since both crates live
under the same repository the ancestor walk resolves to the same root.

**Verdict: the app-core copy is a strict, stale subset of the live
telemetry-core module and has no unique behavior. Deleting it is safe**, and no
divergence needed reporting. Gate 3 is the empirical confirmation: if anything
had referenced it, `build --workspace` would fail.

## Validation

Exact commands and their recorded results are appended to `command-log.txt`,
which `score.sh` parses. Reproduce the score with:

```bash
bash docs/fork/ideal-base/evidence/F26/score.sh
```

Remote cargo was disabled (`JCODE_REMOTE_CARGO=0`) for every run: the configured
remote builder resolves this worktree's gitdir under
`/Users/jrudnik/labs/jcode/.git/worktrees/w4-f26`, which it cannot see, so it
fails with a libgit2 path error. All builds and tests are local.

| Command | Result |
| --- | --- |
| `scripts/dev_cargo.sh test -p jcode-base --lib reconcile_active_sessions` | 2 passed, 0 failed (gate 1 baseline at `eee5ccc71`) |
| same, with the sweep call mutated out | 1 passed, **1 failed** (gate 1 non-vacuity) |
| same, mutation reverted | 2 passed, 0 failed |
| `scripts/dev_cargo.sh test -p jcode-telemetry-core --lib state_support` | 6 passed, 0 failed |
| same, with liveness mutated back to mtime | 4 passed, **2 failed** (gate 2 non-vacuity) |
| `scripts/dev_cargo.sh test -p jcode-telemetry-core --lib` | 30 passed, 0 failed |
| `scripts/dev_cargo.sh build --workspace` (duplicate deleted) | see `command-log.txt` `GATE3:` line |
| `scripts/dev_cargo.sh test -p jcode-storage --lib active_pids` | see `command-log.txt` |

Residue check: the mutations were reverted with `git checkout --` and
`git diff --stat` confirmed a clean tree before proceeding. No process, socket,
or temporary marker state is left behind; all fixtures use `tempfile::TempDir`
scoped to the test.

## Files changed

- `crates/jcode-telemetry-core/src/state_support.rs` - PID-bearing markers,
  liveness-based pruning, six new tests.
- `crates/jcode-app-core/src/telemetry_state.rs` - deleted (uncompiled duplicate).
- `crates/jcode-storage/src/active_pids.rs` - **unchanged**; gate 1 verified only.
- `crates/jcode-base/src/session.rs` - **unchanged**; gate 1 verified only.
