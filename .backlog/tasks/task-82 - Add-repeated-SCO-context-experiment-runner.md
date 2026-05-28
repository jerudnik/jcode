---
id: TASK-82
title: Add repeated SCO context experiment runner
status: Done
assignee:
  - '@jcode'
created_date: '2026-05-28 13:49'
updated_date: '2026-05-28 13:52'
labels:
  - context
  - evaluation
  - experiments
  - ssh
  - vm
dependencies:
  - TASK-79
  - TASK-80
  - TASK-81
references:
  - scripts/context_pipeline_eval.py
documentation:
  - docs/CONTEXT_PIPELINE_EVAL.md
modified_files:
  - scripts/context_eval_matrix.py
  - scripts/context_pipeline_eval.py
  - docs/CONTEXT_PIPELINE_EVAL.md
priority: high
ordinal: 75000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add scripts and docs to run the context/cache evaluation battery repeatedly under varying assumptions on serious-callers-only or a VM started from that host. The runner should sweep scenario kind, local replay inclusion, tool budgets, repetitions/seeds, and optional VM start hooks, then aggregate variance and go/no-go signals.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Runner can execute a configurable assumption matrix locally and remotely via serious-callers-only without interactive prompts
- [x] #2 Runner writes per-run artifacts plus an aggregate summary with mean/min/max/stdev and pass/fail style gates
- [x] #3 Docs explain SCO/VM usage, safety boundaries, assumption sets, and interpretation of repeated-run variance
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add a stdlib Python matrix runner that expands assumption sets and invokes scripts/context_pipeline_eval.py locally or via its run-remote path.\n2. Add aggregation over matrix.csv files with mean/min/max/stdev, threshold gates, and deterministic run metadata.\n3. Add documentation for SCO/VM invocation, safety boundaries, and interpreting variance.\n4. Validate with a tiny local matrix and a tiny serious-callers-only remote matrix.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented scripts/context_eval_matrix.py as a stdlib repeated matrix runner. It sweeps scenario kind, local replay inclusion, tool budget, repetition count, and optional technique subsets. It can invoke local runs or remote runs through serious-callers-only, passes VM start hook through JCODE_CONTEXT_EVAL_VM_START_CMD/--vm-start-cmd, writes per-run artifacts, and aggregates all rows plus summary mean/min/max/stdev with reliability gates.

Validation: local smoke matrix with synthetic scenario, two repetitions, baseline+combined_p0 passed and wrote target/context-eval-matrix/local-smoke. Remote SCO smoke matrix with realistic+local sessions, one repetition, baseline+combined_p0 passed and wrote target/context-eval-matrix/sco-smoke. Remote summary correctly marked combined_p0 gate pass and baseline gate fail due stale_foreign_retention=1.0.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a repeated SCO/VM context experiment matrix runner to gather enough repeated, assumption-varied data before choosing runtime context/cache changes.\n\nChanges:\n- Added scripts/context_eval_matrix.py to sweep scenario kind, local replay inclusion, tool budgets, repetitions, and technique subsets.\n- Extended scripts/context_pipeline_eval.py run-remote to pass tool-budget and technique assumptions through to the remote run.\n- Documented local, SCO remote, and VM-hook usage plus output layout and reliability gate interpretation in docs/CONTEXT_PIPELINE_EVAL.md.\n\nValidation:\n- python3 -m py_compile scripts/context_pipeline_eval.py scripts/context_eval_matrix.py\n- python3 scripts/context_eval_matrix.py --mode local --scenario-kind synthetic --include-local-sessions false --tool-budget-chars 4000 --repetitions 2 --technique baseline --technique combined_p0 --out target/context-eval-matrix/local-smoke\n- python3 scripts/context_eval_matrix.py --mode remote --host serious-callers-only --remote-dir /tmp/jcode-context-eval-matrix-smoke --scenario-kind realistic --include-local-sessions true --tool-budget-chars 4000 --repetitions 1 --technique baseline --technique combined_p0 --out target/context-eval-matrix/sco-smoke
<!-- SECTION:FINAL_SUMMARY:END -->
