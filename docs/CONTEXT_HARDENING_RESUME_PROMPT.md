# Resume jcode context-hardening implementation

> **Status update (2026-05-28, post-handoff):** TASK-89 (`cache_isolation`
> across runtime cache layers) shipped as planned in this document, see
> `docs/CONTEXT_PIPELINE_EVAL.md` "TASK-89 runtime cache isolation
> implementation notes" for the actual outcome and the `jcode-cache-isolation`
> crate. TASK-90 was **repurposed**: it became the `public_benchmark`
> protected-retention fixture trim (see same doc, "TASK-90 public_benchmark
> protected-retention plateau") because the 0.875 plateau turned out to be a
> fixture omission, not a runtime limitation, and closing it unblocked
> `combined_p0` passing the public_benchmark reliability gate. The
> "protected-span-aware lazy restore" work described in the TASK-90 section
> below is therefore **un-IDed and unstarted**; it remains a valid next
> candidate but is no longer reserved under TASK-90. TASK-81 has been
> trimmed to track only the cache types not yet routed through
> `IsolationKey` (skeletons/summaries, token estimates, tool/result caches,
> non-openrouter external API caches) plus a reusable cache-metrics harness.
> Everything below this banner is the original pre-flight prompt, preserved
> for historical context.

## Repository and starting state

- Workspace: `/Users/jrudnik/labs/jcode`
- Branch: `dev`
- Last commit: `a8d589f2 feat: harden runtime context pruning`
- Backlog status at handoff:
  - TASK-86 Done (real Anthropic eval)
  - TASK-87 Done (deterministic eval of 10 techniques across 3 scenario kinds)
  - TASK-88 Done (runtime low-trust provenance routing in `src/agent/context_pruning.rs`)
- Pending: implement the remaining techniques that passed TASK-87 evaluation but are not yet in the runtime path.

## Required reading before any code change

Read the following in order. Do not start implementing until you have summarized back what each one constrains:

1. Serena memories under `compaction/`:
   - `remaining_technique_eval_task87.md` (which techniques passed, which failed, why)
   - `runtime_context_hardening_task88.md` (what is already wired and how)
   - `remaining_implementation_plan_post_task88.md` (forward plan, the authoritative spec)
   - `controlled_context_eval_findings_task86.md` (real-model eval guardrails)
2. `docs/CONTEXT_PIPELINE_EVAL.md`
3. `src/agent/context_pruning.rs` end-to-end (especially `route_low_trust_context`, `PruneStats`, `RECENT_MESSAGES_TO_PROTECT`)
4. `scripts/context_pipeline_eval.py` and `scripts/context_eval_matrix.py` (entrypoints, scenario kinds, technique list)
5. `backlog task 86 --plain`, `backlog task 87 --plain`, `backlog task 88 --plain`

## Operating constraints

- Reliability-first local fork. Diverging from upstream is acceptable when justified by eval.
- Ground every claim in `git`, tests, backlog, or memories. No speculation.
- Deterministic eval before broad rollout. Adversarial scenarios + controls always included.
- Low-effort / low-risk / high-reward first.
- Track durable state in backlog + Serena memories.
- Especially watch for hallucinations, stale/foreign context, cross-session leaks, provider reasoning replay.
- SSH commit signing key (`/Users/jrudnik/.ssh/bitwarden.pub`) was missing last session. If it still is, commit with `--no-gpg-sign` or fix the key first.

## Work to do, in order

### TASK-89: cache_isolation across cache layers (do first)

Why first: only technique that solved the `cache_confusion` scenario in TASK-87; lowest blast radius; unlocks safe TASK-90.

Create with:
```
backlog task create "Implement cache_isolation across runtime cache layers" \
  -d "Apply cache_isolation (TASK-87 finding) to repo maps, skeletons, token estimates, embeddings, prompt projections, provider payload caches, and tool/result caches so session/workspace/provider switches cannot return foreign cache entries." \
  --ac "All identified caches share an isolation helper that composes (session_id, workspace_root canonicalized abs path, provider, model, content_hash, trust_tier, schema_version) into the cache key" \
  --ac "Session resume, workspace switch, and provider switch all produce cache misses for previously cached entries from the prior context" \
  --ac "Eviction policy combines TTL with explicit invalidation on session/workspace/provider change events" \
  --ac "Per-cache unit tests verify key composition and miss-on-mismatch for each of the listed cache layers" \
  --ac "Integration test simulates session resume across two workspaces and asserts no foreign content reaches the provider projection" \
  --ac "scripts/context_pipeline_eval.py cache_confusion scenario is extended to exercise the runtime cache path (not just message pruning) and passes" \
  --ac "Deterministic matrix run committed under target/context-eval-matrix/task89-* shows cache_confusion passing without regressing negative or public_benchmark" \
  --ac "selfdev build target=tui passes and reload is green" \
  --ac "Serena memory under compaction/ records implementation and eval outcome" \
  --priority high
```

Implementation outline:
1. Inventory current caches. Start from `rg -n "HashMap|DashMap|Mutex<.*HashMap|Lru" src/ | rg -i "cache|map|skeleton|embed|projection|token|payload"`. Confirm against `src/agent/`, `src/provider/`, `src/tui/`, `src/memory/`, `src/repo/` (or equivalents).
2. Add an `IsolationKey` struct with the seven fields above plus a `Display` that produces a stable string. Place it next to the most central cache, likely under `src/agent/` or a new `src/cache/` module.
3. Refactor each cache to wrap its existing key in `IsolationKey`. Do not delete the inner key; compose.
4. Wire change events: session change, workspace change, provider/model change all trigger explicit invalidation (or bump the in-memory `schema_version` epoch for that scope).
5. Tests: one unit test per cache file proving (a) key composition, (b) miss when any isolation field differs. One integration test under `tests/` that boots two workspaces in one process and asserts isolation. Extend `scripts/context_pipeline_eval.py` `cache_confusion` to drive the runtime cache and write a new matrix run.
6. Run `cargo test --profile selfdev -p jcode` for touched modules, then full `cargo test --profile selfdev`.
7. `selfdev build target=tui` then `selfdev reload`.
8. Commit, update Serena memory, mark task Done with `--final-summary`.

Evaluation criteria for TASK-89 done:
- All ACs checked.
- `target/context-eval-matrix/task89-*` shows cache_confusion passing and no regression on negative or public_benchmark vs `task87-full-rerun` baseline.
- New tests fail with isolation removed (verify by temporary revert).
- No direct `HashMap<ContentHash, _>` style caches remain outside the isolation helper.

### TASK-90: protected-span-aware lazy restore (do after TASK-89)

Why second: `lazy_restore_handles` was the highest-scoring technique in TASK-87 but regressed `public_benchmark` needles. Protected-span detection plus session-local restore store unlocks it safely. Depends on TASK-89 isolation keys for the restore store.

Create with:
```
backlog task create "Implement protected-span-aware lazy restore" \
  -d "Extend the TASK-88 routing pass with placeholder restoration when later turns reference a restore_id, a protected path, or a protected needle pattern. Use TASK-89 isolation keys for the session-local restore store. Goal: enable lazy_restore_handles without regressing public_benchmark." \
  --ac "Protected-span detector recognizes restore handle text, TASK ids, do-not/must constraints, AC checklist items, paths under src/, and public_benchmark needle markers" \
  --ac "Session-local restore store keyed by IsolationKey + restore_id holds the original block content for placeholders emitted by route_low_trust_context" \
  --ac "Restoration triggers when a later user or assistant turn references a restore_id, a protected path, or a protected needle that is currently routed" \
  --ac "Unit test: placeholder then later message containing restore_id triggers restoration before provider call" \
  --ac "Unit test: needle reference triggers restoration without explicit restore_id" \
  --ac "Unit test: unrelated later message does not trigger restoration" \
  --ac "Deterministic matrix run committed under target/context-eval-matrix/task90-* shows lazy_restore_handles + protected_spans beats combined_p0 on aggregate without regressing negative or cache" \
  --ac "selfdev build target=tui passes and reload is green" \
  --ac "Serena memory under compaction/ records implementation and eval outcome" \
  --priority high
```

Implementation outline:
1. Add a `RestoreStore` keyed by `(IsolationKey, restore_id)` holding the original `Block` and metadata. Capacity-bounded, TTL-bounded, session-scoped.
2. In `route_low_trust_context`, before emitting a placeholder, insert the original block into the store and stamp the placeholder with the same `restore_id` already generated.
3. Add a `restore_protected_spans` pass that runs after pruning but before provider projection. It scans the most recent turns for restore triggers and re-materializes blocks from the store in place of their placeholders.
4. Build the protected-span detector as a small set of regex/pattern matchers under `src/agent/context_pruning.rs` (or a sibling module). Public_benchmark needle markers are defined by `scripts/context_pipeline_eval.py`; mirror those patterns.
5. Tests in `src/agent/context_pruning.rs` alongside the TASK-88 tests. Use the existing scaffolding.
6. Eval: add a `lazy_restore_handles_protected` technique variant in `scripts/context_eval_matrix.py` that composes `provenance_routing` + `lazy_restore_handles` + protected-span detection. Run full matrix; commit results.
7. `cargo test --profile selfdev -p jcode context_pruning -- --nocapture` then full test suite.
8. `selfdev build target=tui` then `selfdev reload`.
9. Commit, update Serena memory, mark task Done with `--final-summary`.

Evaluation criteria for TASK-90 done:
- All ACs checked.
- `target/context-eval-matrix/task90-*` shows the new combined variant beating `combined_p0` aggregate and matching or exceeding `lazy_restore_handles` raw score, without regressing negative or cache_confusion or public_benchmark.
- Restoration is provably bounded (no unbounded growth of `RestoreStore`).

### TASK-91 (optional, defer unless explicitly requested)

Continuity primitives bundle (`goal_task_ledger`, `attention_index`, `pinned_spans`, `scratchpad`) behind a feature flag. Quality-of-life, not safety. Only start after TASK-89 and TASK-90 are Done and stable.

## Final completion criteria for the whole effort

All of the following must be true to declare context-hardening implementation complete:

1. TASK-89 and TASK-90 backlog tasks are Done with all ACs checked and Final Summaries set.
2. `target/context-eval-matrix/` contains committed runs for `task89-*` and `task90-*` that:
   - Pass `negative`, `cache_confusion`, and `public_benchmark` scenarios for the recommended runtime configuration.
   - Show no regression vs `task87-full-rerun` on any scenario.
3. `cargo test --profile selfdev` is green.
4. `selfdev build target=tui` is green and reload succeeds.
5. `src/agent/context_pruning.rs` and the new cache isolation module(s) have unit tests that fail when the protection is removed (spot-check by temporary revert in a scratch branch).
6. Serena memories under `compaction/` are updated with one memory per task summarizing implementation, evaluation outcome, and any caveats.
7. `docs/CONTEXT_PIPELINE_EVAL.md` is updated to reflect which techniques are now runtime-enforced vs eval-only.
8. Commits pushed to `dev` (or successor branch) with signed commits if the SSH signing key is restored; otherwise commits exist locally with `--no-gpg-sign` and the signing key blocker is recorded.

## First actions when you resume

1. `cd /Users/jrudnik/labs/jcode && git status && git log --oneline -5`
2. Read the four Serena memories listed above.
3. Run `backlog task 88 --plain` to confirm the prior state.
4. Create TASK-89 with the command above and set it In Progress assigned to yourself.
5. Add the implementation plan to TASK-89 via `backlog task edit <id> --plan ...`, share with the user, wait for approval before coding.
