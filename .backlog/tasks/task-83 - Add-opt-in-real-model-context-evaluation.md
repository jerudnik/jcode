---
id: TASK-83
title: Add opt-in real model context evaluation
status: Done
assignee:
  - '@jcode'
created_date: '2026-05-28 13:55'
updated_date: '2026-05-28 13:58'
labels:
  - context
  - evaluation
  - models
  - reliability
dependencies:
  - TASK-79
  - TASK-82
references:
  - scripts/context_pipeline_eval.py
  - scripts/context_eval_matrix.py
documentation:
  - docs/CONTEXT_PIPELINE_EVAL.md
modified_files:
  - scripts/context_model_eval.py
  - docs/CONTEXT_PIPELINE_EVAL.md
priority: high
ordinal: 76000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add an opt-in model-backed evaluation layer for the context/cache experiment harness. The evaluator should reuse generated context artifacts, ask deterministic answerability and contamination questions, call a configured real model with cost limits, and score expected versus forbidden answers without leaking secrets into logs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Evaluator can score existing context-eval or context-eval-matrix artifacts with real model calls behind explicit opt-in flags
- [x] #2 Evaluator records prompts, sanitized responses, expected/forbidden scoring, latency, and provider/model metadata without exposing API keys
- [x] #3 Docs explain provider configuration, cost controls, safety boundaries, and a tiny smoke-test workflow
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add a stdlib opt-in evaluator that reads context artifacts and asks fixed answerability/contamination questions.\n2. Support OpenAI-compatible and Anthropic HTTP APIs via environment variables, with max-call and max-context controls.\n3. Score expected and forbidden terms from responses, store sanitized artifacts, and avoid logging secrets.\n4. Document provider setup, cost controls, and smoke-test commands.\n5. Run a tiny smoke if credentials are discoverable without exposing them.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented scripts/context_model_eval.py. It reads *.context.json artifacts, asks deterministic default or custom questions, supports jcode-run subscription-backed calls plus OpenAI-compatible/OpenRouter/Anthropic HTTP APIs, caps calls/context/output, scores expected_any and forbidden_any response terms, and writes sanitized results without API keys.

Credential check: raw OPENAI_API_KEY/ANTHROPIC_API_KEY/OPENROUTER_API_KEY were unset, but jcode run --json --tool-profile none -p openai works with existing subscription auth. Real-model smoke against target/context-eval-matrix/sco-smoke using gpt-5.5 completed 6 calls with pass_rate=1.0 across baseline and combined_p0. Note: baseline still fails deterministic stale-retention gates even if the model answered the stale-contamination question safely in this tiny smoke.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added opt-in real-model evaluation for context experiment artifacts.\n\nChanges:\n- Added scripts/context_model_eval.py to score existing *.context.json artifacts with deterministic answerability/contamination questions.\n- Supports subscription-backed JCODE CLI calls via --provider jcode-run, plus OpenAI-compatible/OpenRouter/Anthropic HTTP APIs via environment variables.\n- Adds cost controls for max calls, max contexts, context chars, output tokens, and stores sanitized responses/results without API keys.\n- Documented real-model usage, JCODE subscription-backed smoke, API-key alternatives, custom question schema, and interpretation guidance in docs/CONTEXT_PIPELINE_EVAL.md.\n\nValidation:\n- python3 -m py_compile scripts/context_model_eval.py\n- Dry run with --max-calls 0 against target/context-eval-matrix/sco-smoke\n- jcode run --json --tool-profile none --quiet -p openai 'Answer exactly: model-eval-smoke-ok'\n- python3 scripts/context_model_eval.py --artifacts target/context-eval-matrix/sco-smoke --provider jcode-run --jcode-provider openai --model gpt-5.5 --technique baseline --technique combined_p0 --max-contexts 2 --max-calls 6 --max-context-chars 8000 --max-output-tokens 128 --timeout-seconds 180 --out target/context-eval-model/sco-smoke-jcode-run-compare
<!-- SECTION:FINAL_SUMMARY:END -->
