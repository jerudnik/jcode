---
id: TASK-14
title: Verify Ollama model switching fix and add regression coverage
status: To Do
assignee: []
created_date: '2026-05-18 04:41'
updated_date: '2026-05-28 04:57'
labels:
  - upstream
  - owner-interest
  - reliability
  - regression-coverage
  - ollama
  - model-picker
  - local-models
dependencies: []
references:
  - 'https://github.com/1jehuang/jcode/issues/157'
  - >-
    .backlog/tasks/task-14 -
    Verify-Ollama-model-switching-fix-and-add-regression-coverage.md:25@0aea41ac
  - 'commit:0aea41ac'
priority: medium
ordinal: 14000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Upstream #157 has owner signal that v0.12.1 should fix Ollama model switching. Add a verification task so the behavior is covered and does not regress, especially model persistence across sessions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Ollama models appear in /model for configured local profiles
- [ ] #2 Selected default model persists consistently across new sessions when configured
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 Regression or validation added where applicable
- [ ] #2 Upstream issue/PR reference preserved in final notes
<!-- DOD:END -->
