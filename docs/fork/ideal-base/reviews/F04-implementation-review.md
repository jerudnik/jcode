# F04 independent implementation review

## Verdict

**FAIL.**

Reviewed exact commit `4f1e5adfafa4e61dd87b03612395f35a648d30ae` (`F04: atomic serialized TaskStatusStore; migrate all status writes`) at HEAD `4f1e5adfafa4e61dd87b03612395f35a648d30ae`.

Reviewer route: **OpenAI `gpt-5.6-sol`, high effort**.

The store migration substantially improves status persistence. Production status writes are centralized, same-process same-task mutations are serialized, replacement is atomic to readers, first-terminal-wins is enforced on persisted state, and write failures are returned by the store and logged by manager call sites. Gates 1 and 3 are substantially met.

Gate 2 is not honestly met under the failure behavior that F04 explicitly claims to support. Both natural-completion wrappers remove their live-map entry after `write_terminal` returns, even when it returns `Err`; initial persistence failures are also logged and execution continues. If the initial write failed and the terminal write later exhausts its retries, there is no status file to reconcile, yet the task is pruned. Cancel and shutdown-finalize paths remove live tasks before attempting terminal persistence. Thus a live/just-terminated task can disappear from the map without successful terminal persistence and without any durable recovery record.

I also found two important store/behavior defects: a no-op progress update republishes a progress bus event because `MutateOutcome::Applied` does not distinguish “closure returned false,” and `mutate` returns closure-mutated state on a false-return terminal mutation without restoring terminal truth, even though no such state was persisted.

## Validation performed

### Exact source and evidence review

- Confirmed `HEAD == 4f1e5adfafa4e61dd87b03612395f35a648d30ae` and the worktree was clean before creating this review.
- Read `git show 4f1e5adfa` completely for all six changed paths.
- Read `docs/fork/ideal-base/evidence/F04/README.md`, all of `background/store.rs`, the migrated `background.rs`, the persisted schema/helpers in `background/model.rs`, and the new manager/store tests.
- Compared the new progress/delivery paths with their pre-F04 implementation.
- Verified the F04 evidence SHA-256 entry matches the README.

### Direct-write census

Independent searches covered `background.rs` and `background/**` for:

- `fs::write`, `tokio::fs::write`, and `std::fs::write`;
- `serde_json::to_string_pretty`;
- `File::create` and `OpenOptions`;
- `.status.json` and `status_path` uses.

Results:

- The only production status serialization/write is `TaskStatusStore::write_atomic` in `background/store.rs`.
- `background.rs:949` creates/writes an output file, not a status file.
- Direct status writes in `background/tests.rs` and store tests are test-fixture setup or corruption/failure injection, not production bypasses.
- All production initial, mutation, terminal, cancel, finalize, and refused-status paths route through the store.

### Tests run

1. `scripts/dev_cargo.sh test -p jcode-base --lib background`
   - **35 passed; 0 failed; 1146 filtered out; 0.42s**.
   - Includes six store tests, the two new manager gate tests, and 27 prior background-related tests.
2. `scripts/dev_cargo.sh test -p jcode-base --lib`
   - **1178 passed; 0 failed; 3 ignored; 33.68s**.

Both commands re-entered the repository Nix shell because `cargo` was not on the ambient `PATH`. The build emitted one warning: `TaskStatusStore::read_lenient` is unused.

### Store guarantee analysis

#### Atomic replacement and temporary names

`write_atomic` serializes to memory, writes a same-directory sibling named `<task>.status.json.tmp.<pid>`, then renames it over the destination. A reader of the final path observes the old complete file or the new complete file, not partially written JSON. The implementation comment says “fsync,” but no file or directory sync is performed; crash durability is therefore not established by F04 and belongs in F05.

Within one `TaskStatusStore`, the per-task mutex makes the PID-suffixed temporary name safe for concurrent writers of the same task: only one writer can use that name at a time. Different processes use different PIDs. Two independent store instances in the same process pointing at the same directory and task ID would share the same temp path without sharing a lock, but production uses one global manager/store and generated task IDs. Cross-process/crash collision stress belongs in F05.

#### Per-task lock map

The mutex-map insertion is sound and avoids remove-while-locked races. Entries are never removed, so memory grows with distinct task IDs touched by the process. Each entry is a string, Arc, and mutex. This is acceptable for the current daemon/task-volume scope, but it is not intrinsically bounded over an indefinitely running process and should be measured or eventually reclaimed if task cardinality becomes large.

