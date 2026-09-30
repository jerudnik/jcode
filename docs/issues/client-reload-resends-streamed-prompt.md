---
title: "Client reload can resend a user prompt the server already streamed"
status: open
priority: medium
owner: unassigned
opened: 2026-09-29
related:
  - crates/jcode-tui/src/tui/app/state_ui.rs
  - crates/jcode-tui/src/tui/app/remote/input_dispatch.rs
  - crates/jcode-tui/src/tui/app/remote/server_events.rs
---

# Client reload can resend a user prompt the server already streamed

`save_input_for_reload` (`state_ui.rs`, `resume_prompt`) persists an in-flight
plain user prompt (`!is_system`, `!auto_retry`) as restored input with
`submit_on_restore = true`. Every caller of that function is a client-only
handoff (`/client-reload`, `/restart`, the maintenance card); the server keeps
running the turn and the restarted client reattaches to it. If the server had
already streamed content for that prompt, the restored client submits it
again once idle.

The reconnect-queue-wake port (wave 3) added proven-delivery handling for
queued *continuations* (`is_system`) on disconnect and client reload, but left
`resume_prompt` alone as out of scope.

## Mitigation today

History dedup in `remote/input_dispatch.rs` (around the startup-submission
check) suppresses the resend only when the restored prompt has no images and
exactly equals the latest user message in the History payload;
`server_events.rs` clears the startup submission on that match. Exposure
remains for:

- a streamed prompt that carried images;
- a streamed prompt whose latest-user-message slot was displaced by a later
  injected or user message before the reload.

## Fix direction

Apply the same rule the continuation path uses: when
`App::pending_remote_delivery_is_proven()` is true at snapshot time, do not
persist the prompt for submission (persist it as plain restored input at most,
without `submit_on_restore`). Add a test beside
`test_save_input_for_reload_skips_streamed_continuation`.

## Evidence

Re-verification report for `automation/reconnect-queue-wake` at `3ede08ec9`
(gpt-6-astra, 2026-09-29), finding 4.
