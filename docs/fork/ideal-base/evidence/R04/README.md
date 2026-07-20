# R04 incident: reload drain misclassified as accept-loop failure

Recorded: 2026-07-20. Incident time: 2026-07-19 23:02 local (2026-07-20 03:02 UTC).

## Summary

A non-forced `jcode debug reload` (promote + reload of `9e786069e-dirty`)
was issued while session `fish` had an active streaming turn. The daemon
(`brook`, pid 2580, `607d3cbad-dirty`) began the reload drain, and the drain's
own intake cancellation stopped the accept loops. The `run()` select
interpreted the accept-loop exit as a crash and upgraded
`reload -> accept-loop-failure` (priority 4 > 3). The upgraded 2s drain budget
expired against the in-flight `provider-turn:streaming-turn` lease, the reload
exec was refused with `ShutdownInProgress(AcceptLoopFailure)`, and the daemon
exited code 45 **without exec'ing a successor**. The attached TUI retried the
dead socket for ~3 minutes (attempts 1-10) until a manual `jcode --resume fish`
spawned a fresh server (`stadium`), after which the TUI re-exec'd into the new
build and recovered.

## Root cause

`ServerRuntime::intake_cancel_token()` returns the *same* root cancellation
token (`tasks.cancellation`) that the accept loops watch
(`crates/jcode-app-core/src/server/runtime.rs:323`, accept loops child-token it
at `runtime.rs:158,198`). `ShutdownCoordinator::begin_reload_drain` cancels it
by design ("F02-B3: reload Draining stops intake",
`crates/jcode-app-core/src/server/shutdown.rs:982`). But the `tokio::select!`
in `Server::run` (`crates/jcode-app-core/src/server.rs:2263`) treats *any*
`main_handle`/`debug_handle` completion as a failure and calls
`accept_loop_failure_terminal()`, which upgrades the coordinator to
`AcceptLoopFailure`. The reload drain loop observes the upgraded reason
(`shutdown.rs:992`) and aborts the handoff.

The race is only visible when the reload drain has to *wait* (an active
drain-blocking lease). An idle daemon drains instantly and execs before the
select polls the finished accept-loop handles, which is why prior reloads
(D023) succeeded.

## Evidence

- `incident-log-excerpt.txt`: log lines from `~/.jcode/logs/jcode-2026-07-19.log`
  showing the signal, drain start, upgrade, abandoned lease, refused reload,
  exit 45, client retry storm, and eventual manual recovery.
- Durable exit marker left by the incident:
  `~/.jcode/state/shutdown-watchdog.json` =
  `{"event":"cancelled","reason":"accept-loop-failure","pid":2580,...}`.

## Fix shape (for the implementing worker)

`run()` must distinguish "accept loop exited because shutdown/drain cancelled
it" from "accept loop crashed". Options: check
`shutdown::coordinator().has_begun()` before upgrading; or have the accept
loop return a typed exit (Cancelled vs Failed). Regression fixture: F03-style
in-process test that holds a drain-blocking lease, begins a reload drain, and
asserts the coordinator reason remains `Reload` after the accept loops wind
down.

## Relationship to accepted nodes

Reopen trigger for D022 ("F08 integrated gate finding a reload-path
regression") fired early via live use. R01's own gates were about target
selection and publishing, not this drain/select race; R01 remains accepted.
