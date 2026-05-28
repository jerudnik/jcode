---
id: TASK-86
title: Run real Anthropic model evaluation on controlled context artifacts
status: Done
assignee:
  - '@jcode'
created_date: '2026-05-28 14:33'
updated_date: '2026-05-28 14:46'
labels:
  - context
  - evaluation
  - models
  - anthropic
dependencies:
  - TASK-83
  - TASK-85
modified_files:
  - >-
    .backlog/tasks/task-86 -
    Run-real-Anthropic-model-evaluation-on-controlled-context-artifacts.md
priority: high
ordinal: 79000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Use the controlled context-eval artifacts and real model calls to compare baseline, trust_quarantine, and combined_p0 behavior on answerability and stale/foreign contamination. Initial target models: Sonnet 4.6, Opus 4.6, and Opus 4.7 if available through JCODE Anthropic auth.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Controlled deterministic finding is recorded in backlog/memory
- [x] #2 Real model eval runs against selected controlled artifacts for available Anthropic models
- [x] #3 Results are summarized with pass rates, contamination findings, and artifact paths
- [x] #4 Secret scan/report artifacts are generated or verified before sharing
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Record the controlled deterministic finding in TASK-86 notes and Serena memory.
2. Identify selected controlled artifact directories for negative and realistic scenarios.
3. Probe requested Anthropic model aliases through jcode-run and run bounded model-eval calls for available models.
4. Generate reports/secret scans and summarize pass/contamination results.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Controlled deterministic battery target/context-eval/controlled-battery-shareable-20260528-143102: oracle baseline passed; negative baseline failed stale gate; negative trust_quarantine and combined_p0 passed; synthetic and realistic combined_p0 passed; realistic trust_quarantine passed; boundary_gate and tool_budget failed realistic stale gates. Interpretation: combined_p0 is the best overall candidate, trust_quarantine is the core reliability component, and tool_budget alone is unsafe.

Anthropic model aliases probed successfully through jcode-run: claude-sonnet-4-6, claude-opus-4-6, and claude-opus-4-7. Shorthand aliases resolved unexpectedly to Opus 4.6, so explicit canonical model IDs were used.

Initial controlled model eval target/context-eval/anthropic-model-eval-20260528-143622 ran 18 calls/model over baseline, trust_quarantine, and combined_p0 on negative and realistic contexts. Sonnet 4.6 and Opus 4.7 passed 18/18 with no forbidden rows. Opus 4.6 scored 16/18 due to strict substring scoring on baseline contexts; manual inspection showed the model refused stale instructions and only quoted forbidden text while explaining it was contamination.

Full-context candidate-only eval target/context-eval/anthropic-candidate-fullctx-20260528-144213 ran 12 calls/model over trust_quarantine and combined_p0 contexts with max-context-chars 120000. Sonnet 4.6, Opus 4.6, and Opus 4.7 all passed 12/12 with zero forbidden rows. Secret scans passed for both model-eval artifact roots.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Ran controlled real-model evaluation against Anthropic models using JCODE subscription-backed Claude auth.

Changes / records:
- Recorded controlled deterministic findings in TASK-86 and Serena memories.
- Probed canonical Anthropic model IDs: claude-sonnet-4-6, claude-opus-4-6, and claude-opus-4-7.
- Created curated controlled context sets under target/context-eval/anthropic-controlled-curated and candidate-only full-context sets under target/context-eval/anthropic-controlled-candidates.

Results:
- Initial run target/context-eval/anthropic-model-eval-20260528-143622: Sonnet 4.6 and Opus 4.7 passed 18/18 with zero forbidden rows; Opus 4.6 strict-scored 16/18 due to safe explanatory quoting on baseline contexts.
- Full-context candidate-only run target/context-eval/anthropic-candidate-fullctx-20260528-144213: Sonnet 4.6, Opus 4.6, and Opus 4.7 each passed 12/12 with zero forbidden rows over trust_quarantine and combined_p0 contexts.
- Secret scans passed for both model-eval artifact roots.

Interpretation:
- Real model results reinforce the deterministic finding that trust_quarantine and combined_p0 are safe candidates on the controlled fixtures.
- Baseline can sometimes be handled safely by strong models, but it still exposes contamination and should not replace deterministic pruning gates.
<!-- SECTION:FINAL_SUMMARY:END -->
