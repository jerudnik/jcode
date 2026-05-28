---
id: TASK-24
title: Map agent-data consumer compatibility matrix
status: To Do
assignee: []
created_date: '2026-05-28 00:32'
labels:
  - planning
  - compatibility
dependencies: []
references:
  - AGENTS.md
  - .jcode/skills/devshell-workflow/SKILL.md
documentation:
  - backlog/docs/planning/doc-1 - Agents-flake-transition-roadmap.md
priority: medium
ordinal: 18000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Identify how current and expected consumers load skills, prompts, permissions, specs, and MCP catalog data so flake outputs can remain portable without breaking existing local or external workflows.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Known consumers and entry points are listed with the data types they read.
- [ ] #2 Compatibility risks are documented for direct file access, CLI usage, flake outputs, and nix-config consumption.
- [ ] #3 Recommendations distinguish backward-compatible shims from intentional migration steps.
<!-- AC:END -->
