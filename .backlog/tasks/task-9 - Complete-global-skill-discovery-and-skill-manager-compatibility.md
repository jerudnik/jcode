---
id: TASK-9
title: Complete global skill discovery and skill-manager compatibility
status: To Do
assignee: []
created_date: '2026-05-18 04:41'
updated_date: '2026-05-28 04:57'
labels:
  - upstream
  - owner-interest
  - reliability
  - compatibility
  - skills
  - skill-discovery
  - UX
dependencies: []
references:
  - 'https://github.com/1jehuang/jcode/issues/158'
  - >-
    .backlog/tasks/task-9 -
    Complete-global-skill-discovery-and-skill-manager-compatibility.md:25@0aea41ac
  - 'commit:0aea41ac'
priority: medium
ordinal: 9000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Upstream #158 has owner signal that the issue is not fully fixed. Runtime global loading currently reads only one level of ~/.jcode/skills, while import tooling can discover nested paths. Align skill discovery, diagnostics, and aliases with common agentskills layouts.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Skills installed under documented ~/.jcode/skills and compatible global paths are discoverable
- [ ] #2 Missing skill errors explain searched paths and expected SKILL.md layout
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 Regression or validation added where applicable
- [ ] #2 Upstream issue/PR reference preserved in final notes
<!-- DOD:END -->
