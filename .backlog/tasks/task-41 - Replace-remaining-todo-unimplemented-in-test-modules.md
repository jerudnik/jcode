---
id: TASK-41
title: Replace remaining todo!()/unimplemented!() in test modules
status: To Do
assignee: []
created_date: '2026-05-28 05:00'
labels:
  - reliability
  - tests
  - placeholders
  - server
  - tui
  - provider
dependencies: []
references:
  - 'docs/CODE_QUALITY_TODO.md:468-471@0aea41ac'
  - src/tui/app/tests.rs@0aea41ac
  - src/server/startup_tests.rs@0aea41ac
  - src/server/queue_tests.rs@0aea41ac
  - src/server/client_session_tests.rs@0aea41ac
  - src/provider/tests/auth_refresh.rs@0aea41ac
  - 'src/tui/app/tests/state_model_poke_03.rs:196'
  - '273'
  - '315'
  - '366'
  - 405@0aea41ac
  - 'src/tui/app/tests/support_failover/part_01.rs:49'
  - '70'
  - 95@0aea41ac
  - 'src/tui/app/tests/support_failover/part_02.rs:46'
  - '86'
  - '139'
  - '222'
  - '270'
  - '364'
  - '475'
  - 557@0aea41ac
  - 'commit:0aea41ac'
priority: medium
ordinal: 35000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: M

Replace todo!/unimplemented! placeholders in tests with real test bodies or remove tests that no longer apply. Auth-related test placeholders carry security weight.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Test files listed have zero todo!/unimplemented! placeholders
- [ ] #2 Each replaced placeholder is either a real test or a documented removal
- [ ] #3 Tests still pass
<!-- AC:END -->
