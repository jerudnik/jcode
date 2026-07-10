<!-- Promoted 2026-07-10 from the closed notes workstream projects/jcode/proposals/orchestration-hardening/_attachments/ (swarm loop safety runbook). Durable design/runbook home is here; PM history remains in notes archive. -->

---
title: Swarm loop safety net
type: note
created: 2026-07-04
updated: 2026-07-04
---

# Swarm loop safety net

Operational runbook for the recursive research→build→verify loop, derived
from the failure modes actually hit during the 2026-07-04 exemplar-tool spike.

## Principle

Every seam in the loop needs (a) a detector, (b) a bypass, (c) a backstop
timer. Never trust a status transition without evidence.

## Observed failure modes and recoveries

### F1 — driver stall (`run_plan` can't bind workers)
- Symptom: `run_plan stalled: runnable task(s) could not be assigned`.
- Detect: task fails within seconds; workers show `ready`/idle in `swarm list`.
- Recover: drive manually — `assign_task` + `start_task` with **full session
  IDs** (friendly names may not resolve).

### F2 — premature "all done" wake
- Symptom: `await_members` fires while a worker is mid-task; its "report" is
  a truncated thought, not findings.
- Detect: report reads mid-sentence; `plan_status` still shows node active.
- Recover: re-check `plan_status`, re-await only the unfinished sessions.
- Note: a later legitimate wake usually follows; treat wakes as hints, state
  lives in `plan_status`.

### F3 — work done, node never completed (owner-only completion)
- Symptom: worker reports success but node stays `active`, blocking children.
- Detect: `plan_status` active + worker `ready`/idle.
- Recover: coordinator cannot `complete_node` for others — DM the worker
  with explicit instructions to file `complete_node` with a typed artifact.

### F4 — coordinator-state desync
- Symptom: `Only the coordinator can assign tasks` while `swarm list` shows
  you as coordinator.
- Recover: `resync_plan`; if still wedged, bypass the plan API entirely —
  send the full task spec by DM (delivery=wake) and treat awaits as the
  dependency mechanism. The DAG is bookkeeping; the work is the point.

## Invariants (the actual net)

1. **Evidence gate**: a node counts as done only with a typed artifact
   (findings/evidence/confidence/what-not-checked). Check `summary` (tool-call
   log) when a report smells wrong.
2. **Backstop timer**: every background await gets a scheduled wakeup at
   ~90% of its timeout that re-checks `plan_status` idempotently. A missed
   wake must never orphan the loop.
3. **Verification is a separate agent** from implementation, always.
4. **Timebox explore nodes** (they are the unbounded-loop risk).
5. **Full session IDs everywhere**; friendly names are display sugar.
6. **Coordinator persists state outside the swarm**: seed specs, artifacts,
   and rulings get written to disk/git as they land, so a total swarm loss
   costs only the in-flight node, not the run.
7. Prefer bifrost MCP search tools for explore nodes (websearch scraping is
   the flakiest dependency observed).

## Status of the F1-F5 failure modes (2026-07-04, orchestration-hardening)

All five modes above now have failing-test reproductions and fixes on the
fork (`jerudnik/jcode` branches `orch/f5-name-resolution`,
`orch/failure-scoreboard`; test file
`crates/jcode-app-core/src/server/comm_control_tests/failure_scoreboard.rs`):

- F1: stale `assigned_to` is reclaimed automatically when the assignee left
  the swarm (queued items only; running items remain salvage territory).
- F2: `await_members` no longer counts a `ready`/`completed` status as done
  while the member's plan task is non-terminal. Wakes are still hints;
  keep the backstop timer (invariant 2) because the fix trades premature
  wakes for timeout-bounded waits when plan state drifts.
- F3: coordinators can `complete_node` for a departed owner (engine op
  `take_over_node` + membership-gated policy). The DM-the-worker recovery
  above is no longer the only path.
- F4: `resync_plan` now repairs the coordinators map from the member role,
  so the documented recovery actually recovers.
- F5: friendly names resolve on assign/task-control paths (shared resolver
  with the DM path). Invariant 5 relaxes to "friendly names OK when
  unique; IDs when ambiguous".

New specimen (local-model workers): small models may call tools outside
the allowed set and abort the subagent. Detector: subagent errors with
"Tool 'x' is not allowed". Bypass: name the allowed tools explicitly in
the task prompt (see experiments/local-model-routing.md).

## W2 update (2026-07-05): awaits wake from the control log

`await_members` no longer relies on the in-memory broadcast channel as its
wake source. The watcher anchors a durable byte cursor in the per-swarm
control log (`jcode-swarm-state/<id>.control.jsonl`) and re-checks on
wake-relevant events past it (ArtifactFiled, terminal TaskStatusChanged,
(un)assignment, member lifecycle). Consequences for operators:

- The F2 "lost wake" class (lagged/dropped broadcast while a worker
  finished) is retired structurally: light-mode auto-completes and salvage
  completions wake awaits even when no broadcast fires.
- Invariant 2 (backstop timer) SURVIVES but is demoted from "must have"
  to "cheap insurance": deadline handling is unchanged, and plan-state
  drift is still possible.
- Completion summaries now flag LOW-CONFIDENCE artifact evidence by node
  id and point at inject_gap. Treat that as a routing instruction, not
  decoration: the deep gate will refuse to pass over those nodes.
- Legacy pending awaits (pre-W2 state files) resume by re-anchoring their
  cursor at the current log tail; they cannot spuriously wake on
  pre-await history.
