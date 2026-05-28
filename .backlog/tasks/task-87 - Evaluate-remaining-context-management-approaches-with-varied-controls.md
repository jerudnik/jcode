---
id: TASK-87
title: Evaluate remaining context-management approaches with varied controls
status: Done
assignee:
  - '@jcode'
created_date: '2026-05-28 14:52'
updated_date: '2026-05-28 15:04'
labels:
  - context
  - evaluation
  - experiments
  - reliability
dependencies:
  - TASK-79
  - TASK-80
  - TASK-81
  - TASK-86
references:
  - scripts/context_pipeline_eval.py
  - scripts/context_eval_matrix.py
  - target/context-eval-matrix/task87-full-rerun
  - target/context-eval-matrix/task87-sco-confirm
documentation:
  - docs/CONTEXT_PIPELINE_EVAL.md
modified_files:
  - scripts/context_pipeline_eval.py
  - scripts/context_eval_matrix.py
  - docs/CONTEXT_PIPELINE_EVAL.md
priority: high
ordinal: 80000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Run a thorough deterministic assessment of the remaining context-management approaches identified after TASK-79, including controls and enough scenario variation to reduce overfitting risk.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 All ten remaining context-management approaches have prototype transforms or scenario-specific evaluators in the harness
- [x] #2 Experiments include positive/negative controls plus varied synthetic, realistic, cache-confusion, and public-benchmark-style scenarios with repeated runs
- [x] #3 Results are summarized with recommended adoption order, risks, artifact paths, and validation
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Extend the deterministic context pipeline harness with scenario coverage for cache confusion and public-benchmark-style long-context replay.
2. Add rough prototype transforms/evaluators for all ten remaining approaches from TASK-80/TASK-81.
3. Run a full local matrix with controls, varied scenarios, local-session replay on/off, two budgets, and repeated runs.
4. Run a smaller SCO confirmation matrix to check cross-host behavior.
5. Generate reports, run secret scans/validation, document results, and record recommendations.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented the ten remaining prototype techniques plus cache_confusion and public_benchmark stress scenarios in the deterministic harness.

Local full matrix completed at target/context-eval-matrix/task87-full-rerun with 576 rows across controls, synthetic, realistic, cache-confusion, and public-benchmark scenarios.

SCO confirmation completed at target/context-eval-matrix/task87-sco-confirm with 144 rows. It confirmed the main pattern but differed on private local-session replay because the remote host has different session logs.

Generated reports for both artifact roots. Raw local-session artifacts are non-shareable because the secret scanner found sentinel strings in private replay blocks; aggregate report directories scanned clean.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Completed a thorough assessment of the remaining context-management approaches from TASK-80/TASK-81.

Changes:
- Extended scripts/context_pipeline_eval.py with cache_confusion and public_benchmark stress scenarios.
- Added prototype techniques for goal_task_ledger, supersession_prune, attention_index, lazy_restore_handles, pinned_spans, recency_importance, provenance_routing, scratchpad, memory_ttl, and cache_isolation.
- Extended scripts/context_eval_matrix.py to sweep the new scenarios and techniques.
- Documented TASK-87 results and recommended adoption order in docs/CONTEXT_PIPELINE_EVAL.md.

Experiments:
- Local full matrix target/context-eval-matrix/task87-full-rerun: 576 rows across 6 scenarios, local-session replay on/off, two tool budgets, two repetitions, baseline/combined controls, and all ten remaining approaches.
- SCO confirmation target/context-eval-matrix/task87-sco-confirm: 144 rows over the same scenario/technique set with one budget and one repetition.
- Reports generated under each artifact directory's report/ folder.

Findings:
- lazy_restore_handles had the best mean score but needs protected-span-aware summarization before runtime use.
- combined_p0 remains the best balanced pipeline but needs explicit supersession pruning and stronger local-session contamination handling.
- supersession_prune is the best low-risk next runtime candidate and reduced local-session stale retention more than other single techniques.
- provenance_routing is a strong trust-tier foundation.
- cache_isolation works for cache-confusion but is not a general contamination solution.
- goal_task_ledger, attention_index, pinned_spans, and scratchpad are useful downstream continuity/salience layers but not safety controls by themselves.

Validation:
- python3 -m py_compile scripts/context_pipeline_eval.py scripts/context_eval_matrix.py scripts/context_experiment_report.py scripts/context_artifact_secret_scan.py
- git diff --check
- Report secret scan passed with zero findings for generated aggregate reports.
- Raw local replay artifacts intentionally triggered sentinel-secret findings and are marked non-shareable.
<!-- SECTION:FINAL_SUMMARY:END -->
