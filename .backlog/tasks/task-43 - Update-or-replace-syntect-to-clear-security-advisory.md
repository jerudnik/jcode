---
id: TASK-43
title: Update or replace syntect to clear security advisory
status: To Do
assignee: []
created_date: '2026-05-28 05:03'
labels:
  - security
  - dependencies
  - syntect
  - supply-chain
dependencies: []
references:
  - 'docs/SECURITY_DEPENDENCIES.md:12-20@0aea41ac'
  - 'commit:0aea41ac'
priority: high
ordinal: 37000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: M

Track syntect upgrades or replace syntect to resolve the open advisory tracked in SECURITY_DEPENDENCIES.md.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 syntect is upgraded or replaced
- [ ] #2 cargo audit reports no syntect-related advisory
- [ ] #3 Syntax-highlight features still work in TUI and tests
<!-- AC:END -->
