---
id: TASK-58
title: >-
  Ambient mode safety/cadence: rate-limit awareness, active-session detection,
  event triggers
status: To Do
assignee: []
created_date: '2026-05-28 05:05'
labels:
  - reliability
  - ambient
  - rate-limiting
  - cadence
  - event-triggers
  - active-session
dependencies:
  - TASK-14
references:
  - 'docs/AMBIENT_MODE.md:944@0aea41ac'
  - 'docs/AMBIENT_MODE.md:946-948@0aea41ac'
  - 'docs/AMBIENT_MODE.md:962@0aea41ac'
  - 'commit:0aea41ac'
priority: medium
ordinal: 52000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: M

Add adaptive resource calculator, rate-limit awareness from provider headers, event triggers (session close, crash, git push), active-session detection with pause/throttle, and a budget bar.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Rate-limit headers are respected
- [ ] #2 Ambient pauses when an active session is detected
- [ ] #3 Event triggers wake ambient
- [ ] #4 Budget bar visible in TUI
<!-- AC:END -->