#### Terminal precedence

For persisted mutations that return true, the store restores the five declared terminal-truth fields: `status`, `exit_code`, `error`, `completed_at`, and `duration_secs`. Those fields completely describe the terminal outcome/timing. Delivery flags and event history are intentionally mutable after terminal. Actual manager mutation closures do not alter task identity, owner metadata, detached state, start time, or PID, so the narrower frozen set does not currently permit a production terminal-outcome corruption.

`write_terminal` correctly checks existing state under the per-task lock and preserves the first terminal state. A malformed existing file is logged and overwritten by terminal recovery.

#### Initial-write collision behavior

`write_initial` refuses an existing terminal file but overwrites an existing `Running` file. A same-task-ID collision with another live task could therefore replace its ownership/session metadata. Generated task IDs make this extremely unlikely, but safe collision semantics would reject any existing valid file unless an explicit recovery protocol authorizes replacement. Cross-process collision testing is appropriate for F05.

`write_initial` also uses `if let Ok(Some(existing)) = read(...)`, so a malformed/unreadable existing file is treated like absence. If the following atomic write succeeds, the read error is not surfaced. This does not directly violate gate 3's serialization/write-failure wording, but it is inconsistent with the store's general “corruption is surfaced” claim.

## Findings

### Blocking

#### F04-B1: live tasks are pruned on terminal persistence failure, including a state with no durable recovery record

The evidence claims pruning happens after terminal persistence “succeeds or is durably recoverable.” The implementation only orders pruning after the write attempt returns:

- Spawn completion calls `write_terminal`, logs an error if it fails, awaits registration, then unconditionally removes the task (`background.rs:718-781`).
- Adopted completion has the same sequence (`background.rs:955-1010`).
- `write_terminal` retries three times and returns `Err`; it does not create a separate durable failure/recovery marker (`background/store.rs:223-273`).
- Initial writes in spawn/adopt log `Err` and continue running the task (`background.rs:670-674`, `:893-897`).

Therefore a real sequence exists:

1. The initial status write fails because the directory is unwritable, absent/replaced, full, or otherwise unavailable.
2. The task still starts and is inserted into the live map.
3. It completes.
4. All terminal retries fail for the same condition.
5. The error is logged, then the live-map entry is removed.
6. No `Running` or terminal status file exists, so next-boot orphan reconciliation has nothing to discover.

Even when an initial `Running` file exists, pruning after terminal `Err` relies on a later process-image change before `status_is_reconcilable_orphan` will act. In the current process, owner instance equality deliberately prevents reconciliation. The cancel path removes/aborts the live task before its terminal write and returns `Ok(true)` after a logged write error (`background.rs:1427-1472`). `finalize_non_detached` drains the entire map before attempting any terminal writes (`:464-516`). Those paths are recoverable at next boot only if the initial file exists.

The happy-path test `live_map_prunes_only_after_terminal_persistence` never injects a terminal write failure, so it does not establish the gate under the failure semantics F04 advertises.

Fix by making successful initial persistence a prerequisite for starting/registering non-detached work, or creating a separate durable recovery record. On terminal failure, retain a recoverable live-map/tombstone entry or propagate the failure through lifecycle APIs until durable recovery is guaranteed. Add manager-level failure-injection tests for spawn completion, adoption, cancel, and shutdown finalize.

This directly fails acceptance gate 2.

### Important

#### F04-I1: no-op equivalent progress updates are republished

Before F04, an equivalent progress value returned immediately without writing or publishing. The new closure correctly returns false, and `mutate` skips the write. However, false on a non-terminal file returns `MutateOutcome::Applied(updated)`. The caller then decides whether the mutation applied by checking whether the returned persisted progress is equivalent to the requested value (`background.rs:1337-1351`). For the exact-equivalent no-op, that condition is necessarily true, so it publishes a duplicate `BackgroundTaskProgress` bus event at `:1352-1360`.

The same inference is fortunately false for the “less determinate parsed output” skip when the old value differs, but exact-equivalent updates regress prior behavior. Give `MutateOutcome` an explicit `Unchanged` variant or return an applied boolean from `mutate`; publish only on `Changed`.

#### F04-I2: `mutate` false-return state can disagree with persisted terminal truth

