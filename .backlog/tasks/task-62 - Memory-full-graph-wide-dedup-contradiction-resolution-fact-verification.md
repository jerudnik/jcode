---
id: TASK-62
title: 'Memory: full graph-wide dedup, contradiction resolution, fact verification'
status: To Do
assignee: []
created_date: '2026-05-28 05:06'
labels:
  - memory
  - deduplication
  - contradiction
  - fact-verification
  - reliability
dependencies: []
references:
  - 'docs/MEMORY_ARCHITECTURE.md:765-768@0aea41ac'
  - 'docs/AMBIENT_MODE.md:933-935@0aea41ac'
  - 'commit:0aea41ac'
priority: low
ordinal: 56000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: L

Implement graph-wide similarity-based memory merging, redundancy/dedup, contradiction resolution across the full graph, and fact verification against the codebase.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Dedup runs over the full graph
- [ ] #2 Contradictions are surfaced with sources
- [ ] #3 Facts are verifiable against current code
- [ ] #4 Embeddings backfill where missing
<!-- AC:END -->
