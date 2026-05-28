---
id: TASK-21
title: Specify portable MCP catalog metadata
status: To Do
assignee: []
created_date: '2026-05-27 17:54'
labels:
  - planning
  - mcp
milestone: m-0
dependencies:
  - TASK-1
  - TASK-2
references:
  - docs/
  - AGENTS.md
documentation:
  - backlog/docs/planning/doc-1 - Agents-flake-transition-roadmap.md
priority: medium
ordinal: 4000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Define a portable metadata shape for MCP catalog entries that this repo can export and nix-config can translate into deployment-specific configuration.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Metadata fields distinguish portable catalog facts from consumer-specific runtime settings.
- [ ] #2 Spec covers names, descriptions, tool boundaries, docs references, and optional client/vendor extensions.
- [ ] #3 Design identifies how nix-config should consume or transform catalog data without this repo managing secrets or services.
<!-- AC:END -->