`mutate` snapshots terminal truth, invokes the closure, and if the closure returns false immediately returns `TerminalPreserved(updated)` without restoring terminal fields (`background/store.rs:194-200`). A closure that mutates a terminal status to `Running` and then returns false causes no disk write, which is correct, but the returned value falsely reports the hostile mutation rather than the persisted terminal state. The non-terminal false case likewise returns an in-memory mutation that was not persisted.

Current manager closures return false before modifying state, so this is not presently a disk-corruption route. It is still an incorrect store contract and undermines callers' ability to treat the returned state as persisted truth. On false, return the original state unchanged, or prevent mutation by making the closure return an explicit replacement/change decision.

#### F04-I3: “all store errors are surfaced” is weaker at manager boundaries than the evidence implies

Mutation APIs propagate store errors with `?`, which is strong. Terminal paths generally log errors and continue or return success-shaped values: `cancel_with_grace` returns `Ok(true)`, detached/orphan finalizers publish completion, wrappers publish completion, and shutdown finalize reports its count even after persistence failure. Logging at error level means the failure is not silently swallowed, so gate 3 can pass as written, but callers do not receive persistence failure and may report successful terminal handling while disk remains `Running` or absent.

A structured persistence-health event/result would make failures observable to callers and automated recovery rather than only operators reading logs.

### Minor

#### F04-M1: `read_lenient` is dead code

The method is unused and produces a warning in both focused and full test builds. `read_status_file` uses `read_path` and logs itself. Remove `read_lenient` or use it consistently.

#### F04-M2: `write_atomic` documentation claims fsync that is not implemented

The function provides atomic replacement to concurrent readers, not durable commit across power loss/kernel crash. Correct the comment now and implement file-plus-directory sync under F05 if required by its crash-interruption gate.

## Gate checklist

| Acceptance gate | Result | Evidence |
|---|---|---|
| No direct non-atomic status-file writes remain | **PASS** | Independent production census found status serialization/writing only in `TaskStatusStore::write_atomic`. Remaining direct file writes are output files or test fixtures. |
| Live task is not removed before terminal persistence succeeds or is durably recoverable | **FAIL** | Natural wrappers prune after `write_terminal` returns even on `Err`; initial persistence failure does not prevent execution, so terminal failure can leave neither a status file nor a live-map entry. Cancel and shutdown finalize remove before persistence. F04-B1. |
| Status-serialization and write failures are surfaced, not swallowed | **PASS with important qualification** | Store methods return contextual errors; mutation callers propagate and terminal callers log at error level after retries. Some manager APIs still return success-shaped results after logging, noted in F04-I3. |

## F05 handoff list

1. Crash/power-loss interruption around temp write and rename, including file and parent-directory fsync semantics.
2. Malformed/truncated destination recovery and stale temp-file cleanup.
3. Cross-process writers for one task ID, including PID reuse and two writers racing terminal/initial/mutation operations.
4. Same-process independent `TaskStatusStore` instances sharing a directory/task ID and therefore a PID-suffixed temp path without a shared lock.
5. Task-ID collision behavior: reject any existing valid status unless an explicit takeover/recovery token authorizes replacement.
6. Windows replacement semantics and fault injection for rename-over-existing destination.
7. Manager-level write-failure fixtures proving initial failure prevents untracked work and terminal failure retains a durable/live recovery authority.
8. Repeated crash/restart orphan reconciliation with permission/disk errors and malformed status files.
9. Verify no stale `*.tmp.<pid>` files survive interrupted writes or confuse directory sweeps.

## What I did not check

- I did not run `jcode-app-core`, the full workspace suite, Miri, Loom, sanitizers, or filesystem fault-injection frameworks.
- I did not test on Windows or a filesystem with nonstandard rename/durability semantics.
- I did not simulate disk-full, permission loss, process crash between temp write and rename, or process crash after rename before directory sync.
- I did not create cross-process writers or force task-ID collisions.
- I did not inspect every consumer of background bus events outside `jcode-base`; the duplicate progress publication is established locally against the pre-F04 behavior.
- I did not modify implementation code.

## Confidence

**High (99%).** The blocking sequence follows directly from explicit control flow: initial write errors do not stop execution, terminal errors do not stop pruning, and no alternate recovery record is created. The progress no-op regression and false-return state defect are similarly direct. Both requested test suites pass, but the new gate test exercises only successful persistence and therefore cannot invalidate the failure-path finding.

# Round 2: blocker-fix re-review

