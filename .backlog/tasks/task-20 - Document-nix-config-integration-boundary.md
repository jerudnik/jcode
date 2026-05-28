---
id: TASK-20
title: Document nix-config integration boundary
status: Done
assignee:
  - '@jcode-agent'
created_date: '2026-05-27 17:54'
updated_date: '2026-05-28 07:33'
labels:
  - planning
  - nix-config
  - security
  - architecture
  - docs
  - boundaries
  - secrets
milestone: m-3
dependencies:
  - TASK-3
  - TASK-4
references:
  - AGENTS.md
  - docs/PROJECT_STATE.md
  - .backlog/docs/planning/doc-3 - Current-project-state-snapshot.md
  - /Users/jrudnik/infrastructure/nix-config/modules/ai/home-manager/skills/
documentation:
  - .backlog/docs/planning/doc-1 - Agents-flake-transition-roadmap.md
  - >-
    .backlog/docs/planning/doc-2 -
    Repo-hygiene-and-portable-data-boundary-audit.md
  - .backlog/docs/planning/doc-3 - Current-project-state-snapshot.md
  - docs/PROJECT_STATE.md
modified_files:
  - AGENTS.md
  - README.md
  - docs/PROJECT_STATE.md
  - .backlog/docs/planning/doc-3 - Current-project-state-snapshot.md
priority: high
ordinal: 5000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create a clear contract for how /Users/jrudnik/infrastructure/nix-config should consume this repo and where integration policy lives during the gradual poly-repo transition.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Boundary document states this repo exports portable content and metadata only.
- [x] #2 Boundary document states nix-config owns deployment locations, activation behavior, services, secrets, and host policy.
- [x] #3 Migration prerequisites and review checkpoints are listed before changing nix-config flake inputs.
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Completed boundary documentation in the repo state snapshot and Backlog.md planning snapshot. The boundary keeps portable source/content/checks in this repo and host deployment policy, secrets, services, activation, and runtime state in consumer repos such as nix-config.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Documented the nix-config integration boundary in the current project-state materials.

Changes:
- Added a repository-facing state snapshot that states this repo exports portable source data, portable agent content, metadata, checks, and binaries.
- Added a Backlog.md planning snapshot with the same boundary statement.
- Clarified that nix-config owns deployment policy, installation locations, activation behavior, launchd/home-manager/nix-darwin wiring, secrets, services, and host-specific runtime state.

Validation:
- Ran `scripts/git-hooks/check-backlog-tracking.sh --all --strict`.
- Ran `scripts/git-hooks/check-backlog-tracking.sh docs/PROJECT_STATE.md ".backlog/docs/planning/doc-3 - Current-project-state-snapshot.md"`.
- Ran `python3 scripts/backlog_pointer_verify.py check` (passed with pre-existing warnings only).
<!-- SECTION:FINAL_SUMMARY:END -->
