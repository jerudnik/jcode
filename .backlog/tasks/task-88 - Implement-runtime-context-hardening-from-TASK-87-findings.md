---
id: TASK-88
title: Implement runtime context hardening from TASK-87 findings
status: Done
assignee:
  - '@jcode'
created_date: '2026-05-28 15:09'
updated_date: '2026-05-28 15:16'
labels:
  - context
  - compaction
  - reliability
  - runtime
dependencies:
  - TASK-87
references:
  - src/agent/context_pruning.rs
  - scripts/context_pipeline_eval.py
  - target/context-eval-matrix/task88-regression
documentation:
  - docs/CONTEXT_PIPELINE_EVAL.md
modified_files:
  - src/agent/context_pruning.rs
  - docs/CONTEXT_PIPELINE_EVAL.md
priority: high
ordinal: 81000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement the first runtime context-management hardening targets selected by TASK-87: supersession pruning, provenance/trust routing, and protected lazy restore handles. Keep changes reliability-first and testable against deterministic fixtures.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Runtime context pruning handles superseded failed tools, speculative stale assistant hypotheses, and stale restored-session/cache blocks without dropping protected user/task facts
- [x] #2 Provider-visible context includes provenance/trust markers or placeholders for pruned untrusted blocks with restore metadata sufficient for targeted expansion
- [x] #3 Protected facts, task IDs, file paths, and restore needles survive pruning/summarization in targeted tests and context-eval regression runs
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Extend the existing provider-visible context pruning path rather than building a separate pipeline.
2. Add runtime routing for old low-trust/stale restored-session tool results and stale speculative assistant text.
3. Preserve protected snippets in placeholders with deterministic restore metadata.
4. Add targeted unit tests for restored-session/tool-result and assistant-hypothesis routing.
5. Run context pruning tests, a small deterministic context-eval regression, formatting, diff checks, and selfdev build/reload.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented route_low_trust_context in the provider-message pruning path. It runs after duplicate/superseded failed result pruning and before stale errored input pruning.

Added deterministic restore placeholders with reason/trust/status/tool/chars/restore_id/protected_snippets metadata for old stale restored-session/tool-result blocks and speculative stale assistant text.

Targeted context_pruning tests passed 6/6. Deterministic TASK-88 regression passed negative/cache-confusion gates for recommended transforms and retained the known public-benchmark protected-retention caveat. selfdev TUI build passed and was reloaded.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented the first runtime context-hardening target selected from TASK-87 findings.

Changes:
- Added a provider-projection pruning pass in src/agent/context_pruning.rs that routes old low-trust tool results and speculative/stale assistant text into compact placeholders.
- Placeholders include provenance fields, trust/status labels, deterministic restore IDs, original character counts, and protected snippets for task IDs, constraints, acceptance criteria, restore handles, and relevant file paths.
- Kept the existing recent-message protection window so current turns are not rewritten.
- Added tests for stale restored-session/tool-result routing and stale speculative assistant text routing while preserving protected user/task facts.
- Documented TASK-88 runtime behavior and validation in docs/CONTEXT_PIPELINE_EVAL.md.

Validation:
- cargo fmt -- src/agent/context_pruning.rs
- cargo test --profile selfdev -p jcode context_pruning -- --nocapture
- python3 scripts/context_eval_matrix.py --mode local --scenario-kind negative --scenario-kind cache_confusion --scenario-kind public_benchmark --include-local-sessions false --tool-budget-chars 1200 --repetitions 1 --technique baseline --technique supersession_prune --technique provenance_routing --technique lazy_restore_handles --technique combined_p0 --out target/context-eval-matrix/task88-regression
- git diff --check
- selfdev build target=tui, followed by reload

Known caveat:
- The TASK-87/TASK-88 public-benchmark protected-retention gap remains. A future protected-span-aware summarizer is still needed before aggressive lazy restore is broadly applied.
<!-- SECTION:FINAL_SUMMARY:END -->
