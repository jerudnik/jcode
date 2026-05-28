---
id: TASK-62
title: 'Memory: full graph-wide dedup, contradiction resolution, fact verification'
status: To Do
assignee: []
created_date: '2026-05-28 05:06'
updated_date: '2026-05-28 12:20'
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
  - 'Serena memory: compaction/dcp_research_task27'
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
- [ ] #5 Dedup/contradiction logic accounts for context artifacts such as duplicate tool outputs, repeated summaries, placeholders, skeletons, and provider-visible projections with source provenance.
- [ ] #6 Fact verification can distinguish verified facts from quarantined, stale, speculative, failed-tool, or superseded context.
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Expanded from TASK-27 research: graph-wide dedup and contradiction handling should include context artifacts and trust tiers, not only long-term semantic memories.
<!-- SECTION:NOTES:END -->
