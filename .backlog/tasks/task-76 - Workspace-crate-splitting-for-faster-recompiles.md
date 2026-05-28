---
id: TASK-76
title: Workspace crate splitting for faster recompiles
status: To Do
assignee: []
created_date: '2026-05-28 05:07'
labels:
  - performance
  - build
  - workspace
  - crates
  - recompilation
dependencies:
  - TASK-51
references:
  - 'RELEASING.md:152-158@0aea41ac'
  - 'commit:0aea41ac'
priority: low
ordinal: 70000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: L

Plan and execute workspace crate splitting (beyond current crate split) targeting ~1 min for small changes.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Workspace splitting plan documented
- [ ] #2 At least one new crate seam landed
- [ ] #3 Incremental small-change build time measured before/after
<!-- AC:END -->
