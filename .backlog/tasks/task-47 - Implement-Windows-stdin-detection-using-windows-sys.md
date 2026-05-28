---
id: TASK-47
title: Implement Windows stdin detection using windows-sys
status: To Do
assignee: []
created_date: '2026-05-28 05:04'
labels:
  - reliability
  - platform
  - windows
  - stdin
  - cross-platform
dependencies: []
references:
  - 'crates/jcode-core/src/stdin_detect.rs:411@0aea41ac'
  - 'commit:0aea41ac'
priority: medium
ordinal: 41000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: M

Replace the TODO at line 411 with a windows-sys based implementation; ensure non-Unix behavior is correct.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 stdin detection works correctly on Windows for piped/tty inputs
- [ ] #2 Tests cover both branches
- [ ] #3 No regression on Unix paths
<!-- AC:END -->
