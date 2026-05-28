---
id: TASK-40
title: Audit and narrow broad lint/clippy suppressions
status: To Do
assignee: []
created_date: '2026-05-28 05:00'
labels:
  - code-quality
  - CI
  - clippy
  - suppressions
  - allow
  - security
  - reliability
dependencies: []
references:
  - 'docs/CODE_QUALITY_TODO.md:25@0aea41ac'
  - 'docs/CODE_QUALITY_TODO.md:434-450@0aea41ac'
  - 'commit:0aea41ac'
priority: medium
ordinal: 34000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: M

Replace broad #![allow(...)] and other wide suppressions with narrow, justified local allowances. Targets agent/turn_loops, auth/mod, cli/dispatch, main, perf, server, startup tests, TUI remote/state/info_widget.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Each broad suppression is removed, narrowed, or justified inline with a rationale
- [ ] #2 Clippy CI runs without disabling rules globally
- [ ] #3 PR notes summarize each retained suppression
<!-- AC:END -->
