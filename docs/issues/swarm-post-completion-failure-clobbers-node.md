---
title: "A provider failure after complete_node flips a completed plan node to failed"
status: open
priority: high
owner: unassigned
opened: 2026-09-21
related:
  - crates/jcode-app-core/src/server/comm_control.rs
  - crates/jcode-plan/src/lib.rs
  - docs/issues/swarm-model-auto-fallback.md
---

# A provider failure after complete_node flips a completed plan node to failed

A worker that has already closed its node with `complete_node` and filed its
`report` can still take another turn (for example to answer a coordinator DM).
If that later turn fails at the provider, the member failure handler writes
`failed` over the node's `completed` status. The artifact survives in
`node_meta`, but the plan treats the node as failed, so every dependent stays
blocked and the coordinator has to reassign finished work to close it again.

## Evidence (2026-09-21, build ffa646a60)

Worker `session_poodle_1789980058941` on node `mcp-contract`:

- 04:58:57 `complete_node` succeeded (artifact 23,724 bytes persisted).
- 04:59:26 `report` recorded; `SWARM_LIFECYCLE new_status=succeeded`.
- 04:59:43 next turn: Z.AI `429 Too Many Requests` on `glm-5.3`;
  `SWARM_LIFECYCLE new_status=failed`, and `plan_status` then listed
  `mcp-contract` under "Failed (terminal without completing)".

`retry` was impossible because the assigned session had already been stopped
("Unknown session"). The node was closed by spawning a bookkeeping worker and
`reassign`ing it, which re-submits a summary of the existing artifact.

## Desired behavior

A node in a completed status is terminal for failure propagation: a later
member failure should update member status and be reported, but must not
change the node's status or discard its artifact. Failure recorded after
completion should be attributed to the member, not the work.

## Related

The same incident shows the openai-compatible provider failing a turn on the
first HTTP 429 with no bounded backoff. Four Z.AI-routed workers died that way
within ten minutes under moderate concurrency. Bounded retry for that provider
is a separate change and is not covered by the Gemini-only retry port.
