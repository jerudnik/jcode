---
id: TASK-HIGH.1
title: Cache isolation across runtime cache layers (TASK-87/88 follow-up)
status: To Do
assignee:
  - '@jcode'
created_date: '2026-05-28 15:40'
labels:
  - context
  - compaction
  - reliability
  - runtime
  - cache
dependencies:
  - TASK-87
  - TASK-88
references:
  - src/agent/context_pruning.rs
  - src/compaction.rs
  - src/memory/cache.rs
  - crates/jcode-tui-messages/src/cache.rs
  - crates/jcode-provider-openrouter/src/lib.rs
  - scripts/context_pipeline_eval.py
  - scripts/context_eval_matrix.py
documentation:
  - docs/CONTEXT_PIPELINE_EVAL.md
  - docs/CONTEXT_HARDENING_RESUME_PROMPT.md
parent_task_id: TASK-HIGH
ordinal: 82000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Apply the cache_isolation technique selected by TASK-87 to runtime caches. TASK-87 found cache_isolation is the only technique that closes the cache_confusion scenario, and it must be applied inside cache layers (not just message pruning). Introduce a shared IsolationKey wrapping session_id, canonicalized workspace_root, provider, model, content_hash, trust_tier, and a schema_version, and route the existing in-memory and disk-memo caches through it. Invalidate on session resume, workspace switch, and provider switch. Extend scripts/context_pipeline_eval.py cache_confusion to exercise the runtime path.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Introduce a shared cache IsolationKey type (or equivalent helper) containing session_id, canonicalized workspace_root, provider, model, content_hash, trust_tier, and schema_version; runtime caches that store provider/projection-sensitive data use it as their key prefix.
- [ ] #2 Runtime caches affected (semantic_embed_cache in src/compaction.rs, GraphCache in src/memory/cache.rs, message render cache in crates/jcode-tui-messages/src/cache.rs, openrouter DISK_CACHE_MEMO/ENDPOINTS_DISK_CACHE_MEMO in crates/jcode-provider-openrouter/src/lib.rs) miss on session/workspace/provider mismatch instead of returning foreign entries.
- [ ] #3 Cache eviction policy: explicit invalidation on session resume, workspace switch, and provider/model change events, plus existing TTL/LRU bounds preserved.
- [ ] #4 Unit tests per touched cache assert key composition and miss-on-mismatch for session/workspace/provider/trust_tier; one integration-style test simulates a session resume across two workspaces and confirms no foreign content reaches projection.
- [ ] #5 scripts/context_pipeline_eval.py cache_confusion scenario is extended to exercise the runtime cache path (not just message pruning) and the deterministic eval matrix shows cache_confusion passes without regressing negative or public_benchmark scenarios.
- [ ] #6 Selfdev TUI build + reload succeed; cargo fmt and targeted tests pass; the protected-retention caveat from TASK-88 is not regressed by this change.
<!-- AC:END -->
