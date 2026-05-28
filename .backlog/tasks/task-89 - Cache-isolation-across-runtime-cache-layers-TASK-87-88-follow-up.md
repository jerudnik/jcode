---
id: TASK-89
title: Cache isolation across runtime cache layers (TASK-87/88 follow-up)
status: In Progress
assignee:
  - '@jcode'
created_date: '2026-05-28 15:41'
updated_date: '2026-05-28 16:18'
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
priority: high
ordinal: 82000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Apply the cache_isolation technique selected by TASK-87 to runtime caches. TASK-87 found cache_isolation is the only technique that closes the cache_confusion scenario, and it must be applied inside cache layers (not just message pruning). Introduce a shared IsolationKey wrapping session_id, canonicalized workspace_root, provider, model, content_hash, trust_tier, and a schema_version, and route the existing in-memory and disk-memo caches through it. Invalidate on session resume, workspace switch, and provider switch. Extend scripts/context_pipeline_eval.py cache_confusion to exercise the runtime path.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Introduce a shared cache IsolationKey type (or equivalent helper) containing session_id, canonicalized workspace_root, provider, model, content_hash, trust_tier, and schema_version; runtime caches that store provider/projection-sensitive data use it as their key prefix.
- [x] #2 Runtime caches affected (semantic_embed_cache in src/compaction.rs, GraphCache in src/memory/cache.rs, message render cache in crates/jcode-tui-messages/src/cache.rs, openrouter DISK_CACHE_MEMO/ENDPOINTS_DISK_CACHE_MEMO in crates/jcode-provider-openrouter/src/lib.rs) miss on session/workspace/provider mismatch instead of returning foreign entries.
- [x] #3 Cache eviction policy: explicit invalidation on session resume, workspace switch, and provider/model change events, plus existing TTL/LRU bounds preserved.
- [ ] #4 Unit tests per touched cache assert key composition and miss-on-mismatch for session/workspace/provider/trust_tier; one integration-style test simulates a session resume across two workspaces and confirms no foreign content reaches projection.
- [ ] #5 scripts/context_pipeline_eval.py cache_confusion scenario is extended to exercise the runtime cache path (not just message pruning) and the deterministic eval matrix shows cache_confusion passes without regressing negative or public_benchmark scenarios.
- [ ] #6 Selfdev TUI build + reload succeed; cargo fmt and targeted tests pass; the protected-retention caveat from TASK-88 is not regressed by this change.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Inventory affected caches and their existing keys, invalidation events, and TTL/LRU bounds:
   - src/compaction.rs::semantic_embed_cache (keyed by u64 of truncated text)
   - src/memory/cache.rs::GraphCache (keyed by PathBuf, mtime check)
   - crates/jcode-tui-messages/src/cache.rs::MESSAGE_CACHE (keyed by MessageCacheKey with width/diff/hash/mermaid)
   - crates/jcode-provider-openrouter/src/lib.rs::DISK_CACHE_MEMO, ENDPOINTS_DISK_CACHE_MEMO (keyed by PathBuf)
2. Introduce a new crate-local shared helper in jcode-context-types (or, if cheaper, an internal module under src/agent/) named cache_isolation with:
   - IsolationKey { session_id, workspace_root_canonical, provider, model, content_hash: u64, trust_tier, schema_version: u32 }
   - canonicalize_workspace_root(&Path) -> String helper
   - TrustTier enum (mirrors the trust enum used in context_pruning.rs)
   - SCHEMA_VERSION constant bumped here whenever the keying contract changes
   - Hash/Eq derives + a 64-bit fingerprint accessor used by hash-only key stores
3. Route each cache through IsolationKey:
   a. semantic_embed_cache: change key from u64 to (IsolationKey-fingerprint, content_hash) or wrap value with the full IsolationKey for cross-checking; add session_id/workspace/provider arguments to ensure_semantic_embedding callers, defaulting to context obtained from CompactionManager owner.
   b. GraphCache: prefix entries with the IsolationKey (session/workspace) so resume across workspaces cannot hit a stale graph from another session even if the path collides.
   c. MESSAGE_CACHE: extend MessageCacheKey with isolation fields (session_id hash, workspace hash, schema_version). This cache is render-only so trust_tier/provider are not needed; document the rationale.
   d. openrouter disk-memo caches: extend memo key from PathBuf to (PathBuf, IsolationKey-fingerprint over provider+model+schema) so a provider/model switch invalidates stale catalog memoization even when the disk file path is unchanged.
4. Add explicit invalidation hooks:
   - on session resume: clear caches owned by that session-id
   - on workspace switch: invalidate entries for the previous workspace_root
   - on provider/model change: invalidate provider-keyed entries
   Wire these into the existing reset/restore paths (CompactionManager::reset, GraphCache, MESSAGE_CACHE, provider model change in openrouter).
5. Tests:
   - per-cache unit tests: same content_hash with differing session_id / workspace_root / provider / model / trust_tier / schema_version produce cache misses; same IsolationKey produces a hit
   - integration test (Rust): simulate two sessions in two workspaces with overlapping content_hash and verify no cross-bleed reaches the provider projection (reuse the prune_provider_messages pipeline path)
   - run cargo fmt + cargo test for the touched modules
6. Extend scripts/context_pipeline_eval.py:
   - In the cache_isolation technique branch, add a runtime-cache simulation that mirrors the IsolationKey contract so cache_confusion exercises the runtime path
   - Re-run scripts/context_eval_matrix.py for cache_confusion / negative / public_benchmark and confirm cache_confusion still passes and no other scenario regresses (compare against target/context-eval-matrix from TASK-87/88)
7. Selfdev TUI build + reload using selfdev build target=tui, then selfdev reload; verify the binary still works on a smoke session.
8. Commit incrementally per acceptance criterion, push at the end. Update task with notes and final summary.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
AC#1 done in a668b9b8 (jcode-cache-isolation crate).
AC#2 done across four commits:
- 65d2c119 semantic_embed_cache (HIGH, provider context: session+workspace+provider+model+trust_tier+schema)
- 86583a13 MESSAGE_CACHE (MED, render-only: session+workspace+schema)
- dc751843 GraphCache (LOW defensive: path+schema)
- 9a2fb88f openrouter DISK_CACHE_MEMO + ENDPOINTS_DISK_CACHE_MEMO (LOW defensive: path+schema)
Each commit includes a new unit test asserting Eq/Hash isolation across the relevant axes; jcode-tui-messages 7/7, jcode-compaction-core 10/10, jcode memory:: 24/24, jcode-provider-openrouter 5/5 pass.

AC#3 done in de640cf6: added clear_message_cache (+_for_isolation), clear_graph_cache, clear_disk_cache_memos; wired via new src/server/cache_invalidation.rs at handle_resume_session and apply_set_model. semantic_embed_cache already covered by CompactionManager::reset() on reset_provider_session. Tests: jcode-tui-messages 5/5, jcode memory::cache:: 2/2, jcode-provider-openrouter 6/6 pass.
<!-- SECTION:NOTES:END -->
