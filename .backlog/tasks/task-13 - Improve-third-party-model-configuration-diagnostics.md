---
id: TASK-13
title: Improve third-party model configuration diagnostics
status: To Do
assignee: []
created_date: '2026-05-18 04:41'
updated_date: '2026-05-28 04:57'
labels:
  - upstream
  - owner-interest
  - reliability
  - diagnostics
  - provider
  - third-party
  - configuration
dependencies: []
references:
  - 'https://github.com/1jehuang/jcode/issues/204'
  - >-
    .backlog/tasks/task-13 -
    Improve-third-party-model-configuration-diagnostics.md:25@0aea41ac
  - 'commit:0aea41ac'
priority: medium
ordinal: 13000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Upstream #204 has owner follow-up asking which models and whether the problem is login/auth/API keys. Capture this as actionable UX: third-party provider setup should collect enough context and guide users to the best-supported configuration path.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Third-party provider setup errors identify auth, model, endpoint, or request-shape failures
- [ ] #2 Diagnostics suggest working alternatives such as supported OAuth or openai-compatible profiles
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 Regression or validation added where applicable
- [ ] #2 Upstream issue/PR reference preserved in final notes
<!-- DOD:END -->
