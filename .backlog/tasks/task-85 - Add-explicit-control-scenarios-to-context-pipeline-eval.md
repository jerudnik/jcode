---
id: TASK-85
title: Add explicit control scenarios to context pipeline eval
status: Done
assignee:
  - '@jcode'
created_date: '2026-05-28 14:27'
updated_date: '2026-05-28 14:28'
labels:
  - context
  - evaluation
  - experiments
dependencies:
  - TASK-84
modified_files:
  - scripts/context_pipeline_eval.py
  - scripts/context_eval_matrix.py
priority: high
ordinal: 78000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add positive oracle and negative contaminated control scenarios so context pipeline experiment results are interpretable beyond the no-transform baseline.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A clean oracle scenario establishes that controls can pass when no stale/foreign contamination is present
- [x] #2 A negative contaminated scenario establishes that stale/foreign gates fail when distractors are retained
- [x] #3 Matrix runner accepts the control scenario kinds
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add oracle and negative scenario kinds to context_pipeline_eval.py.
2. Wire scenario choices through context_eval_matrix.py.
3. Run control smoke and commit.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added explicit oracle and negative scenario kinds. Control smoke verified oracle baseline passes, negative baseline fails stale/foreign gate, and trust_quarantine/combined_p0 remove negative-control stale terms.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added explicit positive and negative controls to the context evaluation pipeline.

Changes:
- Added oracle scenario with protected facts and no stale/foreign contamination.
- Added negative scenario with protected facts plus controlled stale/foreign distractors.
- Wired both scenario kinds through context_pipeline_eval.py and context_eval_matrix.py.

Validation:
- Control smoke under target/context-eval/control-smoke-20260528-142833.
- Oracle baseline passed reliability gates.
- Negative baseline failed with stale/foreign retention 1.0.
- Negative trust_quarantine and combined_p0 passed with stale/foreign retention 0.0.
- Report generation and allowlisted secret scan passed.
<!-- SECTION:FINAL_SUMMARY:END -->
