---
id: TASK-53
title: Burn down dead code and unused symbols in TUI/memory/provider
status: To Do
assignee: []
created_date: '2026-05-28 05:04'
labels:
  - code-quality
  - dead-code
  - refactor
  - tui
  - memory
  - provider
dependencies: []
references:
  - 'docs/CODE_QUALITY_10_10_PLAN.md:140-145@0aea41ac'
  - 'commit:0aea41ac'
priority: medium
ordinal: 47000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: M

Phase 1 of the 10/10 plan: remove unused variables/methods/helpers, replace broad #![allow(dead_code)] with narrow scoped allows, delete abandoned code paths.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Identified dead code in TUI/memory/provider is removed or justified
- [ ] #2 Broad allow(dead_code) suppressions narrowed
- [ ] #3 Build and tests pass without new warnings
<!-- AC:END -->
