---
id: TASK-26
title: Investigate migration path for local runtime state
status: To Do
assignee: []
created_date: '2026-05-28 00:32'
labels:
  - planning
  - migration
  - hygiene
dependencies: []
references:
  - .gitignore
  - AGENTS.md
documentation:
  - backlog/docs/planning/doc-1 - Agents-flake-transition-roadmap.md
priority: high
ordinal: 20000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Audit how runtime-only agent state, caches, logs, secrets references, and local configuration should be preserved or ignored while moving portable source data toward flake-exported outputs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Runtime-state locations are cataloged without deleting or modifying user state.
- [ ] #2 Migration guidance explains what remains local versus what can be regenerated or exported from portable source data.
- [ ] #3 Risks are documented for symlinks, launchers, caches, logs, auth tokens, and user-specific configuration.
<!-- AC:END -->
