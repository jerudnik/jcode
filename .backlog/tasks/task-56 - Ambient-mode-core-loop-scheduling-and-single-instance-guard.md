---
id: TASK-56
title: 'Ambient mode core loop, scheduling, and single-instance guard'
status: To Do
assignee: []
created_date: '2026-05-28 05:05'
labels:
  - reliability
  - ambient
  - scheduling
  - single-instance
  - config
  - storage
dependencies: []
references:
  - 'docs/AMBIENT_MODE.md:925-927@0aea41ac'
  - 'docs/AMBIENT_MODE.md:929-930@0aea41ac'
  - 'commit:0aea41ac'
priority: medium
ordinal: 50000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: L

Implement the ambient agent loop (spawn, run, sleep), single-instance guard, fixed-interval scheduling with max ceiling, [ambient] config section, and storage layout described in AMBIENT_MODE.md.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Ambient loop runs on a configurable interval with a max ceiling
- [ ] #2 Single-instance guard prevents concurrent ambient runs
- [ ] #3 Config section is documented and loaded
- [ ] #4 Storage layout matches docs
<!-- AC:END -->
