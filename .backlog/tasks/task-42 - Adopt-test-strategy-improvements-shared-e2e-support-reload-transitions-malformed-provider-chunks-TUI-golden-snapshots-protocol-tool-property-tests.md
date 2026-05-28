---
id: TASK-42
title: >-
  Adopt test-strategy improvements: shared e2e support, reload transitions,
  malformed provider chunks, TUI golden snapshots, protocol/tool property tests
status: To Do
assignee: []
created_date: '2026-05-28 05:00'
updated_date: '2026-05-28 12:20'
labels:
  - reliability
  - CI
  - tests
  - e2e
  - golden-snapshots
  - property-tests
  - reload
  - provider
dependencies: []
references:
  - 'docs/CODE_QUALITY_TODO.md:56-60@0aea41ac'
  - 'commit:0aea41ac'
  - 'Serena memory: compaction/dcp_research_task27'
priority: medium
ordinal: 36000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: L

Improve the test pyramid as described by the umbrella backlog. Reload coverage is high-priority within this cluster.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Shared e2e support module exists
- [ ] #2 Reload-transition tests added
- [ ] #3 At least one TUI golden snapshot test exists
- [ ] #4 At least one property test exists for protocol/tool parsing
- [ ] #5 Malformed provider chunks are covered
- [ ] #6 Property or fixture tests cover context-intake decisions such as raw vs skeleton vs summary vs placeholder vs reject.
- [ ] #7 Fixtures cover early-intent retention, trust-tier/quarantine labels, duplicate tool-output pruning, oversized/binary gatekeeping, and restore-handle placeholders.
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Expanded from TASK-27 research: shared test strategy should include context-management fixtures/property tests for boundary inspection, pruning, trust tiers, and early-intent preservation.
<!-- SECTION:NOTES:END -->
