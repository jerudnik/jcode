---
id: TASK-8
title: Fix emergency compaction count drift and repeated marker loop
status: To Do
assignee: []
created_date: '2026-05-18 04:41'
labels:
  - upstream
  - owner-interest
dependencies: []
references:
  - 'https://github.com/1jehuang/jcode/issues/175'
priority: high
ordinal: 8000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Upstream #175 received strong owner endorsement. Prevent compacted_count from drifting past messages.len(), stop active_messages from replaying the full transcript on stale state, and prevent repeated emergency marker accumulation from wedging long sessions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Stale compacted_count is clamped or recovered without replaying the full transcript
- [ ] #2 Repeated emergency compaction does not append duplicate markers indefinitely
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 Regression or validation added where applicable
- [ ] #2 Upstream issue/PR reference preserved in final notes
<!-- DOD:END -->
