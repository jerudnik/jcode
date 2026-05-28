---
id: TASK-11
title: Evaluate local agent telemetry warehouse ingestion bridge
status: To Do
assignee: []
created_date: '2026-05-18 04:41'
updated_date: '2026-05-28 04:57'
labels:
  - upstream
  - owner-interest
  - privacy
  - telemetry
  - observability
  - reliability
  - opt-in
dependencies: []
references:
  - 'https://github.com/1jehuang/jcode/issues/210'
  - >-
    .backlog/tasks/task-11 -
    Evaluate-local-agent-telemetry-warehouse-ingestion-bridge.md:25@0aea41ac
  - 'commit:0aea41ac'
priority: low
ordinal: 11000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Upstream #210 has owner signal that user-owned telemetry analysis is interesting but privacy-sensitive. Explore a config-gated bridge that exports safe run/tool/swarm metadata without prompt/code/file-path content by default.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Telemetry export is opt-in and excludes sensitive content by default
- [ ] #2 Design covers run lifecycle, tool metadata, swarm lifecycle, and outcome annotations
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 Regression or validation added where applicable
- [ ] #2 Upstream issue/PR reference preserved in final notes
<!-- DOD:END -->
