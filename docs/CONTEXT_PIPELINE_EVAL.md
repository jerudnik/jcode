# Context Pipeline Evaluation Harness

This document describes the deterministic experiment harness introduced for TASK-79. It is intended to answer a practical question before we implement context-management features in the runtime agent:

> Which candidate techniques are cheap enough, safe enough, and effective enough to deserve native JCODE implementation?

The harness is deliberately modest. It does **not** claim publication-grade evaluation. It creates a repeatable baseline using synthetic scenarios plus optional local session/log samples, applies rough prototype transforms, and writes an evaluation matrix.

## Candidate techniques covered

`./scripts/context_pipeline_eval.py` currently prototypes:

- `baseline`: no transform.
- `stable_tiering`: wraps context blocks in status/trust-tagged XML-like blocks.
- `boundary_gate`: replaces oversized, binary, or minified inputs with placeholders.
- `tool_budget`: head/tail budgets large tool outputs and records restore handles.
- `duplicate_prune`: replaces older duplicate tool output with placeholders while keeping the latest.
- `trust_quarantine`: quarantines failed, speculative, or unverified context.
- `rust_skeleton`: rough Rust source skeletonization for oversized non-target files.
- `combined_p0`: combines the low-risk/high-ROI P0 candidates.

These are intentionally rough prototypes. They measure likely impact before runtime integration.

## Local quick start

From the repository root:

```bash
python3 scripts/context_pipeline_eval.py run-local
```

To include local JCODE log snippets from `~/.jcode/logs`:

```bash
python3 scripts/context_pipeline_eval.py run-local --include-local-sessions
```

To use the higher-fidelity replay fixture, which samples recent
`~/.jcode/sessions/*.json` messages, preserves early intent plus latest state,
and injects controlled stale/foreign distractors:

```bash
python3 scripts/context_pipeline_eval.py run-local \
  --scenario-kind realistic \
  --include-local-sessions
```

Outputs are written under:

```text
target/context-eval/<timestamp>/
  scenarios/context_pipeline_baseline.json
  runs/<technique>.context.json
  matrix.json
  matrix.csv
```

The terminal table includes:

- estimated tokens saved,
- noise-reduction ratio,
- protected-term retention ratio,
- stale/foreign distractor retention ratio,
- restore-handle coverage ratio,
- transform latency,
- a simple heuristic practical score.

## Remote run on `serious-callers-only`

The remote path uses non-interactive SSH and `rsync`. It avoids assuming a specific VM provider by accepting a host-side VM start command.

Basic remote run:

```bash
python3 scripts/context_pipeline_eval.py run-remote \
  --host serious-callers-only
```

Higher-fidelity remote replay run:

```bash
python3 scripts/context_pipeline_eval.py run-remote \
  --host serious-callers-only \
  --remote-dir /tmp/jcode-context-eval-realistic \
  --out target/context-eval/realistic-remote \
  --scenario-kind realistic \
  --include-local-sessions
```

With a host-side VM/provision command:

```bash
JCODE_CONTEXT_EVAL_VM_START_CMD='your-idempotent-vm-start-command-here' \
python3 scripts/context_pipeline_eval.py run-remote \
  --host serious-callers-only \
  --remote-dir /tmp/jcode-context-eval
```

Environment variables:

| Variable | Default | Purpose |
| --- | --- | --- |
| `JCODE_CONTEXT_EVAL_HOST` | `serious-callers-only` | SSH host. |
| `JCODE_CONTEXT_EVAL_REMOTE_DIR` | `/tmp/jcode-context-eval` | Remote staging directory. |
| `JCODE_CONTEXT_EVAL_VM_START_CMD` | empty | Optional idempotent command run on the SSH host before syncing/running. |

Safety boundaries:

- SSH uses `BatchMode=yes`, so the pipeline fails instead of prompting.
- Repository sync excludes `.git/` and `target/`.
- Remote staging defaults to `/tmp/jcode-context-eval`.
- The script does not mutate runtime JCODE state.
- Local session inclusion is opt-in with `--include-local-sessions`.

## How this maps to a VM-backed jcode experiment

The current script is the deterministic core. A full VM-backed experiment can wrap it in the following phases:

