---
id: TASK-81
title: Evaluate cache isolation and cross-project confusion controls
status: To Do
assignee:
  - '@jcode'
created_date: '2026-05-28 13:35'
updated_date: '2026-05-28 13:46'
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
Investigate whether shared caches can cause cross-project context confusion, stale retrieval, or performance degradation. Catalog and prototype cache-keying, namespacing, invalidation, provenance, and observability techniques that keep cache hits fast without mixing unrelated project/session state.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Cache-confusion risks are categorized by cache type, including repo maps, skeletons, token estimates, embeddings/retrieval, prompt projections, provider payloads, and tool/result caches
- [ ] #2 Candidate mitigations include concrete keying, namespacing, invalidation, provenance, and observability techniques
- [ ] #3 Evaluation metrics are defined for cross-project leakage, stale-hit rate, hit quality, latency, and cache size
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Research swarm refinement: cache risks categorized across repo maps, skeletons/summaries, token estimates, embeddings/retrieval, prompt projections/compaction, provider payload cache, tool/result caches, UI/render caches, and external API caches. Recommended structured keys with project_namespace, source/content/transform/environment identity; two-level cache design with global content-addressed blobs plus project/session namespace manifests; zero cross-project leakage/false hits as hard requirements. Added taxonomy, metrics, and JCODE integration points to docs/CONTEXT_PIPELINE_EVAL.md.

Second swarm added cache experiment gates: cache_cross_project fixture, zero cross-project leakage/false hits/provider protected-fact stale hits, hit quality parity against recompute, invalidation recall, miss penalty, and cache size growth thresholds. Consolidated in docs/CONTEXT_PIPELINE_EVAL.md.
<!-- SECTION:NOTES:END -->