## Verdict

**FAIL.**

Reviewed exact commit `10209b09cb8fdb06f1a8e454fa0ea936fc574902` (`F04: fix review blockers - persistence-failure durability (F04-B1) and store contract (I1/I2/M1/M2)`) at HEAD `10209b09cb8fdb06f1a8e454fa0ea936fc574902`.

Reviewer route: **OpenAI `gpt-5.6-sol`, high effort**.

The fix closes the original blocker for ordinary spawned tasks and natural wrapper completion: a spawn now fails closed if its initial `Running` write fails, and both spawn/adoption wrappers retain their live-map entry after terminal failure while a backoff loop retries. I1, I2, M1, and M2 are correctly fixed. Cancel and shutdown-finalize are also persistence-safe for tasks whose initial `Running` file exists because that file is a durable next-boot recovery record.

One blocking hole remains at the intersection the new design explicitly permits: adoption may continue after its initial write fails, but cancel removes that adopted task before terminal persistence and `finalize_non_detached` drains it before terminal persistence. If the terminal write also fails and the process exits before the detached cancel retry succeeds, no live-map tombstone and no status file exists. The orphan sweep has nothing to recover. This is avoidable for cancel by retaining/reinserting the aborted `RunningTask` as a tombstone until persistence succeeds. Shutdown finalize needs an explicit failure policy for the same initial-write-failed adopted state. Gate 2 therefore remains false as an absolute lifecycle guarantee.

## Validation performed

### Exact diff and evidence

- Confirmed clean baseline and `HEAD == 10209b09cb8fdb06f1a8e454fa0ea936fc574902` before appending this review.
- Read `git show 10209b09c` for all five changed paths, then read the complete updated `background/store.rs`, relevant `background.rs` lifecycle/mutation regions, both new manager tests, and `evidence/F04/README.md`.
- Verified the updated evidence SHA-256 entry matches the README.
- Re-censused production status serialization/writes and live-map removal sites. No direct production status write was reintroduced. The only map removals are shutdown finalize, cancel, and the terminal-recovery helper.

### Tests run

1. `scripts/dev_cargo.sh test -p jcode-base --lib background`
   - **37 passed; 0 failed; 0 ignored; 1146 filtered out; 0.60s**.
2. `scripts/dev_cargo.sh test -p jcode-base --lib`
   - **1180 passed; 0 failed; 3 ignored; 32.90s**.

### B1 path analysis

#### Spawn initial persistence

`spawn_with_notify` acquires its activity lease, constructs the initial state, and returns before publishing, spawning, or inserting a `RunningTask` when `write_initial` fails (`background.rs:619-683`). The local activity lease drops on return. The test `spawn_refuses_to_start_when_initial_persistence_fails` verifies the closure never runs and the task is not tracked. This closes the original no-record spawn sequence.

#### Natural spawn and adoption completion

Both wrappers await registration and call `persist_terminal_with_recovery` (`background.rs:727-758`, `:934-958`). The helper removes the map entry only on immediate terminal-write success. On failure it logs, leaves the existing `RunningTask` entry in place as a tombstone, and starts an exponential-backoff retry that removes only after `write_terminal` returns success (`:1679-1737`). The test `terminal_persistence_failure_retains_tombstone_then_recovers` breaks the directory after initial persistence, observes the retained entry, heals the directory, then observes terminal persistence and pruning. This closes the original natural-completion blocker.

#### Cancel

For an ordinary spawned task or successfully persisted adopted task, cancel's ordering is acceptable: it removes and aborts the live task first, but the already-persisted `Running` file is durable recovery authority; the helper retries in-process, and a process death makes the owner stale so the next-boot orphan sweep can finalize it.

The helper does not actually retain a cancel tombstone, however. Cancel has already removed the `RunningTask` at `background.rs:1371` before calling the helper at `:1384`, so on write failure the helper's “retaining live-map tombstone” log is false for this caller. That distinction is blocking when the task is an adopted future whose permitted initial write failed: there is neither a map entry nor an initial file after removal. See F04-R2-B1 below.

#### Adoption initial failure

The rationale at `background.rs:867-873` is sound as far as fail-closed execution is concerned: adoption receives an already-running foreground future, so refusing to start it is impossible. Tracking it despite the initial error preserves shutdown abort authority. Natural completion also uses the tombstone helper, so it remains tracked while terminal persistence fails.

