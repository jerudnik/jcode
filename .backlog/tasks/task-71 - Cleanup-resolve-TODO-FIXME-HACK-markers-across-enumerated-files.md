---
id: TASK-71
title: 'Cleanup: resolve TODO/FIXME/HACK markers across enumerated files'
status: To Do
assignee: []
created_date: '2026-05-28 05:07'
labels:
  - code-quality
  - cleanup
  - markers
  - docs
  - stdin
  - ios
  - memory
dependencies: []
references:
  - 'docs/CODE_QUALITY_TODO.md:475-480@0aea41ac'
  - 'docs/CODE_QUALITY_AUDIT_2026-04-18.md:11@0aea41ac'
  - 'docs/CODE_QUALITY_AUDIT_2026-04-18.md:532@0aea41ac'
  - 'docs/CODE_QUALITY_AUDIT_2026-04-18.md:552@0aea41ac'
  - 'docs/IOS_CLIENT.md:429@0aea41ac'
  - src/tui/ui_tests/prepare.rs@0aea41ac
  - src/tui/ui_tests/tools.rs@0aea41ac
  - src/stdin_detect.rs@0aea41ac
  - 'commit:0aea41ac'
priority: low
ordinal: 65000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: S

Resolve deferred markers enumerated by the umbrella backlog and the audit doc; remove placeholder mock strings (e.g., the iOS client 'Review stale TODO items' mock).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Markers in enumerated files resolved or justified
- [ ] #2 No remaining placeholder mock strings in docs
- [ ] #3 Build/tests pass
<!-- AC:END -->
