---
id: TASK-18
title: Audit repo hygiene and portable-data boundaries
status: To Do
assignee: []
created_date: '2026-05-27 17:54'
labels:
  - planning
  - hygiene
milestone: m-2
dependencies: []
references:
  - AGENTS.md
  - .gitignore
  - serena/
documentation:
  - backlog/docs/planning/doc-1 - Agents-flake-transition-roadmap.md
priority: high
ordinal: 1000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Inventory tracked, generated, runtime, and local-only files so the repo remains portable agent data before any flake work begins. Document cleanup needs rather than touching runtime state.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Repository content is categorized into portable source data, generated artifacts, local runtime state, and deployment-policy concerns.
- [ ] #2 Cleanup recommendations identify ignore-rule or documentation updates without deleting runtime files.
- [ ] #3 Boundary notes explicitly state that secrets and runtime services remain outside this repo.
<!-- AC:END -->
