---
id: TASK-3
title: Replace ad-hoc backlog and quality docs with Backlog.md tasks
status: To Do
assignee: []
created_date: '2026-05-18 04:38'
updated_date: '2026-05-28 04:57'
labels:
  - docs
  - backlog
  - process
  - code-quality
  - migration
dependencies: []
references:
  - docs/CODE_QUALITY_TODO.md
  - >-
    .backlog/tasks/task-3 -
    Replace-ad-hoc-backlog-and-quality-docs-with-Backlog.md-tasks.md:27@0aea41ac
  - 'commit:0aea41ac'
priority: medium
ordinal: 3000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The repo already has docs/CODE_QUALITY_TODO.md and audit docs. Migrate the highest-value active items into Backlog.md tasks so agents use one task system, while keeping historical audit docs as references.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 At least the top current code-quality items are represented as Backlog.md tasks with acceptance criteria.
- [ ] #2 No large historical audit content is deleted unless redundant.
  [HIGH] TASK-1 - Stabilize global env and cache isolation in lib tests
  [HIGH] TASK-2 - Make selfdev build environment independent of interactive shell PATH for active work.
- [ ] #3 Docs point contributors to `backlog task list --plain` for active work.
<!-- AC:END -->
