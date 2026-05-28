---
id: TASK-32
title: Harden panic-prone unwrap/expect call sites in high-count files
status: To Do
assignee: []
created_date: '2026-05-28 04:59'
labels:
  - reliability
  - security
  - auth
  - code-quality
  - error-handling
  - unwrap
  - expect
  - provider
  - server
  - tool
dependencies: []
references:
  - 'docs/CODE_QUALITY_TODO.md:292-430@0aea41ac'
  - src/tool/communicate.rs@0aea41ac
  - src/build.rs@0aea41ac
  - src/provider/openai.rs@0aea41ac
  - src/auth/cursor.rs@0aea41ac
  - src/auth/codex.rs@0aea41ac
  - src/server/comm_control.rs@0aea41ac
  - 'commit:0aea41ac'
priority: high
ordinal: 26000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: XL

Reduce panic-prone calls in the highest-count production files identified by the umbrella backlog (tool/communicate, build, provider/openai, auth/cursor, auth/codex, server/comm_control). Auth files carry security weight. Prefer typed errors over panics; preserve diagnostics.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Each top-6 file reduces unwrap/expect/panic!/todo!/unimplemented! count by at least 50% or has documented justification per remaining call
- [ ] #2 New error variants are added where typed propagation is needed
- [ ] #3 Regression tests cover the most reachable panic paths
<!-- AC:END -->
