---
id: TASK-50
title: Wire LSP tool to a real LSP backend
status: To Do
assignee: []
created_date: '2026-05-28 05:04'
labels:
  - feature
  - tool
  - lsp
  - symbols
  - reliability
dependencies: []
references:
  - 'src/tool/lsp.rs:42-43@0aea41ac'
  - 'src/tool/lsp.rs:89-91@0aea41ac'
  - 'commit:0aea41ac'
priority: medium
ordinal: 44000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: L

Replace the LSP tool stub with a real implementation that performs symbol/operation calls against a configured LSP. Until then, the stub returns a non-integrated message.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 LSP tool can execute documented operations against a configured backend
- [ ] #2 Failure mode is graceful when no LSP is configured
- [ ] #3 Tests cover happy path and missing-backend path
<!-- AC:END -->
