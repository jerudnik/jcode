---
id: TASK-37
title: Decompose long functions (>100 LOC) outside oversized files
status: To Do
assignee: []
created_date: '2026-05-28 05:00'
labels:
  - code-quality
  - reliability
  - performance
  - refactor
  - functions
  - reload
  - memory
dependencies: []
references:
  - 'docs/CODE_QUALITY_TODO.md:215-288@0aea41ac'
  - 'commit:0aea41ac'
priority: medium
ordinal: 31000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: L

Break long functions identified by the umbrella backlog into named helpers. Prioritize reload/lifecycle and memory functions for reliability and perf wins.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Each listed function is under 100 LOC or has documented justification
- [ ] #2 Helpers have descriptive names and small surfaces
- [ ] #3 Tests cover the extracted helpers where behavior is non-trivial
<!-- AC:END -->
