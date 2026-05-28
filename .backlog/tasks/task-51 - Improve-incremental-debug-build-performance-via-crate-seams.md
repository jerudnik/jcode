---
id: TASK-51
title: Improve incremental debug build performance via crate seams
status: To Do
assignee: []
created_date: '2026-05-28 05:04'
labels:
  - performance
  - build
  - cargo
  - dx
  - incremental
  - crate-seams
dependencies: []
references:
  - 'README.md:554@0aea41ac'
  - 'RELEASING.md:152-158@0aea41ac'
  - 'commit:0aea41ac'
priority: medium
ordinal: 45000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: L

Pursue refactors and additional crate seams to bring incremental debug builds from ~1 minute toward the 5-20s goal described in the README.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Measured incremental debug build time is documented before and after
- [ ] #2 At least one structural refactor lands and is justified
- [ ] #3 No release-build regressions
<!-- AC:END -->