1. Start or reset an isolated VM on `serious-callers-only` using `JCODE_CONTEXT_EVAL_VM_START_CMD`.
2. Sync this checkout into the VM or into a host-side isolated worktree.
3. Build or reuse `jcode` inside the isolated environment.
4. Generate scenarios from synthetic fixtures, local session history, and public datasets.
5. Run prototype context transforms over the scenarios.
6. Optionally replay selected scenarios through a test jcode instance once runtime hooks exist.
7. Fetch `matrix.csv`, `matrix.json`, and transformed context artifacts.

Until runtime hooks exist, the harness evaluates the context payload transformations directly.

## Interpreting the matrix

Use the matrix as a triage tool, not a final proof.

Good candidates should show:

- high protected-term retention,
- meaningful token/noise reduction,
- very low transform latency,
- clear restore handles or placeholders for omitted content,
- low or zero retention of controlled stale/foreign distractors,
- simple implementation path and low persistent-state burden.

A technique should be deferred if it:

- saves tokens by dropping protected facts,
- requires heavy dependencies or model calls in the hot path,
- needs ambiguous persistent state migrations,
- produces unstable output ordering,
- cannot be snapshot or property tested.

## Suggested decision thresholds

Initial pragmatic thresholds:

| Criterion | P0 target |
| --- | --- |
| Protected-term retention | `>= 0.95` |
| Typical transform latency | `< 20ms` per provider context render fixture |
| Boundary check latency | `< 5ms` per file/tool output in microbenchmarks |
| Noise reduction on noisy scenarios | `>= 0.30` |
| Restore handle coverage for placeholders | `100%` |
| Controlled stale/foreign retention | `0.0` when the scenario includes distractors |
| Snapshot determinism | stable across repeated runs |

## Higher-fidelity replay smoke result

The first realistic local and remote smoke runs sampled recent JCODE session
snapshots and injected controlled stale/foreign context. The top-level result
held, but the ranking became more informative:

- `combined_p0` remained the top candidate.
- `trust_quarantine` moved close to `combined_p0` because it removed the
  controlled stale/foreign terms while preserving protected terms.
- `boundary_gate` reduced noise but still retained some stale/foreign terms in
  smaller distractor blocks.
- `tool_budget` saved tokens but preserved all controlled stale/foreign terms,
  so it should not ship alone as a reliability feature.

Representative remote run on `serious-callers-only`:

| Technique | Saved tokens est. | Noise reduction | Protected retention | Stale/foreign retention | Score |
| --- | ---: | ---: | ---: | ---: | ---: |
| `combined_p0` | 35087 | 0.1984 | 1.0 | 0.0 | 79.96 |
| `trust_quarantine` | 23823 | 0.1347 | 1.0 | 0.0 | 78.37 |
| `boundary_gate` | 24985 | 0.1413 | 1.0 | 0.5 | 71.03 |
| `tool_budget` | 23236 | 0.1314 | 1.0 | 1.0 | 58.28 |

## Next improvements

- Extend the prototype matrix with the pending candidates below. These are
  tracked from TASK-80 but consolidated here so the implemented results and
  pending experiments stay in one evaluation ledger.
- Add real transcript parsers once stable session schemas are confirmed.
- Add public SWE-bench/Terminal-Bench style fixture ingestion.
- Add runtime jcode provider-payload capture mode for true replay experiments.
- Add implementation-burden fields to `matrix.csv` once prototype-to-runtime diffs are estimated.
- Add a small HTML/Markdown report generator for reviewer-friendly summaries.

## Pending prototype/evaluation ledger

