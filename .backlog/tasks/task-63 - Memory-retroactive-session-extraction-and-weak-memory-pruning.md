---
id: TASK-63
title: 'Memory: retroactive session extraction and weak-memory pruning'
status: To Do
assignee: []
created_date: '2026-05-28 05:06'
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
<!-- AC:END -->
