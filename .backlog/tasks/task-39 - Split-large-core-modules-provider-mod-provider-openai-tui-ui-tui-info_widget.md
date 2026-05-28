---
id: TASK-39
title: >-
  Split large core modules: provider/mod, provider/openai, tui/ui,
  tui/info_widget
status: To Do
assignee: []
created_date: '2026-05-28 05:00'
labels:
  - code-quality
  - refactor
  - provider
  - tui
  - CI
dependencies: []
references:
  - 'docs/CODE_QUALITY_TODO.md:42-45@0aea41ac'
  - src/provider/mod.rs@0aea41ac
  - src/provider/openai.rs@0aea41ac
  - src/tui/ui.rs@0aea41ac
  - src/tui/info_widget.rs@0aea41ac
  - 'commit:0aea41ac'
priority: medium
ordinal: 33000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: M

Split the named large modules into smaller focused units. Tests must pass.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Each named module is split into focused submodules
- [ ] #2 Compile time does not regress materially
- [ ] #3 All tests pass
<!-- AC:END -->