| Candidate | Reliability goal | Suggested metric | Initial priority |
| --- | --- | --- | --- |
| Goal/task retention ledger | Preserve active objective, constraints, acceptance criteria, and `do not` rules under compaction/pruning. | Protected goal/constraint retention after aggressive budgets and realistic replay. | P0 |
| Contradiction/supersession pruning | Remove stale hypotheses or older instructions invalidated by later evidence. | Disproven-claim retention, superseded-block removal, false-positive pruning rate. | P0 |
| Attention preamble/context index | Make important facts salient before long context payloads. | Protected fact answerability, ordering stability, preamble token overhead. | P0 |
| Lazy restore handles with targeted expansion | Omit large artifacts safely and restore only when the current turn references them. | Restore precision/recall, handle coverage, token savings, latency. | P0 |
| Pinned facts/protected spans | Make user instructions, task IDs, file paths, decisions, and safety constraints non-prunable unless superseded. | Pinned-span survival and accidental stale-pin retention. | P1 |
| Recency plus importance scoring | Rank blocks by role, recency, explicit constraints, task references, and tool success/failure. | Useful-context retention at fixed budgets, stale/foreign retention. | P1 |
| Context provenance/trust routing | Keep verified user/task/tool-success facts distinct from failed tools, logs, and speculative assistant text. | Foreign/stale retention and hallucination-trigger reduction. | P1 |
| Working-state scratchpad | Preserve compact task continuity without replaying stale reasoning-like context. | Continuity after compaction, scratchpad drift, protected-fact consistency. | P2 |
| Session-local fact memory | Store short-lived facts bounded to one session/task before considering global memory. | Source-bound recall precision, leakage across sessions/tasks. | P2 |
| TTL/expiry for inferred facts | Prevent stale inferred facts from living indefinitely. | Expired-fact retention and still-valid fact loss. | P2 |
| Source-bound memories | Scope memories to repo/session/task provenance. | Cross-repo leakage rate and relevant memory recall. | P2 |
| Conflict-aware memory retrieval | Return memories with conflict labels instead of blindly injecting them. | Conflict detection precision/recall and answer contamination rate. | P2 |

Recommended next prototype batch: goal/task retention ledger,
contradiction/supersession pruning, attention preamble/context index, and lazy
restore handles. These are low-risk enough to simulate in the current harness
and directly measurable with the realistic replay fixture.

## Research swarm findings for TASK-79 through TASK-81

A focused research pass over TASK-79, TASK-80, and TASK-81 refined the next
implementation/evaluation targets.

### Context-management patterns to prototype next

- **Goal/task retention ledger**: generate a canonical block from user/backlog
  and successful tool facts containing current objective, task IDs, acceptance
  criteria, explicit constraints, `do not` rules, current plan state, and
  validation obligations. Do not derive it from speculative assistant text.
- **Supersession pruning**: extend existing runtime pruning in
  `src/agent/context_pruning.rs` from duplicate/failed tool-result cases into
  evidence chains: older failed read/edit/build results superseded by later
  success, hypotheses invalidated by later tool output, and older plan/task
  state superseded by newer backlog status.
- **Attention preamble/context index**: prepend a deterministic, fixed-budget
  index with current objective, critical constraints, relevant files/tasks,
  omitted artifact restore IDs, and known stale/quarantined content.
- **Lazy restore handles**: strengthen placeholders with restore ID, kind,
  path/tool, content hash, byte/token count, trust tier, and supersession
  status. Expand only when the current turn references a matching file path,
  tool ID, symbol, restore ID, or quoted omitted phrase.
- **Pinned facts/protected spans**: pin exact user instructions, task IDs, file
  paths, decisions, safety constraints, and explicit `never`/`do not`/`must`
  statements, but keep pins source-bound and supersedable to avoid stale pins.

### Evaluation methodology refinements

Use three fixture tiers so results do not overfit synthetic canaries:

| Tier | Purpose | Fixture shape | Scoring |
| --- | --- | --- | --- |
| Synthetic canaries | Deterministic unit/regression checks | Hand-built blocks with protected terms, stale terms, cross-project distractors, large logs, duplicate tools, and cache entries. | Exact-match and structural metrics. |
| Realistic local replay | JCODE-specific reliability | Recent session snapshots, early user intent, latest state, controlled stale/foreign injections. | Retention, stale-hit rate, answerability. |
| Public benchmark replay | External validity | Terminal-Bench style terminal tasks, SWE-bench Lite/Verified style issue fixtures, and RULER-style long-context needles. | Task pass rate, exactness, contamination checks. |

Additional target thresholds from the research pass:

| Metric | Suggested threshold |
| --- | --- |
| Protected instruction/task retention | `>= 0.98` |
| Protected answerability | `>= 0.95` |
| Synthetic stale/foreign retention | `0.0` |
| Realistic stale retention | `< 0.02` |
| Realistic foreign retention | `< 0.01` |
| Stale answer contamination | `0.0` for P0 candidates |
| Restore precision | `>= 0.90` |
| Restore recall | `>= 0.98` |
| Ordering stability | `100%` deterministic output hash across repeated runs |
| Transform latency | `< 20ms` typical, `< 100ms` p95 per fixture |

### Cache isolation and cross-project confusion controls

