# R04 implementation

Implemented at `c71628498` (worker draft salvaged and validated by the
coordinator after the delegating tool call was cancelled by the TUI stall
guard at 630s; see open questions).

## Changes

- `crates/jcode-app-core/src/server/shutdown.rs`
  - `AcceptLoopExitDisposition` + `ShutdownCoordinator::classify_accept_loop_exit`:
    an accept-loop exit observed after the coordinator has begun a drain is
    the drain's own intake cancellation and must await the terminal outcome;
    an exit while still `Running` is a genuine failure. Soundness argument in
    the doc comment: both `begin` and `begin_reload_drain` flip the phase off
    `Running` under the state lock before calling `cancel_intake`.
  - Startup beacon: `record_server_beacon_start` writes
    `~/.jcode/state/server-beacon.json` `{pid, started_at, version}` at server
    startup; `finalize_server_beacon` adds `{ended_at, reason}` when the
    coordinator publishes `Cleaned` (watchdog-enabled coordinators only, so
    in-process test coordinators cannot clobber a real daemon's beacon).
    `beacon_indicates_hard_crash(beacon, pid_alive)` gives the positive
    post-mortem classification: unfinalized + dead pid = hard crash.
  - Reload handoffs never finalize: exec keeps the pid and the successor
    overwrites the beacon at its own startup.
- `crates/jcode-app-core/src/server.rs`
  - `accept_loop_failure_terminal` replaced by `accept_loop_exit_terminal`,
    which consults `classify_accept_loop_exit` before escalating; beacon
    start recorded at the socket-listening announcement.
- `crates/jcode-app-core/src/server/shutdown_fixture_tests.rs`
  - `accept_loop_exit_during_reload_drain_does_not_upgrade_reason` (gate 1):
    holds a drain-blocking lease, begins the reload drain, simulates the
    accept-loop exit, asserts disposition `AwaitTerminal` and driving reason
    stays `Reload` through lease release.
  - `accept_loop_exit_during_termination_drain_awaits_terminal`: same
    classification holds for non-reload drains.
  - `accept_loop_exit_while_running_still_fails_with_code_45` (gate 2).
  - Beacon unit tests (gate 3): startup write, clean-exit finalize,
    pid-mismatch and double-finalize no-ops, hard-crash classification.

## Validation

- `scripts/dev_cargo.sh test -p jcode-app-core --lib shutdown`:
  42 passed, 0 failed.
- `scripts/dev_cargo.sh test -p jcode-app-core --lib server::`:
  485 passed, 0 failed.
- `scripts/dev_cargo.sh fmt --all -- --check`: clean after formatting
  (pre-existing drift in six unrelated files committed separately as
  `style:` at `67319de9f`).

## Acceptance gates

1. Reload with held drain-blocking lease completes the handoff path instead
   of upgrading: covered by the reload-drain fixture (reason stays `Reload`,
   disposition `AwaitTerminal`).
2. Genuine accept-loop failure still upgrades and exits 45: covered.
3. Hard crash positively detectable from durable state: beacon tests cover
   the unfinalized+dead-pid classification; no live SIGKILL run performed.

## Open questions

- The delegated worker was cancelled at 630s by the TUI stream-stall guard
  ("Tool 'subagent' interrupted by server reload" is a mislabel; the log
  shows `trigger=stall_guard`). Its uncommitted tree survived and was
  reviewed, validated, and committed by the coordinator. Follow-ups filed:
  stall guard vs long synchronous subagent calls, the misleading interrupt
  label, and D021 background detachment.
- `beacon_indicates_hard_crash` currently has its only callers in tests and
  the sentinel scripts read the JSON directly; wiring it into `jcode doctor`
  is a candidate follow-up.
