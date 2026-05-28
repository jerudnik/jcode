---
id: TASK-44
title: Upgrade ratatui / ratatui-image to current secure releases
status: To Do
assignee: []
created_date: '2026-05-28 05:03'
labels:
  - security
  - dependencies
  - ratatui
  - tui
  - upgrade
dependencies: []
references:
  - 'docs/SECURITY_DEPENDENCIES.md:12-20@0aea41ac'
  - 'commit:0aea41ac'
priority: high
ordinal: 38000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: M

Upgrade ratatui and ratatui-image past advisory windows; resolve any breaking changes in the TUI.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 ratatui and ratatui-image are on a maintained release without open advisories
- [ ] #2 TUI renders unchanged in golden snapshot tests
- [ ] #3 cargo audit clean for these crates
<!-- AC:END -->
