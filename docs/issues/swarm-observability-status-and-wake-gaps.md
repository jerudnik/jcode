---
title: "Swarm worker lifecycle status disagrees across surfaces, and completion wakes can go missing"
status: open
priority: medium
owner: unassigned
opened: 2026-08-28
related:
  - docs/issues/swarm-spawn-model-identity-mismatch.md
---

# Swarm worker lifecycle status disagrees across surfaces, and completion wakes can go missing

Observed 2026-08-27 while coordinating the provider-identity audit (coordinator
session `sheep`, worker `session_lizard_1787863669258_12b84aba3ada7f79`). Four
distinct observability defects, all reproduced in one session:

## 1. Missed completion wake

`await_members` for lizard/monkey resolved at `21:11:32Z`, but the coordinator
received nothing until the human prompted it at `21:20`, at which point the
await result and the worker's completion report arrived together. Nine minutes
of "all workers done" invisible to the agent that asked to be woken.

## 2. Status vocabulary disagreement

The same terminal worker (lizard) was simultaneously reported as:

- `crashed` by the resolved `await_members` result,
- `ready · idle 6m` with a delivered report by `swarm list`,
- `Active` in its persisted session file.

Its evidence log shows a clean final turn (`turn_finished ok` at `21:15:14`).
Nothing crashed. Lifecycle states need one source of truth and one vocabulary.

## 3. Unfocused inline/headless viewport reads as "wedged"

The operator observed lizard as stuck and reported it started moving "as soon
as I opened the window". The evidence log shows continuous provider
request/response cycles the whole time (max gap 83s waiting on a slow k3
response). The stall was purely a rendering artifact of an unfocused worker
viewport, but it is indistinguishable from a real hang without reading
evidence logs by hand. The viewport (or the member chip) should surface
last-activity age so a live worker never looks dead.

## 4. Report truncation mid-delivery

Lizard's final report was cut mid-sentence by the delivery pipeline ("[Report
truncated by jcode before delivery.]"), losing the tail of its fix
recommendations. The tldr + external-memory pattern worked around it, but
truncation of a structured final report silently drops exactly the content a
coordinator needs. Related history: docs/issues/cross-session-content-leakage.md
records the 240-char agent-to-agent lossiness of DMs; this is the report-path
sibling.

## Suggested direction

One lifecycle state machine owned by the server, consumed by await/list/UI;
wake delivery acknowledged or retried; worker chips carry last-evidence-event
age; report delivery either whole or explicitly chunked, never silently cut.
