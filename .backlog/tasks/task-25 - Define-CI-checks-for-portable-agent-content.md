---
id: TASK-25
title: Define CI checks for portable agent content
status: Done
assignee:
  - '@jcode-agent'
created_date: '2026-05-28 00:32'
updated_date: '2026-05-28 02:22'
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
- [x] #1 Required checks are enumerated for schema validation, path portability, generated-output purity, and documentation links.
- [x] #2 The plan identifies which checks run as cargo tests, scripts, nix flake checks, or repository guardrails.
- [x] #3 Failure modes include actionable messages for contributors fixing invalid agent content.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Self-improvement loop:
1. Inspect existing scripts, workflows, format specs, and portable-agent roadmap.
2. Enumerate CI/local checks for schema, path portability, output purity, and docs links.
3. Implement docs and any lightweight local guard/prototype that can validate current repo content.
4. Review failure messages and host-independence.
5. Run lightweight checks and fix issues.
6. Record final summary and close if ACs are met.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Loop results: required checks are enumerated in docs/AGENT_CONTENT_VALIDATION.md for schema/layout, path portability, generated-output purity, documentation links, and repository guardrails. Current executable coverage is the agent content validator.

Implemented CI/local wiring: scripts/test_fast.sh runs the validator before cargo tests, and .github/workflows/ci.yml quality guardrails run python3 scripts/validate_agent_content.py. Future flake-check placement is documented for when flake outputs exist.

Validation run: python3 scripts/validate_agent_content.py passed; py_compile passed; fast precheck path passed; CI step presence verified by script.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Defined CI and local check coverage for portable agent content and made the schema/path portability check executable now.

Changes:
- docs/AGENT_CONTENT_VALIDATION.md enumerates checks for schema validation, path portability, generated-output purity, documentation links, and repository guardrails.
- scripts/validate_agent_content.py provides actionable file-specific failures without host-specific dependencies.
- scripts/test_fast.sh and .github/workflows/ci.yml now run the validator as local and CI guardrails.

Validation:
- python3 scripts/validate_agent_content.py
- python3 -m py_compile scripts/validate_agent_content.py
- fast precheck command with dev_cargo plus agent validation
- CI step presence check.
<!-- SECTION:FINAL_SUMMARY:END -->
