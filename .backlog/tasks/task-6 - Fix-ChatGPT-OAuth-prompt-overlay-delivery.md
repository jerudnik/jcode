---
id: TASK-6
title: Fix ChatGPT OAuth prompt-overlay delivery
status: To Do
assignee: []
created_date: '2026-05-18 04:41'
updated_date: '2026-05-28 04:57'
labels:
  - upstream
  - owner-interest
  - security
  - auth
  - oauth
  - chatgpt
  - provider
  - reliability
  - regression
dependencies: []
references:
  - 'https://github.com/1jehuang/jcode/issues/203'
  - >-
    .backlog/tasks/task-6 -
    Fix-ChatGPT-OAuth-prompt-overlay-delivery.md:25@0aea41ac
  - 'commit:0aea41ac'
priority: high
ordinal: 6000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Upstream #203 has owner signal: problem 1 will be fixed in the next patch. Ensure AGENTS.md and prompt-overlay sections counted in prompt accounting are actually sent to ChatGPT OAuth mode, instead of being replaced by ChatGPT-specific instructions that keep only self-dev content.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 ChatGPT OAuth request path preserves project/global instructions and prompt overlays
- [ ] #2 Add a regression test proving overlay text survives ChatGPT instruction wrapping
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 Regression or validation added where applicable
- [ ] #2 Upstream issue/PR reference preserved in final notes
<!-- DOD:END -->
