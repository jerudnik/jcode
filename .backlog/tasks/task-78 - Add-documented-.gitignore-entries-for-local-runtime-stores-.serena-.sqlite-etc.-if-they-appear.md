---
id: TASK-78
title: >-
  Add documented .gitignore entries for local runtime stores (.serena/,
  *.sqlite, etc.) if they appear
status: To Do
assignee: []
created_date: '2026-05-28 05:08'
labels:
  - repo-hygiene
  - gitignore
  - planning
  - runtime-state
dependencies: []
references:
  - >-
    .backlog/docs/planning/doc-2 -
    Repo-hygiene-and-portable-data-boundary-audit.md:74-77@0aea41ac
  - 'commit:0aea41ac'
priority: low
ordinal: 72000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: XS

Add ignore entries for .serena/, serena/, .jcode/logs/, .jcode/builds/, *.sqlite, *.db, *.log if/when these appear in tree, per the planning doc recommendation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Recommended entries staged in .gitignore once they apply
- [ ] #2 Entries justified in commit message
- [ ] #3 No unwanted files tracked
<!-- AC:END -->
