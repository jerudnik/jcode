---
id: TASK-35
title: Split oversized production files >1200 LOC
status: To Do
assignee: []
created_date: '2026-05-28 05:00'
labels:
  - code-quality
  - maintainability
  - refactor
  - server
  - provider
  - session
  - lifecycle
  - tool
dependencies: []
references:
  - 'docs/CODE_QUALITY_TODO.md:90-139@0aea41ac'
  - src/server/comm_control.rs@0aea41ac
  - src/tool/communicate.rs@0aea41ac
  - src/session.rs@0aea41ac
  - src/server/client_lifecycle.rs@0aea41ac
  - src/provider/openai.rs@0aea41ac
  - src/provider/gemini.rs@0aea41ac
  - 'commit:0aea41ac'
priority: medium
ordinal: 29000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: XL

Break the top oversized production files below 1200 LOC by extracting focused submodules. Server/lifecycle and tool paths are reliability-sensitive.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Each listed file is below 1200 LOC or has documented justification
- [ ] #2 No behavior changes (covered by existing tests + new module-boundary tests where helpful)
- [ ] #3 Public API surface preserved or migration noted
<!-- AC:END -->
