---
id: TASK-25
title: Define CI checks for portable agent content
status: To Do
assignee: []
created_date: '2026-05-28 00:32'
labels:
  - planning
  - ci
  - validation
dependencies: []
references:
  - scripts
  - .github
documentation:
  - backlog/docs/planning/doc-1 - Agents-flake-transition-roadmap.md
priority: medium
ordinal: 19000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Plan the CI and local check coverage needed to validate portable agent data, metadata schemas, and flake outputs before consumers depend on them.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Required checks are enumerated for schema validation, path portability, generated-output purity, and documentation links.
- [ ] #2 The plan identifies which checks run as cargo tests, scripts, nix flake checks, or repository guardrails.
- [ ] #3 Failure modes include actionable messages for contributors fixing invalid agent content.
<!-- AC:END -->
