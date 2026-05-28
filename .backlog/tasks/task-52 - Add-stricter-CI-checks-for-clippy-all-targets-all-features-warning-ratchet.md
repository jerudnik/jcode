---
id: TASK-52
title: 'Add stricter CI checks for clippy, all-targets, all-features, warning ratchet'
status: To Do
assignee: []
created_date: '2026-05-28 05:04'
labels:
  - CI
  - code-quality
  - clippy
  - warnings
  - guardrails
dependencies:
  - TASK-40
references:
  - 'docs/CODE_QUALITY_10_10_PLAN.md:123-128@0aea41ac'
  - 'commit:0aea41ac'
priority: high
ordinal: 46000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: M

Phase 0 of the 10/10 plan. Add CI jobs that fail on warnings for all targets and features; ratchet warning policy downward; document code-quality standards and file-size goals.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 CI runs clippy --all-targets --all-features in CI and fails on warnings
- [ ] #2 Warning ratchet is set with a baseline
- [ ] #3 Standards doc is published
<!-- AC:END -->