The rationale does not justify removing this exceptional task on cancel/finalize without durable replacement. The implementation knows whether initial persistence succeeded and can preserve failure state in memory until a terminal record lands.

#### Shutdown finalize

For tasks with an initial file, `finalize_non_detached` draining before terminal writes is acceptable because the daemon is exiting and the initial `Running` file is precisely the next-boot recovery record. The method aborts both wrapper and adopted original authority and attempts terminal persistence synchronously.

For an adopted task whose initial write failed, the same ordering has no recovery record. The method drains it at `background.rs:465-471`, logs a terminal error at `:507-515`, returns its success-shaped count, and shutdown may complete. There is no retry loop, tombstone, or file for the next process.

### I1 and I2 behavior

- `MutateOutcome::Unchanged(TaskStatusFile)` now explicitly represents closure-declined writes and carries the untouched existing persisted state.
- `mutate(false)` returns `Unchanged(existing)` before terminal restoration/write logic, so hostile in-memory edits are discarded and the returned value is persisted truth (`store.rs:188-195`).
- Progress matching publishes only for `Applied`; `Unchanged`, `TerminalPreserved`, and `Missing` return before bus publication (`background.rs:1285-1303`). An applied mutation reaches exactly one publish call. Thus equivalent no-op updates no longer publish and applied updates publish once.
- `update_delivery` handles `Unchanged` consistently with the other state-bearing outcomes. Its closure currently always returns true, so that arm is defensive rather than reachable through current manager behavior.

## Findings

### Blocking

#### F04-R2-B1: cancel/finalize can erase the only recovery authority for an adopted task whose initial persistence failed

A concrete cancel sequence remains:

1. A foreground future is adopted while the status directory is unavailable.
2. `write_initial` fails; adoption logs and tracks the already-running future without a status file (`background.rs:867-878`).
3. Cancel removes the `RunningTask` from the map and aborts it (`:1370-1376`).
4. Terminal persistence also fails. `persist_terminal_with_recovery` logs that it is retaining a tombstone, but the map entry is already absent; it can only launch an in-memory detached retry (`:1687-1736`).
5. If the process exits before storage recovers, the retry disappears. No initial or terminal file exists, so orphan reconciliation cannot discover the task.

Shutdown finalize has the same terminal state without even the detached retry: it drains the map, terminal persistence fails, and shutdown continues. This is narrower than round 1 and requires the specifically documented adoption-initial-write failure, but it directly violates gate 2 and is avoidable.

Fix cancel by keeping the removed `RunningTask` as an explicit tombstone when terminal persistence fails, or by changing the helper to accept/reinsert it atomically before returning failure. Record whether initial persistence succeeded so the ordinary durable-file case and exceptional no-file case are explicit. For shutdown finalize, do not report the no-file case as finalized: retry until a bounded shutdown deadline and then propagate an incomplete-finalization result to the coordinator, or persist a recovery marker in an independent durable location. Add failure-injection tests for adopted-initial-failure followed by cancel and shutdown finalize.

### Important

#### F04-R2-I1: terminal persistence health still is not propagated through manager results

Round-1 I3 remains nonblocking. The recovery loop materially reduces damage, and every failure is logged. Nevertheless, cancel returns `Ok(true)`, wrappers publish completion, and shutdown finalize returns its count even when terminal state is not yet persisted. Automated callers cannot distinguish durable completion from retry-pending or unrecoverable shutdown finalization. A structured persistence-pending/error result or bus event remains desirable.

### Minor

#### F04-R2-M1: the evidence README still names removed `read_lenient`

The code correctly removed dead `TaskStatusStore::read_lenient`, and test builds no longer warn about it. The evidence design section still says malformed files are handled by “`read_lenient`/`read_path`” at `evidence/F04/README.md:21`. Update the evidence text to name only current APIs/behavior.

#### F04-R2-M2: no manager test directly counts no-op progress publications

The I1 fix is clear from exhaustive `MutateOutcome` matching and has no alternate publish site in the method. The existing progress test covers one applied event, but no new regression test submits the exact same progress twice and asserts that the second call emits zero bus events. Add that targeted test to lock the restored behavior.

## Disposition of round-1 findings

