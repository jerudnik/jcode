---
id: TASK-31
title: Eliminate production todo!()/unimplemented!() placeholders in jcode runtime
status: To Do
assignee: []
created_date: '2026-05-28 04:59'
labels:
  - reliability
  - code-quality
  - placeholders
  - panic
  - runtime
  - server
  - provider
  - tui
  - ambient
  - selfdev
dependencies: []
references:
  - 'docs/CODE_QUALITY_TODO.md:454-464@0aea41ac'
  - src/tui/ui_header.rs@0aea41ac
  - src/tui/app/remote.rs@0aea41ac
  - src/tool/mod.rs@0aea41ac
  - src/server/debug_command_exec.rs@0aea41ac
  - src/server/debug.rs@0aea41ac
  - src/server/client_state.rs@0aea41ac
  - src/server/client_comm.rs@0aea41ac
  - src/server/client_actions.rs@0aea41ac
  - src/provider/gemini.rs@0aea41ac
  - src/cli/selfdev.rs@0aea41ac
  - src/ambient/runner.rs@0aea41ac
  - 'commit:0aea41ac'
priority: high
ordinal: 25000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: L

Replace remaining production todo!/unimplemented! placeholders enumerated by the umbrella backlog with real implementations or explicit error returns. Covers server lifecycle, provider, TUI, ambient, and selfdev paths. Goal: zero production placeholders that can panic at runtime.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 No production code path returns todo!() or unimplemented!() (verified by ripgrep excluding tests)
- [ ] #2 Each replaced site has an error return or implementation with rationale in PR notes
- [ ] #3 CI fails if a new production todo!/unimplemented! is added (lint or grep-based check)
<!-- AC:END -->
