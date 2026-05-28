---
id: TASK-54
title: Largest-files decomposition (Phase 2 of code-quality plan)
status: To Do
assignee: []
created_date: '2026-05-28 05:05'
labels:
  - code-quality
  - refactor
  - maintainability
  - e2e
  - server
  - agent
  - provider
  - tui
dependencies:
  - TASK-35
  - TASK-36
  - TASK-38
  - TASK-39
references:
  - 'docs/CODE_QUALITY_10_10_PLAN.md:152-177@0aea41ac'
  - tests/e2e/main.rs@0aea41ac
  - src/server.rs@0aea41ac
  - src/agent.rs@0aea41ac
  - src/provider/mod.rs@0aea41ac
  - src/provider/openai.rs@0aea41ac
  - src/tui/ui.rs@0aea41ac
  - src/tui/info_widget.rs@0aea41ac
  - 'commit:0aea41ac'
priority: medium
ordinal: 48000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: L

Execute the Phase 2 decomposition plan. Coordinates with TASK-35/36/38/39 (umbrella splits) but tracks the plan-document priorities specifically.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Each Phase 2 file is decomposed per the plan or has a documented justification
- [ ] #2 Tests pass
- [ ] #3 Compile times measured before and after
<!-- AC:END -->
