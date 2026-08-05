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

## Re-verified on 2026-08-05 before re-disposition

This node moved `rejected -> superseded`, so its claims were re-derived against
the current tree rather than inherited. Node text in this program has been
falsified by measurement more than once, and the original evidence was written
against a crate layout that has since changed, so a re-read was mandatory rather
than ceremonial.

Every path in section 1 and 2 above was stale, and **all three claims still
hold** at the new locations:

| Claim | Then | Now | Holds |
|---|---|---|---|
| production unregister exists | `jcode-base/src/session.rs:1058` | same file, `mark_closed_and_persist` still calls `remove_session_pid_markers_if_unchanged` | yes |
| marker removal helper | `active_pids.rs:1055` | moved crate: `jcode-storage/src/active_pids.rs:227` | yes |
| headless callers | `jcode-base/.../ambient/runner.rs:399,473,941,968,992` | moved crate: `jcode-app-core/.../ambient/runner.rs:439,519,1035,1062,1086` | yes, still exactly 5 |
| liveness not a 24h window | `marker_contents_are_live` | `active_pids.rs:470`, still `pid_from_marker_contents(...).is_some_and(is_running)`; grep for `86400`/`24 * 60`/`hours(24)` in the sweep returns **nothing** | yes |

The crate move (`jcode-base` -> `jcode-app-core` and `jcode-storage`) is why a
line-number citation alone would have read as a false claim. The behavior is
unchanged.

Empirical check repeated on the same machine, now with a larger sample than the
original 2+2:

```
~/.jcode/active_pids     : 27 markers, 27 live, 0 dead
~/.jcode/streaming_pids  :  5 markers,  5 live, 0 dead
```

32 markers, zero dead-PID residue.

## Why `superseded` and not `rejected`

The disposition is unchanged in substance and this is not a re-opening. The
state name is being corrected because `rejected` and `superseded` are not
interchangeable to the railway:

    DEPENDENCY_COMPLETE = {accepted, authorization_blocked, superseded}

`rejected` is in `ALLOWED_STATES` but **not** in `DEPENDENCY_COMPLETE`, so a
`rejected` node can never satisfy a dependency. With W6's other ten children
accepted, this single node blocked W6 synthesis, which blocks W7, which blocks
`D01-FIX-3`/`D01-FIX-4`, which blocks `S01 -> S02 -> S03`: the entire signoff
tail. The deadlock was found by SIMULATING the `G02 -> accepted` checkpoint
before writing it and observing the runnable projection go empty, not by CI
catching it afterward.

`superseded` is also the more accurate word for what happened. This file's own
heading says *"RETIRED (false node)"* and *"already satisfied"*. Nothing was
verified and found wanting, which is what `rejected` implies; the node's premise
was superseded by production code that already existed under a different name.
