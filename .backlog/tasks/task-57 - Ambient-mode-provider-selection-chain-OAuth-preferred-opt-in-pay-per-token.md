---
id: TASK-57
title: 'Ambient mode provider selection chain (OAuth-preferred, opt-in pay-per-token)'
status: To Do
assignee: []
created_date: '2026-05-28 05:05'
labels:
  - security
  - auth
  - ambient
  - provider
  - oauth
  - opt-in
dependencies: []
references:
  - 'docs/AMBIENT_MODE.md:928@0aea41ac'
  - 'commit:0aea41ac'
priority: high
ordinal: 51000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: M

Implement provider selection chain for ambient: OpenAI OAuth -> Anthropic OAuth -> pay-per-token opt-in -> disabled. Default disabled.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Chain order matches docs
- [ ] #2 Pay-per-token requires explicit opt-in
- [ ] #3 Disabled by default until a provider is reachable
<!-- AC:END -->
