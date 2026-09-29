---
title: "Scheduled wakeups and background commands cannot reliably resume self-dev sessions"
status: open
priority: high
owner: maintainers
opened: 2026-08-29
related:
  - crates/jcode-provider-anthropic/src/lib.rs
  - crates/jcode-provider-core/src/anthropic.rs
  - crates/jcode-app-core/src/tool/ambient.rs
  - crates/jcode-app-core/src/tool/bash.rs
  - docs/issues/swarm-observability-status-and-wake-gaps.md
---

# Scheduled wakeups and background commands cannot reliably resume self-dev sessions

The normal ways for an agent to yield during a long external wait are broken in
the Claude OAuth self-development path. A scheduled wake is rejected because
the provider advertises one input contract while the internal tool executes a
different contract. Background shell commands have a related contract gap:
the provider does not expose the fields that ask the server to wake the session
when a command finishes.

The practical result is that an agent waiting for CI or another long-running
operation must hold its turn open with `sleep` and poll inline. That consumes an
active turn, hits the shell timeout ceiling, and still cannot guarantee that
work resumes without another human message.

## Verified narrowing (2026-09-04)

Live testing during the stall-guard investigation moved most of this issue
from suspected to resolved:

- **Wake delivery works end to end.** A background command with `wake: true`
  (set through `bg delivery`) completed after the originating turn had fully
  yielded, and the session was woken 0.4 seconds later with the completion
  delivered as a message. The seam this issue feared was broken is healthy
  when the flag is actually set.
- **The flag could never be set from the model side.** The curated Claude
  OAuth `Bash` schema hid `notify`/`wake` (`BashInput.wake` also defaults
  false), so agents blocked in `bg wait` instead of yielding. Fixed: the
  curated schema now advertises both fields, and the provider-boundary drift
  ledger shrank accordingly.
- **Blocking waits were then killed by the client stall guard**, which
  cancels any turn with no server events for `stream_idle_timeout + 30s`
  (630s observed four times on 2026-09-03, `trigger=stall_guard`). Fixed:
  the server now emits keepalive Pongs for as long as a tool executes, and
  the client Pong handler refreshes its activity clock (it previously did
  not, despite the #451 commit message assuming it did).

Still open, unchanged: the `ScheduleWakeup` payload contract mismatch
(`delaySeconds`/`reason`/`prompt` advertised vs `action`/`task`/
`wake_in_minutes` executed). The conformance drift ledger in
`crates/jcode-provider-anthropic/src/tool_conformance_tests.rs` pins the
exact divergence; the acceptance tests below still apply to that half.

## Live reproduction

Observed on 2026-08-29 in the running self-dev session `mouse`
(`session_mouse_1788015405020_aa829fb426495759`), build `fc76f06f2`, using
Claude Fable 5:

1. The session needed to check PR state again after 22 minutes.
2. The advertised `ScheduleWakeup` tool was called with all three fields its
   schema requires: `delaySeconds: 1320`, `reason`, and `prompt`.
3. The evidence log recorded `schedule` starting at `21:31:02Z` and failing in
   1 ms with `task is required for action=create`.
4. `ambient:queue` remained empty. `ambient:status` reported zero scheduled
   tasks and no next scheduled due time, so the failed call did not create a
   wake by another path.
5. The agent fell back to `sleep 540` followed by an inline poll. The session
   history explicitly records this as a workaround for the rejected scheduler
   and broken background pollers.

The background registry also returned 13 Bash task records owned by this
session, all with `wake: false`, including the detached record. Two prior inline
waits failed at exactly 600 seconds with exit 124, and the detached record was
cancelled without waking the session. These are not a viable replacement for a
durable scheduled resume.

## Immediate cause

`crates/jcode-provider-anthropic/src/lib.rs` curates `ScheduleWakeup` with the
legacy schema:

```text
delaySeconds, reason, prompt
```

The OAuth name mapper converts `ScheduleWakeup` to the internal tool name
`schedule`, but it does not translate the payload. `ScheduleTool` in
`crates/jcode-app-core/src/tool/ambient.rs` now expects:

```text
action, task, wake_in_minutes or wake_at, target, ...
```

Because `action` defaults to `create`, the legacy payload reaches
`execute_create` without `task` and deterministically produces the live error.

The same curated-provider seam affects background commands. The Claude OAuth
`Bash` schema advertises `command`, `timeout`, and `run_in_background`, while
the internal `BashInput` also has `notify` and `wake`. Since the model cannot
request `wake: true` through the advertised schema, background commands default
to notification without session wakeup.

## Required work

- Make the provider-advertised `ScheduleWakeup` contract identical to the
  internal `schedule` contract, or translate the legacy payload completely
  before execution. Do not keep two independently evolving schemas.
- Expose background completion wake semantics through the Claude OAuth Bash
  contract, or make `run_in_background` use a documented resume policy that
  does not depend on hidden fields.
- Return a schedule ID and due time only after the item is durably visible in
  the queue. Surface a clear delivery state through the debug status and queue
  commands.
- Deliver a scheduled or background completion wake exactly once, including
  across a self-dev reload, without requiring an active foreground tool call or
  a human message.
- Keep this distinct from the swarm completion-wake issue: this failure occurs
  before a normal self-scheduled wake is queued and also affects non-swarm
  background commands.

## Acceptance

- A provider-boundary test sends the exact schema advertised as
  `ScheduleWakeup` through OAuth name mapping and proves that the internal queue
  contains the requested task at the expected due time. The test must fail if
  either side changes without its adapter.
- A live self-dev test schedules a short wake, lets the originating turn become
  idle, observes the queued item through the debug socket, and proves the same
  session resumes exactly once when it becomes due.
- A live background-command test uses only fields advertised to the provider,
  finishes after the originating turn has yielded, and proves completion wakes
  the session without polling. Repeat across a self-dev reload.
- Failure tests cover malformed or incomplete payloads and assert that no queue
  item or false completion notification is created.
