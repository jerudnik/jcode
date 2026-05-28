---
id: TASK-74
title: Implement ambient duplicate-memory detection
status: To Do
assignee: []
created_date: '2026-05-28 05:07'
labels:
  - ambient
  - memory
  - deduplication
  - similarity
  - embeddings
dependencies: []
references:
  - 'src/ambient/prompt.rs:98-100@0aea41ac'
  - 'commit:0aea41ac'
priority: low
ordinal: 68000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: M

Replace the placeholder note in src/ambient/prompt.rs with embedding-similarity-based duplicate-memory detection during the ambient cycle.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Duplicate candidates are flagged by similarity threshold
- [ ] #2 Threshold is configurable
- [ ] #3 Tests cover at least one duplicate and one non-duplicate case
<!-- AC:END -->
