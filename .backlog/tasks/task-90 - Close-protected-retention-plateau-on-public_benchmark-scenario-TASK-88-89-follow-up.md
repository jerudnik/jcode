---
id: TASK-90
title: >-
  Close protected-retention plateau on public_benchmark scenario (TASK-88/89
  follow-up)
status: In Progress
assignee:
  - '@jcode'
created_date: '2026-05-28 16:32'
updated_date: '2026-05-28 16:34'
labels:
  - context
  - compaction
  - reliability
  - eval
  - fixtures
dependencies:
  - TASK-88
  - TASK-89
references:
  - scripts/context_pipeline_eval.py
  - scripts/context_eval_matrix.py
documentation:
  - docs/CONTEXT_PIPELINE_EVAL.md
priority: high
ordinal: 83000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
TASK-87 / TASK-88 / TASK-89 each left the public_benchmark scenario's protected_retention_ratio_min stuck at 0.875 across every technique, including baseline (no transformations). Root cause: the universal PROTECTED_TERMS list contains 'serious-callers-only' (the SCO development host name), and every other scenario fixture (oracle, negative, realistic, cache_confusion) mentions it exactly once, but the public_benchmark_blocks() fixture omits it entirely. This gives the metric a hard 7/8 = 0.875 ceiling by construction. No technique can ever pass the reliability gate on public_benchmark while the fixture is wrong. Fix the fixture so the metric ceiling is 1.0, then re-run the eval matrix and confirm baseline still leaks stale-foreign content (so cache_isolation still has work to do) while a proven-good technique (combined_p0 or cache_isolation) now actually crosses the protected-retention gate it should.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 public_benchmark_blocks() in scripts/context_pipeline_eval.py mentions 'serious-callers-only' at least once in a verified-trust block, matching the convention of the other scenario fixtures (oracle, negative, realistic, cache_confusion). The fix is colocated with bench-user where the SCO host context is most natural.
- [x] #2 Baseline protected_retention_ratio_min on public_benchmark reaches 1.0 in the eval matrix (target/context-eval-matrix/task90-*/summary.csv), confirming the metric ceiling is now achievable and the prior 0.875 plateau is gone.
- [x] #3 Cache_isolation and combined_p0 protected_retention_ratio_min on public_benchmark remain >= baseline post-fix (no technique regresses on protected retention). At least one technique now crosses the reliability_gates threshold on public_benchmark.
- [x] #4 Other scenarios (oracle, negative, synthetic, realistic, cache_confusion) are bit-identical pre/post fix on protected_retention_ratio_min, stale_foreign_retention_ratio_max, and practical_score_mean. The fix is scoped to one fixture only.
- [x] #5 Cargo fmt clean and no Rust source changes required (this is a Python fixture fix). Selfdev TUI build still green.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Establish baseline: run pre-fix matrix on public_benchmark + 4 control scenarios with baseline + cache_isolation + combined_p0 techniques (target/context-eval-matrix/task90-pre/).
2. Fix fixture: add 'serious-callers-only' mention to bench-user block in public_benchmark_blocks() (the verified-trust user prompt block), kept minimal and contextually plausible (this benchmark *was* replayed from a session on SCO).
3. Run post-fix matrix with same parameters (target/context-eval-matrix/task90-post/).
4. Diff pre/post on (technique, scenario_kind, include_local_sessions, tool_budget_chars) tuples for protected_retention_ratio_min / stale_foreign_retention_ratio_max / practical_score_mean / passes_reliability_gates. Assert: public_benchmark protected ratios go up to 1.0 across the board; all other scenarios bit-identical.
5. Cargo fmt clean, run targeted cache tests, commit fixture fix + AC checks + final summary.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Pre/post matrix diff (3 techniques x 6 scenarios x 2 include_local x 3 budgets = 108 cells per side, target/context-eval-matrix/task90-pre/ and task90-post/):

public_benchmark deltas (all 18 cells per technique uniform):
- baseline: protected 0.875 -> 1.0, stale 0.4286 (unchanged), score 55.09 -> 60.71, gate False -> False.
- cache_isolation: protected 0.875 -> 1.0, stale 0.4286 (unchanged), score 55.09 -> 60.71, gate False -> False.
- combined_p0: protected 0.875 -> 1.0, stale 0.0 (unchanged), score 73.19 -> 78.8, gate False -> True.

Other 5 scenarios (oracle, negative, synthetic, realistic, cache_confusion): 90 cells bit-identical pre/post on protected_retention_ratio_min, stale_foreign_retention_ratio_max, practical_score_mean, passes_reliability_gates. Fix is fully scoped.

cargo fmt --all --check clean. jcode-tui-messages cache:: 6/6. No Rust source changes; selfdev TUI binary unaffected.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Close the protected-retention plateau on the public_benchmark scenario by fixing a fixture omission: the universal PROTECTED_TERMS list contains 'serious-callers-only' (the SCO development host name added during TASK-87), and every other scenario fixture (oracle / negative / realistic / cache_confusion) mentions it exactly once, but public_benchmark_blocks() never did. The metric had a hard 7/8 = 0.875 ceiling by construction, so no technique — not even an oracle — could ever pass the reliability gate.

Changes:
- Added 'on serious-callers-only' to the bench-user verified-trust prompt in scripts/context_pipeline_eval.py::public_benchmark_blocks(). Single-line fixture fix, contextually plausible (the benchmark replays a session that ran on SCO).

Tests:
- Eval matrix re-run (108 cells per side, 3 techniques x 6 scenarios x 2 include_local x 3 budgets) under target/context-eval-matrix/task90-pre/ and task90-post/.
- public_benchmark: baseline / cache_isolation / combined_p0 all move protected_retention_ratio_min from 0.875 to 1.0. combined_p0 newly crosses passes_reliability_gates (False -> True) with practical_score_mean 73.19 -> 78.8 -- the first technique to actually pass the public_benchmark reliability gate.
- 90 other-scenario cells bit-identical pre/post on every gate metric. Fix is fully scoped.
- cargo fmt --all --check clean (no Rust changes). jcode-tui-messages cache:: 6/6.

User impact: the eval matrix now correctly distinguishes techniques on public_benchmark. The TASK-88 caveat documented in TASK-89's Final Summary is closed: the 0.875 plateau was a fixture artifact, not a runtime limitation. combined_p0 (the consensus winner from TASK-87) now passes the public_benchmark reliability gate as it should, which restores confidence in the scoring on the SWE-bench-style replay scenario.

Risks/follow-ups: cache_isolation still scores identically to baseline on public_benchmark because that scenario contains no runtime-axis-mismatched cache entries (only generic foreign content). That is correct -- the public_benchmark scenario tests stale-content rejection in general, not runtime cache isolation specifically; cache_confusion (TASK-89 AC#5) is the dedicated runtime-cache scenario.
<!-- SECTION:FINAL_SUMMARY:END -->
