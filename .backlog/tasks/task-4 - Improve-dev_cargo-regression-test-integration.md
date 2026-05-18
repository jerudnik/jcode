---
id: TASK-4
title: Improve dev_cargo regression test integration
status: To Do
assignee: []
created_date: '2026-05-18 04:38'
labels:
  - tests
  - tooling
dependencies: []
references:
  - tests/test_dev_cargo.py
  - scripts/test_fast.sh
priority: medium
ordinal: 4000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
tests/test_dev_cargo.py currently runs standalone. Decide whether to wire it into an existing fast test script or document it in the developer workflow so the new build-env regression test is actually exercised.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A standard local/CI command includes the dev_cargo setup tests, or README/AGENTS clearly documents when to run them.
- [ ] #2 The test remains fast and does not require real sccache or cargo compilation.
<!-- AC:END -->
