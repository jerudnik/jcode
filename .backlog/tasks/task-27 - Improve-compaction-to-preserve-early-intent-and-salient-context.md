---
id: TASK-27
title: Improve compaction to preserve early intent and salient context
status: Done
assignee:
  - '@jcode'
created_date: '2026-05-28 00:41'
updated_date: '2026-05-28 11:58'
labels:
  - exploratory
  - compaction
  - context
  - reliability
  - context-preservation
  - session
  - salience
dependencies: []
references:
  - crates/jcode-compaction-core/src/lib.rs
  - src
  - README.md
  - >-
    .backlog/tasks/task-27 -
    Improve-compaction-to-preserve-early-intent-and-salient-context.md:28@0aea41ac
  - 'commit:0aea41ac'
  - 'https://github.com/Opencode-DCP/opencode-dynamic-context-pruning'
priority: high
ordinal: 21000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Explore compaction strategies that preserve important earlier conversation context rather than relying primarily on recency, so user intent, constraints, decisions, and durable facts survive long sessions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Existing compaction triggers, cutoffs, summaries, and emergency behavior are reviewed against examples where early intent can be lost.
- [x] #2 Candidate salience signals are evaluated, such as explicit user goals, decisions, constraints, files changed, task state, tool outcomes, and recurring references.
- [x] #3 A testable design is proposed with regression fixtures or metrics showing important earlier messages are retained or summarized accurately.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Dispatch parallel research agents over DCP documentation, source code, and public discussions/issues.
2. Inspect JCODE compaction/context-memory surfaces enough to map external ideas to native integration points.
3. Synthesize candidate native designs, risks, and test fixtures against TASK-27/TASK-1/TASK-34/TASK-48/TASK-62/TASK-63.
4. Persist findings in a Serena MCP memory for future implementation work.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Started DCP comparative research and added the upstream DCP repository as a reference.

Dispatched four subagents covering DCP documentation, DCP code, public issues/discussions/releases, and local JCODE compaction integration points.

Wrote consolidated Serena memory `compaction/dcp_research_task27` with DCP design findings, JCODE opportunities, failure-mode cautions, salience signals, regression fixtures, and phased implementation recommendations.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Completed exploratory DCP research for TASK-27 and persisted the synthesis as Serena memory `compaction/dcp_research_task27`.

Findings:
- DCP’s strongest transferable idea is separating canonical session history from the provider-visible context projection.
- Native JCODE should evolve toward deterministic salience-aware context planning with protected facts, durable summary provenance, compaction ledgers, and debug visibility.
- High-value near-term work: strengthen summary/emergency prompts, preserve goals/constraints/ACs/tool outcomes, add early-intent regression fixtures, deduplicate repeated tool outputs, and prune stale errored tool inputs.
- DCP issue history warns against fragile model-facing boundary IDs, non-durable compaction metadata, opaque threshold triggers, provider-specific reasoning metadata leaks, and complex block lifecycle bugs.

Research sources included DCP docs, source structure, public issues/PRs/releases, and local JCODE compaction surfaces.
<!-- SECTION:FINAL_SUMMARY:END -->
