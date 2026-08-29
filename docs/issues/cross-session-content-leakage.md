---
title: "Todo view may bind to the wrong session"
status: open
priority: high
owner: unassigned
opened: 2026-08-17
---

# Todo view may bind to the wrong session

Two reports remain unexplained:

1. A third concurrent jcode session on its own worktree appeared to show content
   from another session.
2. A session in a separate `nix-config` checkout appeared to show todo
   content written by jcode sessions.

Neither report has been reproduced. The affected surface is also unknown: it may
have been a local pane, a remote attachment, an inline todo card, or the swarm
status panel.

## Todo storage and fan-out are excluded for the recorded cross-repository case

The todo tool stores and reads state by the exact `ToolContext.session_id`. A
successful write publishes `TodoUpdated` with the same session ID.

Both consumers preserve that identity:

- `crates/jcode-tui/src/tui/app/local.rs` ignores `TodoUpdated` unless
  `event.session_id == app.session.id`.
- `dispatch_swarm_todo_progress` in
  `crates/jcode-app-core/src/server/background_tasks.rs` first looks up the exact
  `event.session_id`. It updates only that `SwarmMember`, then asks
  `broadcast_swarm_status` to notify that member's swarm. The broadcast obtains
  recipients only from `swarms_by_id[source_swarm_id]`, and
  `fanout_session_event` sends only to attachments registered under each exact
  recipient session ID.

Swarm IDs normally come from the repository's Git common directory. An explicit
`JCODE_SWARM_ID` can intentionally join sessions from different directories, but
that did not happen in the recorded incident. The preserved control logs show:

- `session_tulip_1786957837407_383165b29b9720d0` joined
  the `nix-config` Git common directory at `09:10:38.124Z`.
- `session_rose_1786957987850_ed471b6456696972` joined
  the `jcode` Git common directory at `09:13:08.181Z`.
- The suspected jcode todo write was at `09:30:36Z`, and the operator noticed
  the nix-config display at `09:34:43Z`. Neither session changed swarms during
  that interval.

A regression test, `todo_progress_does_not_cross_swarm_boundaries`, models the
same two-swarm case. It requires the source swarm to receive the todo snapshot
and the other swarm to receive no event and no cached todo state.

The reported cross-repository display therefore did not come from todo storage,
the process-wide `TodoUpdated` bus, or swarm status fan-out as currently
implemented. Do not add a delivery fix without new evidence that contradicts
this routing proof.

## Remaining lead: the view may select the wrong session

Remote todo views load from `App::active_client_session_id`, which returns
`remote_session_id` in remote mode. Local views use `app.session.id`. If a client
is attached to the wrong session, or retains a stale remote pointer or
session-derived cache, it can correctly load another session's todos and still
present them in the wrong pane.

This fits both reports without requiring cross-session storage or delivery. It
is not yet proved because the original report did not identify the surface or
capture the view's bound session ID.

## Reproduction needed

Run at least three concurrent sessions: two jcode worktrees and one unrelated
repository. Give each a unique todo marker. For every pane or attachment under
test, capture:

1. the local session ID;
2. the remote session ID, if any;
3. the repository working directory and derived swarm ID;
4. the session ID embedded in the rendered todo payload;
5. which server attachment receives each session event.

A reproduction must show that a view whose immutable bound session is B renders
A's marker. If the bound ID has already changed to A, the defect is session
selection or attachment routing, not todo fan-out. The final fix must bind every
session-derived view and cache to one immutable session ID and reject incoming
updates for any other ID.
