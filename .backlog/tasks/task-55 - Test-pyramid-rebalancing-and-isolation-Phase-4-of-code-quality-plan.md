---
id: TASK-55
title: Test pyramid rebalancing and isolation (Phase 4 of code-quality plan)
status: To Do
assignee: []
created_date: '2026-05-28 05:05'
updated_date: '2026-05-28 12:20'
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
  - 'Serena memory: compaction/dcp_research_task27'
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
- [ ] #5 Test isolation utilities cover context-management caches, ledgers, prompt/context rendering state, and provider-visible projection artifacts.
- [ ] #6 Snapshot/property coverage includes stable context ordering and XML/status-tagged context tier rendering where implemented.
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Expanded from TASK-27 research: test-pyramid work should include isolation and snapshot/property coverage for context projection, stable ordering, and context-intake caches.
<!-- SECTION:NOTES:END -->
