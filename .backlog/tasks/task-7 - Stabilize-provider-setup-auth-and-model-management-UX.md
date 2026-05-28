---
id: TASK-7
title: 'Stabilize provider setup, auth, and model management UX'
status: To Do
assignee: []
created_date: '2026-05-18 04:41'
updated_date: '2026-05-28 04:57'
labels:
  - upstream
  - owner-interest
  - security
  - auth
  - provider
  - model-picker
  - UX
  - reliability
dependencies: []
references:
  - 'https://github.com/1jehuang/jcode/issues/177'
  - >-
    .backlog/tasks/task-7 -
    Stabilize-provider-setup-auth-and-model-management-UX.md:25@0aea41ac
  - 'commit:0aea41ac'
priority: high
ordinal: 7000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Upstream #177 has owner signal that auth, cross-platform setup, and provider/model setup are the weakest part of the project and are actively being worked on. Use this umbrella task to track concrete fixes that make setup failures actionable and model selection reliable.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Provider setup/login failures provide actionable diagnostics
- [ ] #2 Model picker and route switching avoid stale or unavailable provider routes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 Regression or validation added where applicable
- [ ] #2 Upstream issue/PR reference preserved in final notes
<!-- DOD:END -->
