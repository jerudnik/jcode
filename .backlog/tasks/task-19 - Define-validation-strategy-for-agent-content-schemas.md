---
id: TASK-19
title: Define validation strategy for agent content schemas
status: To Do
assignee: []
created_date: '2026-05-27 17:54'
labels:
  - planning
  - validation
milestone: m-1
dependencies:
  - TASK-1
references:
  - skills/repo-structure/SKILL.md
  - permissions/python-dev/PERMISSION.md
  - prompts/voice-and-posture/PROMPT.md
documentation:
  - backlog/docs/planning/doc-1 - Agents-flake-transition-roadmap.md
  - docs/skills/format-spec.md
  - docs/permissions/format-spec.md
  - docs/prompts/format-spec.md
  - docs/specs/format-spec.md
priority: high
ordinal: 2000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Turn existing format specs and reference examples into a validation plan for skills, permissions, prompts, specs, and future metadata without introducing deployment policy.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Validation scope maps each content type to its format spec and canonical file layout.
- [ ] #2 Plan covers frontmatter, naming, required sections, and vendor-neutral extension handling.
- [ ] #3 Validation approach can run locally and in flake checks without host-specific dependencies.
<!-- AC:END -->
