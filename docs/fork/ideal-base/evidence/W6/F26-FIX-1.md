# F26-FIX-1 — active-PID marker unregister — RETIRED (false node)

**State:** rejected as already satisfied. No code change.

The node claims "a session path never unregisters its active-PID marker, so a
leaked marker lingers up to the 24h liveness window and inflates the
concurrent-session count for headless sessions."

Both halves of that claim are false against shipped code.

## 1. A production unregister exists

Named differently than the node's proposed `mark_closed/end_session`, which is
why a name-based search missed it:

- `Session::mark_closed_and_persist` (`jcode-base/src/session.rs:1058`) calls
  `persist_terminal_state_with_observed_markers`, which calls
  `remove_session_pid_markers_if_unchanged` (`session.rs:1055`).
- Production callers, 7 total: `agent.rs:1036` (`Agent::mark_closed`, normal
  exit), `turn_execution.rs:208`, `client_disconnect_cleanup.rs:268`,
  `commands_review.rs:271`, `conversation_state.rs:633`, plus the crashed
  variant at `agent.rs:1065`.
- The headless path specifically: `Agent::mark_closed` is called at
  `ambient/runner.rs:399, 473, 941, 968, 992`.

## 2. Reclamation is by PID liveness, not a 24h window

`sweep_stale_pid_markers` (`active_pids.rs:362`) -> `remove_marker_if_stale`
-> `marker_contents_are_live` (`active_pids.rs:470`), which is
`pid_from_marker_contents(...).is_some_and(jcode_core::process::is_running)`.

A marker whose PID is dead is removed on the next sweep regardless of age, and
the sweep runs from session reconciliation (`session.rs:66`). So even the
crash path the node worries about does not wait 24h.

## 3. Empirical check on the live machine

Independent of reading the code:

```
~/.jcode/active_pids     : 2 markers, 2 live, 0 dead
~/.jcode/streaming_pids  : 2 markers, 2 live, 0 dead
```

If markers leaked as described, this machine (which has run thousands of
sessions, including headless ones) would show dead-PID residue. It shows none.

## Provenance

This node was asserted live earlier in the same session by another agent on
the basis that `unregister_active_pid` had only test callers, then retracted
when the production path above surfaced under a different name. The retraction
produced the standing rule: a node closed as already-fixed must cite a
production call site, never the absence of a grep hit.
