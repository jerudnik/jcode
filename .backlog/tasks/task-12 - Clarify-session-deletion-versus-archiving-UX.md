---
id: TASK-12
title: Clarify session deletion versus archiving UX
status: To Do
assignee: []
created_date: '2026-05-18 04:41'
updated_date: '2026-05-28 04:57'
labels:
  - upstream
  - owner-interest
  - safety
  - UX
  - sessions
  - destructive-actions
  - data-safety
dependencies: []
references:
  - 'https://github.com/1jehuang/jcode/issues/207'
  - >-
    .backlog/tasks/task-12 -
    Clarify-session-deletion-versus-archiving-UX.md:25@0aea41ac
  - 'commit:0aea41ac'
priority: low
ordinal: 12000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Upstream #207 has owner discussion questioning deletion and suggesting archive may be the better user intent. Turn this into a safe session-management design that avoids accidental irreversible deletion.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Session management distinguishes archive/hide from permanent delete
- [ ] #2 Any destructive delete path requires explicit confirmation and documented artifact cleanup
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 Regression or validation added where applicable
- [ ] #2 Upstream issue/PR reference preserved in final notes
<!-- DOD:END -->