| Round-1 finding | Disposition | Evidence |
|---|---|---|
| F04-B1 | **Partially fixed, still blocking** | Spawn fails closed; natural spawn/adoption completion retains tombstones and retries. Cancel/finalize still lose all recovery authority for adopted tasks whose permitted initial write failed. |
| F04-I1 | **Fixed** | `Unchanged` returns before progress publication; `Applied` reaches one publish call. |
| F04-I2 | **Fixed** | `mutate(false)` returns untouched `existing` persisted truth. All manager matches handle `Unchanged`. |
| F04-I3 | **Improved, remains nonblocking** | Failures are logged and retry-pending state is retained for natural completion, but manager APIs/events still do not communicate persistence health structurally. |
| F04-M1 | **Fixed in code** | `read_lenient` is removed and warning is gone. Evidence has one stale reference. |
| F04-M2 | **Fixed** | Store documentation now correctly claims reader atomicity, not crash durability/fsync. |

## Gate checklist

| Acceptance gate | Result | Evidence |
|---|---|---|
| No direct non-atomic status-file writes remain | **PASS** | Re-census found no direct status serialization/write in production `background.rs`; status writes remain centralized in the store. |
| Live task is not removed before terminal persistence succeeds or is durably recoverable | **FAIL** | Normal spawn/cancel/finalize and natural wrappers are now safe through a retained map entry or initial file. An adopted task may have no initial file, yet cancel removes it before terminal persistence and shutdown finalize drains it; terminal failure then leaves no durable recovery authority. |
| Status-serialization and write failures are surfaced, not swallowed | **PASS with qualification** | Store errors propagate; manager terminal paths log them and retry natural/cancel completion. Success-shaped manager results still hide persistence health from callers, but failures are not silent. |

## F05 handoff list

1. Crash/power-loss behavior around temp write/rename, including file and parent-directory fsync.
2. Malformed/truncated destination recovery and stale `*.tmp.<pid>` cleanup.
3. Cross-process same-task writers, PID reuse, and same-process independent store instances sharing one temp path.
4. Task-ID collision policy for existing `Running` files.
5. Windows replace-over-existing semantics and rename fault injection.
6. Structured persistence-health results/events for retry-pending terminal states.
7. Bounded retry lifecycle and shutdown coordination for retry tasks, including storage that never recovers.
8. Lock-map growth measurement/reclamation for long-lived high-cardinality processes.

The adopted-no-initial-file cancel/finalize defect is **not** handed to F05 because it is a live F04 gate-2 defect with a direct in-process fix.

## What I did not check

- I did not run `jcode-app-core`, the full workspace, Miri, Loom, sanitizers, or filesystem fault-injection frameworks.
- I did not run a process-crash fixture, cross-process writer race, Windows test, disk-full test, or power-loss durability test.
- I did not add a temporary adopted-initial-failure cancel/finalize test; the failure state follows directly from the explicit error branches and complete map-removal/write census.
- I did not inspect all downstream consumers of completion/progress bus events outside `jcode-base`.
- I did not modify implementation code.

## Confidence

**High (99%).** The major repair is real and both requested suites pass. The remaining blocker is a narrow but explicit composition of supported branches: adoption continues after initial persistence failure, while cancel/finalize remove it before a successful terminal write. With no status file and no retained map entry, neither the retry after process death nor orphan reconciliation can exist.

# Round 3: final re-review

## Verdict

**PASS.**

Reviewed exact commit `9c4c99897b88456257525d359f11fa357669c134` (`F04: fix round-2 blocker F04-R2-B1 - cancel tombstone + finalize policy`) at HEAD `9c4c99897b88456257525d359f11fa357669c134`.

Reviewer route: **OpenAI `gpt-5.6-sol`, high effort**.

The remaining live-runtime blocker is closed. Cancel now aborts in place while retaining the `RunningTask` map entry, releases the map read lock before terminal persistence, and delegates all pruning to `persist_terminal_with_recovery`. Immediate success and retry success prune idempotently. Terminal failure leaves a real tombstone even for an adopted task whose initial write failed.

`durable_record` accurately distinguishes tasks with an initial recovery record from the exceptional adopted/no-record state. Shutdown finalization remains necessarily lossy only when all of the following hold: an already-running foreground future was adopted, its initial write failed, its terminal write also fails, and the daemon is exiting while storage remains unavailable. The implementation now identifies and loudly reports that bound instead of claiming recoverability. I accept this as the honest shutdown policy for gate 2: blocking daemon shutdown indefinitely on unavailable persistence would preserve neither execution nor durable state and creates a worse system-level failure. During continued process operation, no live task is pruned without terminal persistence or an existing durable recovery record.

