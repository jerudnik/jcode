---
id: TASK-80
title: Prioritize next context-management evaluation techniques
status: To Do
assignee:
  - '@jcode'
created_date: '2026-05-28 13:23'
updated_date: '2026-05-28 13:41'
labels:
  - context
  - evaluation
  - research
dependencies:
  - TASK-79
references:
  - scripts/context_pipeline_eval.py
documentation:
  - docs/CONTEXT_PIPELINE_EVAL.md
priority: medium
ordinal: 73000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Catalog and prioritize additional context-management techniques beyond the initial P0 transform prototypes, including pruning, attention management, goal/task retention, retrieval, and memory-adjacent approaches. The output should guide future extensions to scripts/context_pipeline_eval.py without prematurely expanding runtime scope.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Context-management candidate techniques are grouped by reliability goal and implementation risk
- [ ] #2 Evaluation metrics are defined for pruning, attention management, goal retention, and memory-adjacent techniques
- [ ] #3 Recommended next prototype batch is identified with low-risk/high-reward techniques first
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Candidate techniques to test next:
- Goal/task retention ledger for active objective, constraints, acceptance criteria, and do-not rules.
- Recency plus importance scoring across role, recency, explicit constraints, task refs, and tool success/failure.
- Contradiction and supersession pruning for older assumptions invalidated by later evidence.
- Pinned facts/protected spans for user instructions, task IDs, file paths, decisions, and safety constraints.
- Attention preamble/context index summarizing what matters in the payload before long context.
- Context provenance and trust routing separating verified facts from failed tools, logs, and speculative assistant text.
- Lazy restore handles with targeted expansion only when the current turn references omitted artifacts.
- Working-state scratchpad regenerated from canonical facts instead of replaying stale reasoning-like context.
- Session-local fact memory, TTL/expiry for inferred facts, source-bound memories, and conflict-aware memory retrieval as memory-adjacent experiments.

Proposed near-term prototype batch: goal/task retention ledger, supersession pruning, attention preamble/context index, and lazy restore handles. These look lower-risk and directly measurable against protected-goal retention, stale/foreign retention, token savings, restore precision/recall, and ordering stability.

Consolidation done: the candidate list and recommended next prototype batch have been copied into docs/CONTEXT_PIPELINE_EVAL.md under 'Pending prototype/evaluation ledger', so TASK-79/doc remains the single main place to track completed results plus pending targets.

Research swarm refinement: prioritize canonical goal/task ledger from user/backlog/tool-success facts, runtime supersession pruning extending src/agent/context_pruning.rs, deterministic attention preamble/context index, lazy restore handles with content hashes/trust/supersession metadata, and source-bound supersedable protected spans. Added methodology thresholds to docs/CONTEXT_PIPELINE_EVAL.md.
<!-- SECTION:NOTES:END -->
