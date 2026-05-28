---
id: TASK-20
title: Document nix-config integration boundary
status: To Do
assignee: []
created_date: '2026-05-27 17:54'
labels:
  - planning
  - nix-config
milestone: m-3
dependencies:
  - TASK-3
  - TASK-4
references:
  - AGENTS.md
  - /Users/jrudnik/infrastructure/nix-config/modules/ai/home-manager/skills/
documentation:
  - backlog/docs/planning/doc-1 - Agents-flake-transition-roadmap.md
priority: high
ordinal: 5000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create a clear contract for how /Users/jrudnik/infrastructure/nix-config should consume this repo and where integration policy lives during the gradual poly-repo transition.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Boundary document states this repo exports portable content and metadata only.
- [ ] #2 Boundary document states nix-config owns deployment locations, activation behavior, services, secrets, and host policy.
- [ ] #3 Migration prerequisites and review checkpoints are listed before changing nix-config flake inputs.
<!-- AC:END -->
