---
id: TASK-1
title: Stabilize global env and cache isolation in lib tests
status: To Do
assignee: []
created_date: '2026-05-18 04:38'
updated_date: '2026-05-28 04:56'
labels:
  - tests
  - reliability
  - CI
  - isolation
  - env
  - cache
  - flaky
dependencies: []
references:
  - src/tui/app/tests
  - src/provider
  - >-
    .backlog/tasks/task-1 -
    Stabilize-global-env-and-cache-isolation-in-lib-tests.md:26@0aea41ac
  - 'commit:0aea41ac'
priority: high
ordinal: 1000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The full lib suite still has order-dependent failures under parallel execution. Serial execution reduced failures from 17 to 2, and the remaining failures pass individually. Identify the narrowest shared global state/cache leaks and add local isolation guards without broad rewrites.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Parallel full lib suite no longer reports the known env/cache-related failures, or the remaining failures are documented with a minimal reproduction.
- [ ] #2 Any isolation helper added has targeted tests or is exercised by affected tests.
- [ ] #3 No test-only global locks are held longer than needed.
<!-- AC:END -->
