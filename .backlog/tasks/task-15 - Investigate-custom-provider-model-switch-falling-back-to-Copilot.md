---
id: TASK-15
title: Investigate custom provider model switch falling back to Copilot
status: To Do
assignee: []
created_date: '2026-05-18 04:41'
labels:
  - upstream
  - owner-interest
dependencies: []
references:
  - 'https://github.com/1jehuang/jcode/issues/154'
priority: medium
ordinal: 15000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Upstream #154 has owner follow-up requesting provider details. Reproduce the /model failure where switching a custom openai-compatible route errors on missing Copilot credentials, then fix routing/auth selection if confirmed.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Switching to a custom provider profile does not require unrelated Copilot credentials
- [ ] #2 Regression covers stale/auth-unavailable routes in model picker
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 Regression or validation added where applicable
- [ ] #2 Upstream issue/PR reference preserved in final notes
<!-- DOD:END -->