All three F04 acceptance gates are met. No blocking defect remains.

## Validation performed

### Exact source and evidence review

- Confirmed clean baseline and `HEAD == 9c4c99897b88456257525d359f11fa357669c134`.
- Read the complete `git show 9c4c99897` diff, updated `background.rs`, `background/model.rs`, the new manager test, and `evidence/F04/README.md`.
- Verified the updated evidence SHA-256 entry matches the README.
- Re-censused direct production status serialization/writes, terminal store calls, live-map removals, and all `is_live_task` consumers.

### Tests run

1. `scripts/dev_cargo.sh test -p jcode-base --lib background`
   - **38 passed; 0 failed; 0 ignored; 1146 filtered out; 0.66s**.
2. `scripts/dev_cargo.sh test -p jcode-base --lib`
   - **1181 passed; 0 failed; 3 ignored; 35.08s**.

### Cancel locking and concurrency

`cancel_with_grace` obtains a read guard, looks up the task, aborts the wrapper and adopted-original authority in place, copies every field needed by `TerminalSpec`, then explicitly drops the guard before awaiting `persist_terminal_with_recovery` (`background.rs:1393-1425`). The helper may acquire the task-map write lock only after terminal persistence returns, so there is no read-to-write self-deadlock.

Concurrent cancels are safe:

1. Multiple readers may find the same entry and abort handles repeatedly; Tokio abort handles are idempotent.
2. Each builds an equivalent cancellation terminal spec and drops its read lock before store IO.
3. The per-task store mutex serializes terminal writes. The first successful terminal write wins; later calls receive `AlreadyTerminal`.
4. Each success path removes by task ID. Repeated `HashMap::remove` calls are idempotent.
5. If multiple calls encounter broken storage, multiple retry loops are redundant but safe; the first recovery persists/prunes and later loops observe `AlreadyTerminal`, remove nothing, and exit.

The method retains its existing `Ok(true)` behavior whenever it found a live-map entry, including a concurrent finish/cancel race. First-terminal-wins means a completion that persisted just before cancellation may remain the recorded truth, which is appropriate race semantics.

### Cancel recovery test

`cancel_retains_tombstone_until_terminal_persistence_recovers` proves the intended failure path:

- it starts a long-running spawned task and confirms map membership;
- it moves the status directory away and replaces it with a file, forcing terminal temp writes to fail;
- `cancel` returns `true`, after which the test confirms the map entry remains;
- it restores the directory and waits for retry-driven pruning;
- it finally reads disk state and requires `Failed` with error `Cancelled by user`.

This is materially stronger than a happy-path ordering test because it proves retention during the actual post-retry error state and the complete recovery-to-prune transition.

### `durable_record` and shutdown finalization

- Spawn sets `durable_record: true` only after `write_initial` succeeded; initial failure returns before task creation/insertion (`background.rs:687-700`, `:814-818`).
- Adoption assigns the boolean directly from `write_initial`: true on success and false on its permitted failure (`:884-900`, `:1034-1038`). No other `RunningTask` construction exists.
- `finalize_non_detached` drains and aborts because daemon shutdown cannot leave futures running. If terminal persistence fails with `durable_record == true`, the initial `Running` file is valid next-boot orphan-sweep authority. If false, it emits an explicit `DATA LOSS:` error naming both failed writes and non-recoverability (`:512-531`).

The no-record shutdown corner cannot be made durable while the only persistence target remains unavailable. Retaining an in-memory tombstone cannot survive process exit, and an unbounded shutdown block is operationally worse. A bounded shutdown result propagated to the coordinator could improve observability, but the current loud policy is sufficient for F04's lifecycle gate rather than a reason to keep the daemon alive indefinitely.

### Tombstone visibility and consumers

During a terminal-persistence outage, `list`/`status` continue to show the durable file's `Running` state and `wait` continues waiting or times out. This is conservative: the persisted truth has not yet reached terminal, and claiming completion would violate the same persistence contract. Once retry lands, status becomes terminal and the map entry is pruned.

The two production `is_live_task` consumers are also conservative:

- the run-plan duplicate-driver guard continues blocking a replacement driver while a retry tombstone exists;
- self-dev reconciliation keeps a `Running` request alive while persistence recovery is pending.

