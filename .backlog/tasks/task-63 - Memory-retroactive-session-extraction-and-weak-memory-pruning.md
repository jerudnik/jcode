---
id: TASK-63
title: 'Memory: retroactive session extraction and weak-memory pruning'
status: To Do
assignee: []
created_date: '2026-05-28 05:06'
updated_date: '2026-05-28 12:20'
labels:
  - memory
  - reliability
  - pruning
  - retroactive
  - session-extraction
  - knowledge-graph
dependencies: []
references:
  - 'docs/MEMORY_ARCHITECTURE.md:769-774@0aea41ac'
  - 'docs/AMBIENT_MODE.md:936-938@0aea41ac'
  - 'commit:0aea41ac'
  - 'Serena memory: compaction/dcp_research_task27'
priority: low
ordinal: 57000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: L

Add retroactive session extraction for crashed/missed sessions, weak-memory pruning, cluster reorganization, cross-session relationship discovery, and embedding backfill.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Retroactive extraction runs over historical sessions
- [ ] #2 Weak memories pruned by documented thresholds
- [ ] #3 Cluster reorg + relationship discovery + embedding backfill in scheduled batch
- [ ] #4 Retroactive extraction captures salient context-management artifacts from historical sessions, including protected facts, compaction summaries, tool-output placeholders, trust-tier labels, and restore handles where available.
- [ ] #5 Weak-memory pruning thresholds incorporate provenance, trust tier, repetition/entropy signals, and whether a fact was explicitly protected or repeatedly referenced.
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Expanded from TASK-27 research: retroactive extraction/pruning should understand compaction summaries, protected facts, context placeholders, trust tiers, and repeated-reference salience.
<!-- SECTION:NOTES:END -->
