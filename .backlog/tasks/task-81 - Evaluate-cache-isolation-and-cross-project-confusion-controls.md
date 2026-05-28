---
id: TASK-81
title: Evaluate cache isolation for remaining cache types not covered by TASK-89
status: To Do
assignee:
  - '@jcode'
created_date: '2026-05-28 13:35'
updated_date: '2026-05-28 16:41'
labels:
  - context
  - evaluation
  - cache
  - reliability
dependencies:
  - TASK-79
documentation:
  - docs/CONTEXT_PIPELINE_EVAL.md
priority: medium
ordinal: 74000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
TASK-88 added runtime context-projection hardening and TASK-89 routed four runtime caches (message_render, semantic_embed, repo-map graph, openrouter disk memos) through a shared IsolationKey contract with cache_confusion eval coverage. TASK-90 closed the public_benchmark plateau. This task tracks the remaining items from the original cache-confusion taxonomy that have not yet been exercised at the runtime layer or the eval layer: skeletons/summaries, token estimates, tool/result caches, and non-openrouter external API caches. Also covers building the reusable cross-project leakage / stale-hit / hit-quality measurement harness that the original AC#3 anticipated but TASK-89 only proxied via fixtures.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Audit jcode runtime caches not covered by TASK-89 (skeletons/summaries, token-estimate caches, tool/result caches, non-openrouter external API caches) and confirm for each whether it (a) exists as a real cache layer, (b) is per-session-scoped already, or (c) needs IsolationKey routing
- [ ] #2 For each cache type from AC#1 that needs hardening, route it through the existing jcode-cache-isolation IsolationKey contract following the TASK-89 pattern (structured key + clear_*_for_isolation hook + invalidation wired from cache_invalidation.rs)
- [ ] #3 Extend scripts/context_pipeline_eval.py cache_confusion scenario with one fixture per newly hardened cache type, mirroring the active_* sentinel + foreign-marker pattern from TASK-89 AC#5, so cache_isolation technique strictly dominates baseline on each new fixture
- [ ] #4 Add a reusable cache-metrics harness (cross-project leakage rate, stale-hit rate, hit-quality vs recompute, latency, cache size growth) callable as a measurement step that can be replayed against future cache additions, not just embedded in scenario fixtures
- [ ] #5 cargo fmt --all clean and selfdev TUI build green; all targeted cache-isolation tests pass; eval matrix shows non-regression on the 5 existing scenarios bit-identical on gate metrics
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Research swarm refinement: cache risks categorized across repo maps, skeletons/summaries, token estimates, embeddings/retrieval, prompt projections/compaction, provider payload cache, tool/result caches, UI/render caches, and external API caches. Recommended structured keys with project_namespace, source/content/transform/environment identity; two-level cache design with global content-addressed blobs plus project/session namespace manifests; zero cross-project leakage/false hits as hard requirements. Added taxonomy, metrics, and JCODE integration points to docs/CONTEXT_PIPELINE_EVAL.md.

Second swarm added cache experiment gates: cache_cross_project fixture, zero cross-project leakage/false hits/provider protected-fact stale hits, hit quality parity against recompute, invalidation recall, miss penalty, and cache size growth thresholds. Consolidated in docs/CONTEXT_PIPELINE_EVAL.md.

Trimmed 2026-05-28 after TASK-90: original AC#1/2/3 (taxonomy + mitigation catalog + metric definitions) were satisfied as research output by the two swarm passes documented in docs/CONTEXT_PIPELINE_EVAL.md and then operationalized for 4 of the 9 cache types by TASK-88/89/90 (message_render, semantic_embed, repo-map graph, openrouter disk memos). Remaining scope is execution-only on the untouched cache types plus a reusable metrics harness.
<!-- SECTION:NOTES:END -->
