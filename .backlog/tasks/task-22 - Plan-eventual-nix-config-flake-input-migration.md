---
id: TASK-22
title: Plan eventual nix-config flake input migration
status: To Do
assignee: []
created_date: '2026-05-27 17:54'
updated_date: '2026-05-28 04:57'
labels:
  - planning
  - migration
  - CI
  - release
  - nix
  - flake-inputs
  - compatibility
milestone: m-3
dependencies:
  - TASK-5
references:
  - /Users/jrudnik/infrastructure/nix-config/flake.nix
  - /Users/jrudnik/infrastructure/nix-config/flake.lock
  - >-
    .backlog/tasks/task-22 -
    Plan-eventual-nix-config-flake-input-migration.md:30@0aea41ac
  - 'commit:0aea41ac'
documentation:
  - backlog/docs/planning/doc-1 - Agents-flake-transition-roadmap.md
priority: medium
ordinal: 6000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Prepare the final staged migration plan for nix-config to consume infrastructure/agents as a flake input after validation and pure data outputs are stable.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Plan defines prerequisites, rollback path, and compatibility checks for the flake input migration.
- [ ] #2 Plan avoids changing nix-config locks or deployment policy from this repo session.
- [ ] #3 Plan includes a separate nix-config follow-up task or handoff note for implementation.
<!-- AC:END -->
