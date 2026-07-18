# F05 independent verification review

## Verdict

**PASS.**

Reviewed exact commit `9f4d34d11e9c54e8023538d6bb4ceff0780f0dfe`
(`F05: harden background status durability and verification`), which is HEAD.

Reviewer route: **Anthropic `claude-opus-4-8`** (stated honestly; I ran the
tests and read the source directly, I did not rely on the worker's transcript).

Both F05 acceptance gates are honestly met for the production topology (a
single global `BackgroundTaskManager`) and for the in-process multi-instance
regime the tests exercise:

1. *Concurrent progress/delivery/completion never tears JSON or overwrites
   terminal truth* - **MET.** Unique `create_new` temp names plus atomic
   same-directory rename guarantee readers only ever observe a whole old or
   whole new file (true even across processes). A process-wide, path-keyed
   async mutex plus first-terminal-wins guarantee terminal truth survives every
   concurrent read-modify-write cycle in this process. Verified by
   `concurrent_writers_never_tear_json_or_lose_terminal`,
   `progress_and_delivery_survive_concurrent_terminal_completion`, and
   `cross_instance_concurrency_never_tears_json_or_overwrites_terminal_truth`.
2. *Startup reconciles malformed/orphaned state without false success* -
   **MET.** Malformed/truncated/empty/wrong-schema files surface as `Err` in
   the strict store, are skipped (never fabricated as a successful or completed
   task) by `reconcile`/`list`/`status`, and are preserved as evidence.
   Orphaned non-detached `Running` files still finalize as `Failed`. Startup
   reconciliation now first sweeps interrupted-writer temp files. Verified by
   `malformed_status_matrix_surfaces_without_false_success_or_crash`,
   `crash_interruption_preserves_old_status_and_reconcile_cleans_temp`,
   `reconcile_cleans_age_expired_temp_even_when_pid_is_live`, and the retained
   F04 orphan matrix.

I found no blocking defect. The findings below are important/minor scope and
honesty observations, all of which are either disclosed in the F05 evidence
boundary or do not defeat a gate.

## Validation performed

- Confirmed `HEAD == 9f4d34d11e9c54e8023538d6bb4ceff0780f0dfe`. The only
  worktree change before writing this file was the untracked, non-committed
  `docs/fork/ideal-base/evidence/F05/plan_snapshot_before_prune.json`, which I
  did not touch.
- Read `git show 9f4d34d11` in full, plus `evidence/F05/README.md`,
  `background/store.rs`, `background.rs` (reconcile, list, status, terminal
  construction, retry recovery), `background/tests.rs`, `background/model.rs`
  (`TaskStatusFile` schema), `platform.rs` liveness, `server.rs` startup wiring,
  the F04 round-3 handoff list, and `WORK_GRAPH.json` node F05.
- **Full lib suite**: `scripts/dev_cargo.sh test -p jcode-base --lib`
  → `test result: ok. 1186 passed; 0 failed; 3 ignored; 0 filtered out`.
- **Background module**: `scripts/dev_cargo.sh test -p jcode-base --lib background`
  → `test result: ok. 43 passed; 0 failed; 0 ignored; 1146 filtered out`
  (matches the claimed 43).
- **Named F05 tests, run in isolation** (`--exact`) all green:
  `cross_instance_concurrency_never_tears_json_or_overwrites_terminal_truth`,
  `crash_interruption_preserves_old_status_and_reconcile_cleans_temp`,
  `reconcile_cleans_age_expired_temp_even_when_pid_is_live`,
  `malformed_status_matrix_surfaces_without_false_success_or_crash`,
  `write_initial_rejects_existing_running_collision_without_clobbering`,
  `concurrent_writers_never_tear_json_or_lose_terminal`
  → `test result: ok. 6 passed; 0 failed`.
- Verified `evidence/F05/SHA256SUMS`: recomputed `shasum -a 256 README.md`
  matches the recorded `074814b7...0b9ca8`.
- `git diff --check` on the commit: clean (0 whitespace defects).
- Confirmed commit file set (below) is a strict subset of node F05 `owned_paths`.

### fsync durability audit (`store.rs:102-146`)

`write_atomic` performs, in order: `OpenOptions::create_new` on a
`json.tmp.<pid>.<uuid>` sibling (so a stale name is never truncated) →
`write_all` + `sync_all` on the temp file → atomic `rename` → `open` parent
directory + `sync_all` on the parent. Every failure path is surfaced as an
`anyhow` `Err` with context: temp-create error propagates; write/fsync error
drops the handle, removes the temp, and returns; rename error removes the temp
and returns; parent-open and parent-fsync errors propagate. No error is
swallowed. This is a real improvement over F04, which had no sync at all.

### Cross-instance serialization audit (`store.rs:68-95`)

The per-store `locks` map is gone; locks now live in a process-wide
`static TASK_LOCKS` keyed by the final status **path** (`store.rs:71`,
`lock_for` at `store.rs:84-95`). This is stronger than the "separate per-task
locks" the review brief assumed: two independent stores/managers pointing at
the same directory and task **in one process** now share the same lock, closing
the F04 gap where independent stores could interleave read-modify-write cycles.
The `cross_instance_concurrency_...` test builds three separate managers over
one directory and races progress + delivery + natural completion while an
unsynchronized raw reader continuously `read_to_string` + `from_str`-parses the
final path; it asserts the reader never sees torn JSON and the final state is
`Completed`/exit 0. This honestly proves both gate-1 properties for the
in-process regime.

### Stale temp sweep safety (`store.rs:148-192`, called at `background.rs:434`)

`cleanup_stale_temp_files` deletes a `*.json.tmp.*` sibling only when its
encoded PID is dead (`pid != std::process::id() && !is_process_running(pid)`)
**or** its mtime is ≥ 24h. A live writer's own fresh temp is never eligible
(own PID is excluded, and it is not 24h old); another live process's temp is
protected by its live PID. It is invoked as the first step of
`reconcile_orphaned_tasks`, which `server.rs:1231-1232` spawns on startup, so
the "runs on the startup path" claim holds.

## Findings

### Blocking

None.

### Important

- **F05-I1 — The "cross-instance" test is in-process; true cross-process
  last-writer-wins is unproven and only implicitly disclosed.** All three
  managers in `cross_instance_concurrency_...` (`tests.rs:633-752`) live in one
  process, so they share `static TASK_LOCKS` and are fully serialized. Two
  separate OS processes writing the same task would **not** share that lock; the
  only cross-process protection is the atomic rename, which prevents torn JSON
  but permits a non-terminal read-modify-write in process A to clobber a
  terminal write that landed from process B in the TOCTOU window (classic
  cross-process last-writer-wins on non-terminal fields). The README boundary
  ("true cross-process same-task writers ... remain outside this node") is an
  honest deferral, but the test name `cross_instance` invites the reading that
  cross-process is covered when only same-process multi-instance is. This does
  **not** defeat gate 1: production uses a single global manager
  (`background.rs:1782-1787`, `get_or_init`), so the in-process shared-lock
  regime is the real one, and no-torn-JSON holds universally. Recommend a name
  or doc-comment tightening in follow-up.

### Minor

- **F05-M1 — `sync_all` is `fsync`, not `F_FULLFSYNC`; the "survive crashes"
  claim overreaches power-loss durability on macOS.** `store.rs:118,142` use
  `sync_all` (→ `fsync(2)`). On macOS/APFS `fsync` flushes to the device but not
  necessarily through its write cache; genuine power-loss durability requires
  `fcntl(F_FULLFSYNC)`. The module doc's "successful writes survive crashes"
  (`store.rs:6-8`) is true for process/kernel crashes but not fully guaranteed
  for power loss on macOS. Gate 1 is about atomicity/tearing/terminal-truth
  (crash-consistency), not power-loss durability, and `README.md:50` honestly
  states "not a real power cut," so this is a doc-strength nit, not a gate miss.

- **F05-M2 — The F04-R3-I1 / handoff item 8 delivery-during-retry fix is
  source-proven but lacks the specific regression test the handoff requested.**
  `build_terminal_status` now takes `notify`/`wake` from the freshly loaded
  `prior` state (`background.rs:1672-1706`), correctly closing the window where
  a stale `TerminalSpec` snapshot could overwrite a persisted delivery change on
  a retained tombstone. The store-level
  `terminal_precedence_survives_concurrent_delivery_update` covers the adjacent
  precedence case, but there is no test that mutates delivery while a terminal
  write is failing/pending and then asserts the recovered terminal file kept the
  new flags, which is exactly what the round-3 handoff asked for. The fix is
  present; only its dedicated regression coverage is.

- **F05-M3 — User-facing `status()`/`list()` degrade a corrupt `.status.json`
  to "absent + logged warning" rather than surfacing the error to the caller.**
  `read_status_file` (`background.rs:226-234`) maps a malformed file to `None`
  after a `warn`, so `bg status`/`bg list` report "not found" for a corrupt
  file; only the internal `store.read` is strict. This is **not** a false
  success (not-found ≠ completed/success), and the malformed-matrix test
  confirms the file is preserved and no task is fabricated, so gate 2 holds. But
  corruption is invisible to an end user at those entry points.

- **F05-M4 — Permanently-malformed `.status.json` files are skipped, never
  actively remediated.** The age sweep only targets `*.json.tmp.*` siblings; a
  corrupt final status file is skipped on every `reconcile`/`list`/`status`
  indefinitely. This is a defensible conservative choice in a machine-shared
  directory (the file may belong to another process/version), and it satisfies
  the gate's "without false success" clause, but the "reconciles malformed
  state" language is met only in the weak "does not act falsely or
  destructively" sense.

- **F05-M5 — A parent-dir fsync failure after a successful rename reports the
  whole write as failed.** In `write_atomic`, if the rename succeeds but the
  parent `sync_all` errors (`store.rs:141-144`), the function returns `Err` even
  though the new data is already visible to readers. Conservative and correct
  (durability unconfirmed → report failure); for terminal writes it triggers the
  idempotent retry loop, so no harm.

## Gate checklist

| # | Acceptance gate | Result | Evidence |
|---|---|---|---|
| 1 | Concurrent progress/delivery/completion never tears JSON or overwrites terminal truth | **PASS** | Unique `create_new` temp + atomic rename (no torn JSON, incl. cross-process); path-keyed process-wide mutex + first-terminal-wins (`store.rs:71-95,257-346`). Proven by `concurrent_writers_never_tear_json_or_lose_terminal`, `progress_and_delivery_survive_concurrent_terminal_completion`, `cross_instance_concurrency_...` (all green). Cross-process non-terminal LWW honestly deferred (F05-I1). |
| 2 | Startup reconciles malformed/orphaned state without false success | **PASS** | Strict `read` surfaces corruption as `Err`; reconcile/list/status skip without fabricating success and preserve evidence; orphans still finalize `Failed`; startup temp sweep runs first (`background.rs:433-457`, wired at `server.rs:1231-1232`). Proven by `malformed_status_matrix_...`, `crash_interruption_...`, `reconcile_cleans_age_expired_temp_...`, orphan matrix (all green). |

### F04 round-3 handoff items

| Item | Status | Where |
|---|---|---|
| 1 — file + parent-directory fsync around temp write/rename | **Addressed** | `store.rs:102-146`, errors surfaced. |
| 2 — stale `*.tmp.<pid>` cleanup | **Addressed** | `store.rs:148-192`, invoked at startup via `background.rs:434` / `server.rs:1231`. |
| 4 — task-ID collision policy for existing `Running` files | **Addressed** | `write_initial` is create-once, rejects any existing valid file incl. `Running`, malformed existing files surface as `Err` (`store.rs:228-245`); proven by `write_initial_rejects_existing_running_collision_without_clobbering`. |
| 8 — preserve delivery updates made during terminal retry | **Addressed in source; regression test not added** | `build_terminal_status` reads delivery flags from `prior` (`background.rs:1672-1706`). See F05-M2. |

### Commit scope

Committed files: `crates/jcode-base/src/background.rs`,
`crates/jcode-base/src/background/store.rs`,
`crates/jcode-base/src/background/tests.rs`,
`docs/fork/ideal-base/evidence/F05/README.md`,
`docs/fork/ideal-base/evidence/F05/SHA256SUMS`. Every path is within node F05
`owned_paths` (`docs/fork/ideal-base/evidence/F05/**`, `background/store.rs`,
`background/tests.rs`, `background.rs`). **No out-of-scope path touched.**

## What I did not check

- Real power-loss / kernel-panic durability (only deterministic filesystem
  fixtures were exercised; F05-M1). No `F_FULLFSYNC` behavior on macOS was
  validated against an actual crash.
- True cross-process (separate OS process) same-task concurrent writers and PID
  reuse under the no-shared-lock regime (F05-I1); only in-process multi-instance
  was tested.
- Windows `rename`-over-existing semantics and rename fault injection.
- Long-run growth/reclamation of the process-wide `TASK_LOCKS` map for
  high-cardinality task IDs (F04 handoff item 9, out of F05 gate scope).
- Behavior under concurrent invocation of `reconcile_orphaned_tasks` from
  multiple processes sharing one directory during startup.

## Confidence

**High** that both gates are honestly met for the production single-manager
topology and the in-process multi-instance regime the tests cover: I ran the
full suite (1186 passing), the 43 background tests, and each named F05 test in
isolation, and independently audited the fsync ordering, lock keying, temp-sweep
liveness/age logic, malformed-file paths, collision policy, and commit scope.
**Moderate** confidence only on the boundaries I could not exercise (true
cross-process writers, real power loss, Windows), all of which are explicitly
and honestly deferred in the F05 evidence rather than overclaimed.
