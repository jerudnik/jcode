---
id: TASK-19
title: Define validation strategy for agent content schemas
status: Done
assignee:
  - '@jcode-agent'
created_date: '2026-05-27 17:54'
updated_date: '2026-05-28 02:22'
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
- [x] #1 Validation scope maps each content type to its format spec and canonical file layout.
- [x] #2 Plan covers frontmatter, naming, required sections, and vendor-neutral extension handling.
- [x] #3 Validation approach can run locally and in flake checks without host-specific dependencies.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Self-improvement loop:
1. Inspect format specs, reference examples, and existing validation scripts.
2. Map validation scope by content type and identify host-specific risks.
3. Propose and implement a minimal validation plan/prototype if current capabilities can verify it.
4. Review for deployment-policy leakage and contributor usability.
5. Run local validation against existing repo content and fix issues.
6. Record final summary and close if ACs are met.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Loop findings: roadmap references to top-level skills/permissions/prompts/specs and format specs were initially absent in this checkout, while current portable agent content exists under .jcode/skills. Implemented minimal specs and a dependency-free validator that validates current skills now and future roots when they appear.

Validation strategy implemented in docs/AGENT_CONTENT_VALIDATION.md and docs/*/format-spec.md. The validator checks scalar frontmatter, naming/layout, x-* extension handling, Markdown body presence, and host-specific path leakage without host-specific dependencies.

Validation run: python3 scripts/validate_agent_content.py passed with skill=2; python3 -m py_compile scripts/validate_agent_content.py passed; negative fixture test produced actionable file-specific errors for mismatched name, unsupported field, and host path.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Defined and implemented a host-independent validation strategy for portable agent content.

Changes:
- Added docs/AGENT_CONTENT_VALIDATION.md mapping skills, permissions, prompts, and specs to canonical layouts, required frontmatter, naming rules, extension handling, and local/CI validation placement.
- Added minimal format specs under docs/skills, docs/permissions, docs/prompts, and docs/specs.
- Added scripts/validate_agent_content.py, a dependency-free validator for current .jcode skills and future portable content roots.

Validation:
- python3 scripts/validate_agent_content.py
- python3 -m py_compile scripts/validate_agent_content.py
- Negative temporary fixture verified actionable error messages.
<!-- SECTION:FINAL_SUMMARY:END -->
