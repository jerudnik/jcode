# F04 evidence: atomic serialized TaskStatusStore

Recorded: 2026-07-18. Implements the A2 persistence contract for background
task status files.

## Design

`crates/jcode-base/src/background/store.rs` (`TaskStatusStore`):

- **Atomicity**: same-directory temp file + rename; readers never observe
  torn JSON.
- **Serialization**: per-task async mutex around every read-modify-write and
  write; concurrent progress/delivery/completion updates cannot interleave.
- **Terminal precedence**: `write_terminal` is first-wins
  (`AlreadyTerminal` otherwise); `mutate` restores terminal truth fields
  (status/exit_code/error/completed_at/duration_secs) after any hostile
  mutation while still applying non-terminal fields (delivery flags,
  events); `write_initial` refuses to clobber a terminal file.
- **Surfaced failures**: serialization/IO errors are `Result`s with context.
  Terminal writes retry 3x then surface. Malformed files are distinguished
  from missing (`read` errors on corruption; `read_lenient`/`read_path` log
  instead of silently ignoring). A malformed file does not block terminal
  persistence (the terminal write is the recovery).

## Migration

Every status write in `BackgroundTaskManager` now routes through the store:
initial writes (`spawn_with_notify`, `adopt_with_options`,
`register_detached_task`), terminal writes (both wrapper completions,
cancel live/detached branches, `finalize_non_detached`, orphan/detached
finalize, refused-status), and serialized mutations (`update_progress`,
`update_checkpoint`, `update_delivery`). Zero direct
`fs::write`/`to_string_pretty` status writes remain in `background.rs`
(gate 1; verified by grep).

Terminal wrapper writes now read prior progress/event history INSIDE the
store's critical section (previously an unserialized pre-read raced
progress updates).

## Acceptance gates

1. "No direct non-atomic status-file writes remain": grep census, zero
   matches outside the store.
2. "Live task is not removed before terminal persistence succeeds or is
   durably recoverable": pruning still happens strictly after the terminal
   store write returns; on persistence failure after retries, the error is
   surfaced and the file remains reconcilable (orphan sweep) at next boot.
   Test `live_map_prunes_only_after_terminal_persistence` asserts the
   "Running file => live map" invariant under aggressive polling.
3. "Status-serialization and write failures are surfaced, not swallowed":
   store returns contexts; all manager call sites log errors at error
   level; test `write_failure_is_surfaced_not_swallowed` proves both
   initial and terminal failure surfacing.

## Review round 1 fixes (F04-implementation-review.md, FAIL at 4f1e5adfa)

- F04-B1 (blocking): terminal-persistence failure durability.
  - `spawn_with_notify` now FAILS CLOSED when the initial `Running` write
    fails: the task never starts (durable record is a spawn prerequisite).
    Adoption documents why it cannot fail closed (future already running)
    and tracks anyway for shutdown-finalize abortability.
  - `persist_terminal_with_recovery`: on terminal write failure the
    live-map entry is RETAINED as a visible tombstone (never a phantom
    "pruned but Running on disk" state) and a detached backoff retry loop
    prunes only once persistence lands. Used by both wrapper completions
    and the cancel live branch.
  - New failure-injection tests:
    `spawn_refuses_to_start_when_initial_persistence_fails`,
    `terminal_persistence_failure_retains_tombstone_then_recovers`
    (break-the-directory injection, heal, observe recovery + prune).
- F04-I1: `MutateOutcome::Unchanged` variant; no-op equivalent progress
  updates no longer publish duplicate bus events (pre-F04 behavior
  restored).
- F04-I2: `mutate` returning false now yields the PERSISTED state
  (`Unchanged(existing)`), never the closure's discarded in-memory
  mutations.
- F04-M1: dead `read_lenient` removed.
- F04-M2: `write_atomic` doc corrected (reader-atomic, not crash-durable;
  fsync hardening deferred to F05 if its gates require it).
- F04-I3 (nonblocking): terminal paths log-and-continue by design; the
  recovery loop now bounds the damage. Structured persistence-health events
  are F05-scope follow-up.

## Review round 2 fix (F04-R2-B1, FAIL at 10209b09c)

- Cancel now aborts IN PLACE: the `RunningTask` entry stays in the live map
  as a tombstone until `persist_terminal_with_recovery` lands the terminal
  write (immediately or via its retry loop), so a cancelled task can never
  end up with neither a map entry nor a durable record. Test
  `cancel_retains_tombstone_until_terminal_persistence_recovers`.
- `RunningTask.durable_record` tracks whether an initial `Running` file
  exists (always true for spawns, false only for adopted tasks whose
  permitted initial write failed). `finalize_non_detached` applies an
  explicit failure policy: with a durable record, the next-boot orphan
  sweep is the recovery authority; without one, the data loss is stated
  loudly (`DATA LOSS:` log) rather than silently shaped as success. The
  daemon is exiting, so no in-process retry loop can outlive it; this is
  the honest residual bound for the adopted+broken-storage corner.

## Tests

- Store: 6 tests incl. torn-JSON hammer
  (`concurrent_writers_never_tear_json_or_lose_terminal`, 4 writers x 25
  mutations racing a terminal write with a continuous parse-checking
  reader), terminal precedence vs hostile mutation, first-terminal-wins,
  missing-vs-malformed, clobber refusal, failure surfacing.
- Manager: `live_map_prunes_only_after_terminal_persistence`,
  `progress_and_delivery_survive_concurrent_terminal_completion`, plus the
  27 pre-existing background tests.
- Full: jcode-base 1181/1181, jcode-app-core 1126/1126 all green.
