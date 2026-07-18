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

## Tests

- Store: 6 tests incl. torn-JSON hammer
  (`concurrent_writers_never_tear_json_or_lose_terminal`, 4 writers x 25
  mutations racing a terminal write with a continuous parse-checking
  reader), terminal precedence vs hostile mutation, first-terminal-wins,
  missing-vs-malformed, clobber refusal, failure surfacing.
- Manager: `live_map_prunes_only_after_terminal_persistence`,
  `progress_and_delivery_survive_concurrent_terminal_completion`, plus the
  27 pre-existing background tests.
- Full: jcode-base 1176+2, jcode-app-core 1126 all green.
