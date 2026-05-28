---
id: TASK-2
title: Make selfdev build environment independent of interactive shell PATH
status: To Do
assignee: []
created_date: '2026-05-18 04:38'
updated_date: '2026-05-28 04:57'
labels:
  - selfdev
  - build
  - reliability
  - CI
  - PATH
  - reproducibility
dependencies: []
references:
  - src/tool/selfdev
  - src/cli/selfdev_tests.rs
  - >-
    .backlog/tasks/task-2 -
    Make-selfdev-build-environment-independent-of-interactive-shell-PATH.md:27@0aea41ac
  - 'commit:0aea41ac'
priority: high
ordinal: 2000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
selfdev build failed because the coordinated build environment could not find cargo, even though nix develop had cargo. Make the selfdev build path reliably enter the repo dev shell or resolve cargo before invoking scripts/dev_cargo.sh.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 selfdev build succeeds from a bare/non-interactive environment on this machine.
- [ ] #2 Failure messages explain missing dev-shell/cargo setup clearly.
- [ ] #3 Regression coverage exercises command construction or environment selection.
<!-- AC:END -->
