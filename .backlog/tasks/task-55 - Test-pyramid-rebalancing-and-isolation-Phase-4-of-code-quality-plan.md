---
id: TASK-55
title: Test pyramid rebalancing and isolation (Phase 4 of code-quality plan)
status: To Do
assignee: []
created_date: '2026-05-28 05:05'
labels:
  - CI
  - reliability
  - tests
  - e2e
  - unit
  - property
  - golden-snapshots
  - isolation
dependencies:
  - TASK-1
  - TASK-42
references:
  - 'docs/CODE_QUALITY_10_10_PLAN.md:195-210@0aea41ac'
  - 'commit:0aea41ac'
priority: medium
ordinal: 49000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: L

Phase 4 of the 10/10 plan. Split e2e suites, add unit tests for parsing/protocol/state transitions, add snapshot and property tests, improve isolation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 e2e suite is split by feature
- [ ] #2 New unit tests added for parsing/protocol/state transitions
- [ ] #3 At least one snapshot and one property test added
- [ ] #4 Test isolation utilities documented
<!-- AC:END -->
