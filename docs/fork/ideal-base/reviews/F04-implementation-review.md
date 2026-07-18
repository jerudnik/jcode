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
