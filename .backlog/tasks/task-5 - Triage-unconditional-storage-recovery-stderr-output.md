---
id: TASK-5
title: Triage unconditional storage recovery stderr output
status: To Do
assignee: []
created_date: '2026-05-18 04:38'
updated_date: '2026-05-28 04:57'
labels:
  - storage
  - ux
  - reliability
  - diagnostics
  - logging
  - UX
dependencies: []
references:
  - crates/jcode-storage/src/lib.rs
  - >-
    .backlog/tasks/task-5 -
    Triage-unconditional-storage-recovery-stderr-output.md:25@0aea41ac
  - 'commit:0aea41ac'
priority: low
ordinal: 5000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
jcode-storage emits unconditional stderr when JSON recovery falls back to backups. Confirm whether this is desirable user-facing diagnostics or should use structured logging/test-controlled reporters.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Decision documented as a Backlog.md decision or implemented with a small low-risk logging abstraction.
- [ ] #2 Corrupt-primary recovery remains visible enough for users to diagnose data issues.
<!-- AC:END -->
