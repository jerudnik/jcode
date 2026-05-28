---
id: TASK-3
title: Replace ad-hoc backlog and quality docs with Backlog.md tasks
status: Done
assignee:
  - '@jcode-agent'
created_date: '2026-05-18 04:38'
updated_date: '2026-05-28 07:33'
labels:
  - docs
  - backlog
  - process
  - code-quality
  - migration
dependencies: []
references:
  - docs/CODE_QUALITY_TODO.md
  - docs/PROJECT_STATE.md
  - README.md
  - AGENTS.md
  - .backlog/docs/planning/doc-3 - Current-project-state-snapshot.md
documentation:
  - docs/PROJECT_STATE.md
  - .backlog/docs/planning/doc-3 - Current-project-state-snapshot.md
modified_files:
  - docs/CODE_QUALITY_TODO.md
  - docs/PROJECT_STATE.md
  - README.md
  - AGENTS.md
  - .backlog/docs/planning/doc-3 - Current-project-state-snapshot.md
priority: medium
ordinal: 3000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The repo already has docs/CODE_QUALITY_TODO.md and audit docs. Migrate the highest-value active items into Backlog.md tasks so agents use one task system, while keeping historical audit docs as references.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 At least the top current code-quality items are represented as Backlog.md tasks with acceptance criteria.
- [x] #2 No large historical audit content is deleted unless redundant.
- [x] #3 Docs point contributors to `backlog task list --plain` for active work.
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Completed state sync: code-quality execution tracking is represented by Backlog.md tasks, historical audit docs remain as references, and docs now point contributors to the live Backlog.md board.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Migrated the remaining code-quality tracking status into the live Backlog.md workflow.

Changes:
- Confirmed the top code-quality work is represented as Backlog.md tasks.
- Preserved historical audit and execution docs as references instead of deleting them.
- Updated documentation to direct contributors to `backlog task list --plain` and the `.backlog/` task store.

Validation:
- Ran `scripts/git-hooks/check-backlog-tracking.sh --all --strict`.
- Ran `scripts/git-hooks/check-backlog-tracking.sh docs/PROJECT_STATE.md ".backlog/docs/planning/doc-3 - Current-project-state-snapshot.md"`.
- Ran `python3 scripts/backlog_pointer_verify.py check` (passed with pre-existing warnings only).
<!-- SECTION:FINAL_SUMMARY:END -->
