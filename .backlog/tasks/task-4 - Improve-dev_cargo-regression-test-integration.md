---
id: TASK-4
title: Improve dev_cargo regression test integration
status: Done
assignee:
  - '@jcode-agent'
created_date: '2026-05-18 04:38'
updated_date: '2026-05-28 02:17'
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
- [x] #1 A standard local/CI command includes the dev_cargo setup tests, or README/AGENTS clearly documents when to run them.
- [x] #2 The test remains fast and does not require real sccache or cargo compilation.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Self-improvement loop:
1. Inspect current dev_cargo tests and fast-test workflow.
2. Determine whether integration or documentation best satisfies the ACs.
3. Implement the minimal change.
4. Critically review for speed and dependency risks.
5. Run targeted validations and fix issues.
6. Update task notes/final summary and commit.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Loop results: wired tests/test_dev_cargo.py into scripts/test_fast.sh before cargo tests so the standard fast command exercises the setup regression without requiring compilation for that Python step.

The loop found the standalone regression was failing: dev_cargo still enabled automatic sccache when CARGO_INCREMENTAL was set. Fixed scripts/dev_cargo.sh to skip automatic sccache in that case while preserving explicit RUSTC_WRAPPER behavior.

Validation: python3 tests/test_dev_cargo.py passed; bash -n scripts/dev_cargo.sh scripts/test_fast.sh passed. Full scripts/test_fast.sh confirmed the new regression runs first and passes, then failed later in unrelated existing cargo tests (35 failures across TUI/provider/auth areas).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Integrated the dev_cargo setup regression into the standard fast test loop and fixed the stale regression it exposed.

Changes:
- scripts/test_fast.sh now runs python3 tests/test_dev_cargo.py before cargo tests, ensuring the setup regression is covered by the local fast command.
- scripts/dev_cargo.sh now skips automatic sccache when CARGO_INCREMENTAL is set, matching the regression contract and avoiding sccache/incremental incompatibility.

Validation:
- python3 tests/test_dev_cargo.py
- bash -n scripts/dev_cargo.sh scripts/test_fast.sh
- scripts/test_fast.sh started with the new regression passing; later cargo test failures appear unrelated/pre-existing in TUI/provider/auth tests.
<!-- SECTION:FINAL_SUMMARY:END -->
