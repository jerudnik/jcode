---
id: TASK-84
title: Add experiment infrastructure before context testing
status: Done
assignee:
  - '@jcode'
created_date: '2026-05-28 14:15'
updated_date: '2026-05-28 14:20'
labels:
  - context
  - evaluation
  - experiments
  - infra
dependencies:
  - TASK-79
  - TASK-82
  - TASK-83
documentation:
  - docs/CONTEXT_PIPELINE_EVAL.md
modified_files:
  - scripts/context_experiment.py
  - scripts/context_experiment_report.py
  - scripts/context_artifact_secret_scan.py
  - docs/CONTEXT_PIPELINE_EVAL.md
priority: high
ordinal: 77000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement supporting infrastructure for reliable context/cache experiments before broader testing: run manifests/registry, report generation, cache cross-project fixtures, determinism checks, redaction scanning, and any small supporting glue needed to trust results.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Experiment runs can be described by manifests and recorded with environment/run metadata
- [x] #2 Reports summarize deterministic/model results, gate failures, and recommendations
- [x] #3 Cache cross-project fixtures and determinism checks can be generated/run from scripts
- [x] #4 Artifact redaction scanning catches likely secrets before reports are shared or committed
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Dispatch agents with file ownership for manifest/registry, report generation, fixtures/determinism, redaction scanning, and docs/glue.
2. Consolidate scripts into a coherent stdlib-only experiment support layer.
3. Run compile/smoke validations for each script against existing context-eval artifacts.
4. Update docs and TASK-84 acceptance criteria, then commit.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added stdlib-only HTML/Markdown report generation in scripts/context_experiment_report.py for context-eval artifacts.

Reporter consumes deterministic matrix.json and optional model_eval/results.json, summarizes metrics, gate failures, and recommendation.

Added manifest and run registry support in scripts/context_experiment.py: create-manifest, validate-manifest, register-run, and list-runs. The commands record git/environment metadata, scenario settings, artifact checksums, inferred techniques, and summary metrics from context-eval artifacts.

Validated manifest infrastructure against a synthetic context_pipeline_eval smoke run under target/context-eval/task84-manifest-smoke.

Added cache cross-project fixture generation and determinism hashing in scripts/context_experiment.py. Integrated smoke generated repo_alpha/repo_beta fixtures and confirmed stable hashes across 3 iterations.

Added artifact redaction scanner in scripts/context_artifact_secret_scan.py. Positive synthetic leak fixture exited nonzero and detected OpenAI-style key, private key block, assignment token, and sentinel secret; integrated smoke passed with explicit allowlist for synthetic canaries.

Ran integrated TASK-84 smoke covering pipeline artifacts, manifest create/validate/register/list, cache fixture generation, determinism, report generation, redaction scan, py_compile, and whitespace checks.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added the pre-testing experiment infrastructure needed to make context-management runs auditable and safer to share.

Changes:
- Added scripts/context_experiment.py for manifests, run registries, cache cross-project fixture generation, and determinism hashing.
- Added scripts/context_experiment_report.py for Markdown/HTML reports over deterministic and optional model-eval outputs.
- Added scripts/context_artifact_secret_scan.py for recursive artifact/report secret scanning with JSON and text summaries.
- Documented the new workflow in docs/CONTEXT_PIPELINE_EVAL.md and tracked TASK-84 metadata.

Validation:
- Integrated smoke: pipeline run, manifest create/validate/register/list, cache fixture generation, determinism over 3 iterations, report generation, allowlisted redaction scan.
- Positive redaction fixture detected synthetic OpenAI key, private key block, assignment token, and sentinel secret with nonzero exit.
- python3 -m py_compile for all new scripts.
- git diff --check.
- selfdev build target=tui passed.
<!-- SECTION:FINAL_SUMMARY:END -->
