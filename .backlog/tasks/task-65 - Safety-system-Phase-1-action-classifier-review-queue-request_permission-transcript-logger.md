---
id: TASK-65
title: >-
  Safety system Phase 1: action classifier, review queue, request_permission,
  transcript logger
status: To Do
assignee: []
created_date: '2026-05-28 05:06'
labels:
  - security
  - safety
  - permissions
  - review
  - transcript
  - classifier
dependencies: []
references:
  - 'docs/SAFETY_SYSTEM.md:512-516@0aea41ac'
  - 'commit:0aea41ac'
priority: high
ordinal: 59000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: L

Phase 1 of the safety system: action classifier (tier 1/2/3), persistent review queue, request_permission tool for agents, transcript logger, basic session summary generation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Classifier categorizes actions
- [ ] #2 Review queue persists across restarts
- [ ] #3 request_permission tool callable by agents
- [ ] #4 Transcripts saved to durable storage
<!-- AC:END -->
