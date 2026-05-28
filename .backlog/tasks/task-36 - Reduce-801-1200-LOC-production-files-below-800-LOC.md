---
id: TASK-36
title: Reduce 801-1200 LOC production files below 800 LOC
status: To Do
assignee: []
created_date: '2026-05-28 05:00'
labels:
  - code-quality
  - maintainability
  - refactor
  - auth
  - provider
  - tui
  - server
  - tool
  - reliability
  - security
dependencies: []
references:
  - 'docs/CODE_QUALITY_TODO.md:143-204@0aea41ac'
  - 'commit:0aea41ac'
priority: medium
ordinal: 30000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: L

Bring mid-sized production files below 800 LOC. Auth/permission files are security-sensitive; lifecycle/reconnect/reload files are reliability-sensitive.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Each listed file is below 800 LOC or has documented justification
- [ ] #2 Tests still pass
- [ ] #3 No regressions in compile time vs baseline
<!-- AC:END -->
