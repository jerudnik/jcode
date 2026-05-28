---
id: TASK-79
title: Build context-management evaluation pipeline for VM replay experiments
status: Done
assignee:
  - '@jcode'
created_date: '2026-05-28 12:32'
updated_date: '2026-05-28 13:33'
labels:
  - context
  - evaluation
  - experiments
  - ssh
  - vm
  - compaction
  - reliability
dependencies:
  - TASK-27
  - TASK-34
  - TASK-42
  - TASK-55
references:
  - 'Serena memory: compaction/dcp_research_task27'
documentation:
  - docs/CONTEXT_PIPELINE_EVAL.md
modified_files:
  - scripts/context_pipeline_eval.py
  - docs/CONTEXT_PIPELINE_EVAL.md
priority: high
ordinal: 59000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create a deterministic experimental pipeline for evaluating candidate context-management techniques under realistic conditions. The pipeline should be able to use the host `serious-callers-only` over SSH, spin up an isolated VM or isolated worktree/session runner, launch jcode, apply rough prototype variants of candidate context pipeline techniques, replay generated scenarios from local session history/public fixtures, and output a baseline evaluation matrix for implementation prioritization.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Pipeline can provision or connect to an isolated VM/worktree on `serious-callers-only` over SSH without requiring interactive prompts.
- [x] #2 Pipeline can run rough context-management prototype variants against replayed/synthetic scenarios and collect deterministic metrics into an evaluation matrix.
- [x] #3 Experiment docs describe inputs, outputs, required environment variables, safety boundaries, and how to interpret results.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add a deterministic local experiment runner that can generate synthetic/local replay scenarios and apply rough prototype context transforms.
2. Add a non-interactive SSH/rsync remote runner for `serious-callers-only`, with an optional host-side VM/provision command hook.
3. Emit JSON/CSV evaluation matrix artifacts for token savings, protected-term retention, latency, placeholders, skeletons, and practical score.
4. Document safety boundaries, local/remote usage, required environment variables, and interpretation thresholds.
5. Smoke-test locally and remotely, then record validation in the backlog task.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented the first deterministic harness version with local scenario generation, prototype transforms, metrics, CSV/JSON artifacts, and remote SSH/rsync execution.

Validated non-interactive SSH to `serious-callers-only` and completed a remote smoke run using `/tmp/jcode-context-eval-smoke` as the staging directory.

Added and validated higher-fidelity realistic replay mode. Local realistic run: combined_p0 score 79.67, trust_quarantine 77.98, boundary_gate 71.03, tool_budget 58.2. Remote serious-callers-only realistic run: combined_p0 score 79.96, trust_quarantine 78.37, boundary_gate 71.03, tool_budget 58.28. The ranking broadly holds, but trust quarantine appears more important than raw budgeting for reliability.

Consolidated pending context-management prototype targets from TASK-80 into docs/CONTEXT_PIPELINE_EVAL.md as the main evaluation ledger. Pending candidates now sit alongside prior results: goal/task retention ledger, supersession pruning, attention preamble/context index, lazy restore handles, pinned spans, recency/importance scoring, provenance/trust routing, scratchpad, and memory-adjacent session-local/TTL/source-bound/conflict-aware experiments.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Set up a deterministic context-management evaluation harness for TASK-79.

Changes:
- Added `scripts/context_pipeline_eval.py`, a stdlib Python runner that generates synthetic/local replay scenarios, applies rough prototype transforms, and emits `matrix.json`, `matrix.csv`, and transformed context artifacts.
- Added remote SSH/rsync execution support targeting `serious-callers-only`, with optional `JCODE_CONTEXT_EVAL_VM_START_CMD` for host-side VM/provisioning integration.
- Added `docs/CONTEXT_PIPELINE_EVAL.md` with local/remote usage, safety boundaries, outputs, interpretation guidance, and initial decision thresholds.

Prototype techniques covered:
- stable XML/status tiering
- boundary gatekeeping
- tool-output budgets with restore handles
- duplicate tool-output pruning
- trust-tier quarantine
- rough Rust skeletonization
- combined P0 pipeline

Validation:
- `python3 scripts/context_pipeline_eval.py run-local --out target/context-eval/smoke`
- `python3 -m py_compile scripts/context_pipeline_eval.py`
- `git diff --check`
- `ssh -o BatchMode=yes -o ConnectTimeout=5 serious-callers-only 'echo context-eval-ssh-ok'`
- `python3 scripts/context_pipeline_eval.py run-remote --host serious-callers-only --remote-dir /tmp/jcode-context-eval-smoke --out target/context-eval/remote-smoke`

Follow-up higher-fidelity evaluation mode added: realistic scenarios now sample recent JCODE session snapshots, preserve early intent plus latest state, inject controlled stale/foreign distractors, and score stale/foreign retention plus restore-handle coverage. Local and serious-callers-only smoke runs kept combined_p0 first, moved trust_quarantine close behind, and showed that tool_budget alone preserves stale distractors despite token savings.

Consolidated the pending prototype/evaluation backlog into the main context pipeline evaluation document so completed results and next targets remain in one place. TASK-80 remains the follow-up planning pointer, while docs/CONTEXT_PIPELINE_EVAL.md now carries the unified pending prototype ledger and recommended next batch.
<!-- SECTION:FINAL_SUMMARY:END -->