Shared caches can cause reliability failures even when cache hit rate is high.
The research pass categorized cache risks and required isolation fields:

| Cache type | Main risk | Required isolation fields |
| --- | --- | --- |
| Repo maps / code indexes | Symbols or files from another checkout appear relevant. | Canonical repo ID, worktree root hash, VCS remote, commit/tree hash, branch, index schema. |
| Skeletons / summaries | Stale or foreign file summaries survive edits. | File content hash, file path, parser version, summary prompt/version, repo namespace. |
| Token estimates | Wrong budgeting from provider/model/tokenizer mismatch. | Provider, model, tokenizer version, message format version. |
| Embeddings / retrieval | Similar but unrelated project facts retrieved. | Repo namespace, source URI, content hash, embedding model/version, chunker version, trust tier. |
| Prompt projections / compaction outputs | Old instructions or other project task IDs reused. | Session ID, task ID, context recipe version, source message hashes, compaction version. |
| Provider payload cache | Append-only prefix breaks or wrong cached prefix assumed. | Provider, model, system prompt hash, tools schema hash, message prefix hash, cache policy. |
| Tool/result caches | Tool output from wrong cwd/env reused. | Cwd/repo ID, command/tool name, args hash, env allowlist hash, input file hashes, TTL. |
| UI/render caches | Visual state from wrong project/session shown. | Session ID, surface ID, content hash, schema/theme/version. |
| External API caches | GitHub/issue/model data stale or wrong account/repo. | Account, repo, endpoint, query params, auth identity fingerprint, synced_at, TTL. |

Recommended structured cache-key shape:

```text
cache_key = hash(
  cache_kind,
  schema_version,
  jcode_version_or_cache_format_version,
  project_namespace,
  source_identity,
  content_identity,
  transform_identity,
  environment_identity
)
```

Prefer a two-level design:

1. **Global content-addressed blob store** only for outputs that are pure
   functions of content plus transform version.
2. **Project/session namespace manifests** for lookup eligibility, provenance,
   TTL, invalidation, and trust tier. Cross-project reads should be rejected by
   default unless the entry is explicitly `global-safe`.

Cache evaluation metrics:

| Metric | Suggested threshold |
| --- | --- |
| Cross-project leakage rate | `0.0` |
| False cache-hit rate | `0.0` |
| Stale-hit rate | `< 0.5%` generally, `0.0` for protected facts/provider payloads |
| Hit quality versus recompute | `>= 0.99` protected answerability parity |
| Invalidation recall | `>= 0.99` |
| Miss penalty from stricter isolation | `< 10%` p50, `< 20%` p95 |
| Cache size growth | `< 1.5x` unless hit quality improves materially |

### JCODE integration points identified

- `src/agent/context_pruning.rs`: extend current pruning rules into
  supersession and protected-span tests.
- `src/agent/compaction.rs` and `crates/jcode-compaction-core/src/lib.rs`:
  add context recipe/version/provenance fields and avoid persistent use of
  non-provenance-bearing semantic keys.
- `src/session.rs` and `src/prompt.rs`: carry project/session/task identity into
  context projection and prompt assembly.
- `src/cache_tracker.rs`: extend provider prompt-cache observability with
  project/session/provider/model/system/tool-schema fingerprints and reason
  categories for intentional rewrites.
- Provider payload renderers in `crates/jcode-provider-core/src/lib.rs`,
  `crates/jcode-provider-openai/src/request.rs`, `src/provider/anthropic.rs`,
  `src/provider/openai.rs`, and `src/provider/openrouter_provider_impl.rs`:
  log deterministic projection fingerprints around provider payload construction.
- `src/tool/agentgrep/context.rs`: audit repo/search context projection and
  ensure repo identity is part of cache/retrieval provenance.
- UI cache references such as `crates/jcode-tui-messages/src/cache.rs`,
  `src/tui/ui_messages_cache.rs`, and `crates/jcode-tui-mermaid/src/mermaid_cache_*`
  are lower-risk but should still be scoped by session/surface/content hash.

Easy first patches suggested by the swarm:

1. Add projection fingerprint logging around provider payload construction.
2. Assert working-directory/project identity in session context and cache
   signatures.
3. Add cache-tracker reason categories for known intentional prefix rewrites.
4. Add synthetic cross-project cache canary fixtures to the eval harness.
5. Add tests for cross-project prompt/static cache boundaries.
