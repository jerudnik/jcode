---
id: TASK-33
title: Improve production error context for provider streams and lifecycle
status: To Do
assignee: []
created_date: '2026-05-28 04:59'
labels:
  - reliability
  - error-handling
  - provider
  - streaming
  - reload
  - socket
  - diagnostics
dependencies: []
references:
  - 'docs/CODE_QUALITY_TODO.md:49-52@0aea41ac'
  - 'docs/CODE_QUALITY_10_10_PLAN.md:179-193@0aea41ac'
  - 'commit:0aea41ac'
priority: high
ordinal: 27000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: L

Classify retryable vs user-facing vs invariant failures; add error context around provider-stream parsing and reload/socket lifecycle. Output errors that humans and the TUI can act on.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Provider stream parse errors include offset and a snippet
- [ ] #2 Reload/socket lifecycle errors include the prior state and the requested transition
- [ ] #3 Tests cover at least one retryable, one user-facing, and one invariant failure per surface
<!-- AC:END -->