After terminal persistence succeeds, the helper immediately removes the map entry. There is only a short success-to-remove scheduling window in which disk is terminal and `is_live_task` may still be true; this delays replacement/reconciliation rather than allowing duplicate work or corrupting state. `is_live_task` is explicitly best-effort and uses `try_read`, so transient false negatives under a map write lock predate this change and are already handled by caller-specific grace/reconciliation logic.

## Findings

### Blocking

None.

### Important

#### F04-R3-I1: delivery changes made during a terminal retry window can be overwritten by the stale terminal spec

`TerminalSpec` snapshots `notify`/`wake` before the first terminal attempt. `build_terminal_status` preserves prior progress and event history but writes delivery flags from that snapshot. If terminal persistence fails, a caller can update delivery on the retained tombstone; a later retry then reloads that new state but overwrites its delivery flags with the old spec values.

This does not defeat any acceptance gate: the write is still serialized/atomic, terminal truth remains correct, the task remains recoverable, and failures are surfaced. It does weaken the broader claim that serialized delivery/completion updates never lose each other. On retry, prefer delivery flags from `prior` when present, or refresh them from the retained task's watch sender before constructing each attempt. Add a failure-injection test that changes delivery while the tombstone is pending and checks the recovered terminal file.

### Minor

#### F04-R3-M1: evidence still references removed `read_lenient`

`evidence/F04/README.md:21` still names `read_lenient`, which was removed in the round-1 fixes. This is documentation drift only.

#### F04-R3-M2: concurrent cancel safety is source-proven but not regression-tested

The store mutex, first-terminal-wins behavior, handle abort semantics, and idempotent map removes make concurrent cancels safe. A two-caller test would still be useful to lock down `Ok(true)`/terminal-state behavior and ensure only one durable terminal outcome.

## Gate checklist

| Acceptance gate | Result | Evidence |
|---|---|---|
| No direct non-atomic status-file writes remain | **PASS** | Re-census found no production status serialization/write outside `TaskStatusStore`; all seven terminal call sites route through the store. |
| Live task is not removed before terminal persistence succeeds or is durably recoverable | **PASS** | Spawn fails closed; natural wrappers and live cancel retain real tombstones and prune only after store success; durable initial files cover normal cancel/finalize process death; the exceptional adopted/no-record shutdown case is explicitly bounded and loudly reported during daemon exit rather than silently called recoverable. |
| Status-serialization and write failures are surfaced, not swallowed | **PASS** | Store returns contextual errors and retries terminal writes; manager paths propagate or log them; retry helpers log initial/repeated failures and recovery; shutdown's irrecoverable no-record corner emits an explicit `DATA LOSS:` error. |

## F05 handoff list

1. Crash/power-loss behavior around temp write/rename, including file and parent-directory fsync.
2. Malformed/truncated destination recovery and stale `*.tmp.<pid>` cleanup.
3. Cross-process same-task writers, PID reuse, and same-process independent stores sharing one temp path.
4. Task-ID collision policy for existing `Running` files.
5. Windows replace-over-existing semantics and rename fault injection.
6. Structured persistence-health results/events for retry-pending and shutdown data-loss states.
7. Bounded retry lifecycle, duplicate retry-loop coalescing, and shutdown coordination when storage never recovers.
8. Preserve delivery updates made during terminal retry recovery.
9. Per-task lock-map growth measurement/reclamation for long-lived high-cardinality processes.
10. Direct tests for exact no-op progress publication count and concurrent cancel callers.

## What I did not check

- I did not run `jcode-app-core`, the full workspace, Miri, Loom, sanitizers, or an external filesystem fault-injection framework.
- I did not run a real process-crash, cross-process writer, Windows, disk-full, or power-loss test.
- I did not create an adopted-initial-failure shutdown fixture or capture the `DATA LOSS:` log; the branch and `durable_record` assignments were verified statically.
- I did not execute concurrent cancel callers; safety was established from the read-lock lifetime, store mutex, terminal precedence, and idempotent removal.
- I did not inspect every downstream bus-event consumer outside the identified live-task consumers.
- I did not modify implementation code.

## Confidence

**High (98%).** The exact remaining runtime-pruning defect is fixed, the failure-injection test exercises the important broken-storage state transition, every map removal and live-task consumer was rechecked, and both requested test scopes are green. Confidence is below 100% because shutdown data loss under permanently broken storage is an explicit accepted bound rather than a recoverable outcome, and cross-process/crash behavior remains deferred to F05.
