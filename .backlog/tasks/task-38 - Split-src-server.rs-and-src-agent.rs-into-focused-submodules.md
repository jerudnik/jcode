---
id: TASK-38
title: Split src/server.rs and src/agent.rs into focused submodules
status: To Do
assignee: []
created_date: '2026-05-28 05:00'
labels:
  - code-quality
  - refactor
  - server
  - agent
  - orchestration
  - stream
  - interrupt
  - tool-exec
dependencies: []
references:
  - 'docs/CODE_QUALITY_TODO.md:34@0aea41ac'
  - 'docs/CODE_QUALITY_TODO.md:39@0aea41ac'
  - src/server.rs@0aea41ac
  - src/agent.rs@0aea41ac
  - 'commit:0aea41ac'
priority: medium
ordinal: 32000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: L

Continue splitting server.rs into focused submodules; split agent.rs into orchestration, stream, interrupt, and tool-exec modules.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 server.rs is split into at least three focused submodules
- [ ] #2 agent.rs has orchestration/stream/interrupt/tool-exec modules
- [ ] #3 Tests pass and public API surface is preserved or migration noted
<!-- AC:END -->
