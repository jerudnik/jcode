# D01-FIX-2 evidence

Scope: wires 1 and 3 only. Wire 2 (RateLimitInfo) was split to D01-FIX-3 after
its scope was measured rather than assumed; see that node.

## Wire 1: UsageLog::record has a producer

Producer at `crates/jcode-app-core/src/ambient/runner.rs:811`, placed after the
cycle so the error path, which still spent tokens, is recorded too.

Control, planted and confirmed on disk before the exit code was read:

    -  scheduler.usage_log.record(usage.into_record());
    +  let _ = usage.into_record();

Result:

    ambient::runner::runner_tests::ambient_cycle_records_what_it_spent_in_the_usage_log
    panicked at runner_tests.rs:227
    assertion `left == right` failed:
      one completed cycle must leave exactly one usage record, found []
      left: 0, right: 1

Restored, `diff -q` byte-identical.

## Wire 3: active_user_sessions has a writer, shared with the runner

Writer at `crates/jcode-app-core/src/server/runtime.rs:351` (increment) and
`:374` (decrement). One `Arc` is allocated at `server.rs:633` and cloned into the
runner at `:634`, so there is one counter rather than two kept in agreement.

Control, a different mutation on a different file:

    -  AmbientRunnerHandle::for_server(Arc::clone(&client_count))
    +  AmbientRunnerHandle::for_server(Arc::new(RwLock::new(0)))

Result:

    server::startup_tests::ambient_pauses_while_a_user_client_is_connected
    panicked at startup_tests.rs:269
    ambient must pause while a user client is connected

Restored, `diff -q` byte-identical.

## The two controls fail on different assertions

This is the point of running both. Wire 1's control fails on a record-count
equality in `runner_tests.rs`; wire 3's control fails on a pause boolean in
`startup_tests.rs`. Under each control the OTHER test stays green, so neither
fix can be masking the other.

## What is still inert, stated rather than claimed

Re-measured on this tree, not inherited:

    budget_percent producers hardcoding None           2
      ambient_widget.rs:64, tui/mod.rs:2113
    calculate_interval non-test callers passing None   2
      runner.rs:751, runner.rs:901

So the adaptive resource calculator and the budget bar remain Inert, and
`docs/AMBIENT_MODE.md` still says so. Both are downstream of D01-FIX-3. Wire 1
is engineering-complete but its SCHEDULER payoff is gated behind that node: on
the `None` path production uses, 50 records of 1.8M tokens leave the interval at
7200s -> 7200s.

## Gates

Documentation was updated only after the behavior ran, and cites these controls.
`check_docs_references.py` OK (130 active, 0 machine-local, 0 stale-code-path).
`d01_scoreboard.sh` TOTAL 0.
