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

For repeatable experiment tracking, create a manifest before or after a run and
register completed artifact directories in a JSONL registry:

```bash
python3 scripts/context_experiment.py create-manifest \
  --out target/context-eval/manifest.json \
  --title "combined-p0 realistic replay" \
  --scenario-kind realistic \
  --include-local-sessions \
  --artifacts-dir target/context-eval/<timestamp>

python3 scripts/context_experiment.py validate-manifest \
  target/context-eval/manifest.json

python3 scripts/context_experiment.py register-run \
  --manifest target/context-eval/manifest.json \
  --artifacts-dir target/context-eval/<timestamp>

python3 scripts/context_experiment.py list-runs
```

The manifest records git/environment metadata, scenario settings, expected or
inferred techniques, and artifact checksums. The run registry defaults to
`target/context-eval/run_registry.jsonl` and stores per-run artifact fingerprints
plus summary metrics such as the best practical score technique.

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

## Experiment battery and decision rubric

A second research pass focused on datasets, experimental design, and valuation
criteria. The recommended battery is staged so we can reject unsafe techniques
early before spending effort on expensive provider or benchmark runs.

### Dataset and benchmark shortlist

| Source | Best use | Integration effort | Caveats |
| --- | --- | --- | --- |
| First-party synthetic canaries | Exact tests for protected facts, stale/foreign facts, cross-project cache leakage, duplicate tools, restore handles, and deterministic ordering. | Low | Must avoid overfitting; keep fixtures small and adversarial. |
| Realistic local JCODE replay | JCODE-specific regressions from `~/.jcode/sessions` plus injected canaries. | Low to medium | Private/local data only; do not vendor logs. |
| Terminal-Bench style tasks | End-to-end terminal agent task completion under context variants. | Medium | Slower and environment-sensitive; useful after deterministic gates pass. |
| SWE-bench Lite/Verified style issue fixtures | Repository/task continuity, code-edit relevance, and stale context contamination. | Medium to high | More expensive and may require harness adaptation; licensing/version should be verified before vendoring. |
| RULER / needle-in-a-haystack style long-context tasks | Long-context retention, retrieval, and attention degradation. | Low to medium | Synthetic; good for controlled stress but not sufficient alone. |
| LongBench / NoLiMa-style long-context QA | Attention and retrieval behavior under distractors. | Medium | Less coding-agent-specific. |
| RepoBench-style code retrieval/completion | Codebase context selection and repository-aware retrieval. | Medium | May need adaptation to JCODE tool/prompt format. |
| ToolBench / WebArena / OSWorld / AgentBench-style suites | Multi-tool and agentic task behavior under context/cache changes. | High | Broad agent benchmarks can be noisy and costly; defer until core gates are stable. |

Public stale/foreign cache-leakage datasets are immature, so the battery should
start with first-party synthetic isolation canaries and JCODE replay fixtures,
then graduate only surviving candidates to public benchmarks.

### Recommended experiment battery

1. **Static transform regression**
   - Run synthetic fixtures through each prototype transform.
   - Assert exact protected retention, stale/foreign removal, restore-handle
     coverage, and stable output hashes.
2. **Cross-project cache canary**
   - Create two fake projects with overlapping filenames, symbols, and task IDs
     but conflicting protected facts.
   - Prime caches from project A, switch to project B, and assert zero foreign
     cache hits or answer contamination.
3. **Counterfactual stale-context replay**
   - Replay realistic sessions with injected stale plans, failed tool outputs,
     and superseded hypotheses.
   - Score stale retention, supersession correctness, and protected-goal
     retention.
4. **A/B provider-payload comparison**
   - Render canonical baseline payload and candidate payload from the same
     transcript.
   - Diff ordering, protected spans, omitted artifacts, cache fingerprints, and
     tool/schema/system prompt hashes before any model call.
5. **Question-answerability probes**
   - Attach deterministic questions to each fixture, with expected and forbidden
     answers, e.g. “What task are we working on?” and “Which repo does this fact
     belong to?”
   - Use exact/regex scoring first; add model-as-judge only as a later optional
     layer.
6. **Ablation matrix**
   - Evaluate each technique alone and in combinations: gatekeeping,
     quarantine, goal ledger, supersession, attention preamble, lazy restore,
     cache namespace, and cache provenance.
   - Keep combinations small enough to identify which component causes wins or
     regressions.
7. **Repeated deterministic seeds**
   - Run each fixture multiple times and assert stable output hashes and stable
     metrics. Any nondeterminism blocks runtime adoption.
8. **Graduation benchmarks**
   - For candidates passing deterministic gates, run a small Terminal-Bench or
     SWE-bench-style subset and compare task pass rate, contamination, latency,
     and token cost.

### Go/no-go valuation rubric

Score candidates from `0` to `5` per category, multiply by weight, then apply
the hard gates below. Token savings alone should never justify a candidate that
hurts reliability.

| Criterion | Weight | High score means |
| --- | ---: | --- |
| Correctness / reliability impact | 25% | Prevents real failures: protected facts survive, stale/foreign content and false cache hits disappear. |
| User-visible benefit | 15% | Fewer wrong answers, better continuity, lower latency, fewer repeated context mistakes. |
| Implementation effort | 12% | Small, localized, incremental implementation with limited migrations. |
| Runtime cost | 10% | Low transform/cache overhead and no visible UI/provider delay. |
| Maintenance risk | 10% | Clear schema/versioning story, few hidden couplings, low dependency risk. |
| Observability | 10% | Fingerprints, provenance, reason categories, logs, and replay artifacts make failures debuggable. |
| Regression risk | 10% | Low chance of dropping needed context, poisoning cache behavior, or destabilizing prompts. |
| Reversibility | 8% | Feature-flagged, fallbackable, and rollback-safe with cache invalidation escape hatches. |

Hard gates before scoring:

- Protected instruction/task retention `>= 0.98` and protected answerability
  `>= 0.95`.
- Cross-project leakage rate, false cache-hit rate, and provider/protected-fact
  stale-hit rate are all `0.0` for cache-related candidates.
- Typical transform latency `< 20ms`, p95 `< 100ms`, unless explicitly outside
  the hot path.
- Deterministic snapshot/replay/property test story exists.
- Sufficient observability exists to diagnose wrong inclusion/exclusion.
- No irreversible or migration-heavy design without a separate migration plan.

Decision thresholds after hard gates:

| Decision | Weighted score |
| --- | ---: |
| Go / implement now | `>= 4.0 / 5` |
| Cache isolation Go | `>= 4.2 / 5` |
| Prototype first | `3.25 - 3.99` |
| Defer | `2.5 - 3.24` |
| No-Go | `< 2.5` |

### Immediate evaluation build-out

The next harness extension should add:

- `cache_cross_project` synthetic fixture with two fake repos and conflicting
  task/file facts.
- Fixture-level `questions` with `expected` and `forbidden` answer patterns.
- Output-hash determinism checks across repeated runs.
- Per-candidate valuation rows alongside `matrix.csv`, including estimated
  implementation effort, runtime cost, observability, and reversibility.
- Optional benchmark adapters kept out of the hot path until deterministic
  gates pass.

## Repeated SCO/VM experiment matrix runner

`scripts/context_eval_matrix.py` wraps `scripts/context_pipeline_eval.py` to run
the same candidates repeatedly under an assumption matrix. Use it when deciding
whether a technique is stable enough to implement, not for quick smoke checks.

Local minimal matrix:

```bash
python3 scripts/context_eval_matrix.py \
  --mode local \
  --scenario-kind synthetic \
  --include-local-sessions false \
  --tool-budget-chars 4000 \
  --repetitions 2 \
  --out target/context-eval-matrix/local-smoke
```

Remote minimal matrix on `serious-callers-only`:

```bash
python3 scripts/context_eval_matrix.py \
  --mode remote \
  --host serious-callers-only \
  --remote-dir /tmp/jcode-context-eval-matrix \
  --scenario-kind realistic \
  --include-local-sessions true \
  --tool-budget-chars 4000 \
  --repetitions 2 \
  --out target/context-eval-matrix/sco-realistic-smoke
```

Remote VM/provision hook:

```bash
JCODE_CONTEXT_EVAL_VM_START_CMD='your-idempotent-vm-start-command' \
python3 scripts/context_eval_matrix.py \
  --mode remote \
  --host serious-callers-only \
  --remote-dir /tmp/jcode-context-eval-matrix-vm \
  --scenario-kind synthetic \
  --scenario-kind realistic \
  --include-local-sessions false \
  --include-local-sessions true \
  --tool-budget-chars 2000 \
  --tool-budget-chars 4000 \
  --tool-budget-chars 8000 \
  --repetitions 5
```

The matrix dimensions are:

- `--scenario-kind`: repeatable `synthetic` and/or `realistic`.
- `--include-local-sessions`: repeatable `true` and/or `false`.
- `--tool-budget-chars`: repeatable integer budgets.
- `--repetitions`: repeated runs for each assumption tuple.
- `--technique`: optional repeatable subset of techniques; defaults to all.

Output layout:

```text
target/context-eval-matrix/<run>/
  assumptions.json
  run_config.json
  runs/<assumption-id>/
    assumption.json
    matrix.csv
    matrix.json
    annotated_matrix.json
    runs/<technique>.context.json
  all_rows.csv
  all_rows.json
  summary.csv
  summary.json
```

`summary.csv` aggregates each technique/assumption tuple with
mean/min/max/stdev for numeric metrics. `passes_reliability_gates` currently
requires:

- minimum protected retention `>= 0.98`,
- minimum restore-handle coverage `>= 1.0`,
- maximum stale/foreign retention `<= 0.0`,
- maximum latency `< 100ms`.

Suggested data-gathering sequence:

1. Local synthetic smoke: one budget, two repetitions.
2. SCO synthetic matrix: budgets `2000`, `4000`, `8000`, three repetitions.
3. SCO realistic matrix with local sessions: budgets `2000`, `4000`, `8000`,
   five repetitions.
4. VM-backed run using `JCODE_CONTEXT_EVAL_VM_START_CMD` once an isolated VM
   start/reset command is available.
5. Only candidates passing deterministic gates graduate to Terminal-Bench or
   SWE-bench-style runs.

Interpretation guidance:

- High mean score is insufficient if min protected retention fails.
- High token savings are suspicious if stale/foreign retention is non-zero.
- Large stdev suggests nondeterminism or fixture sensitivity and should block
  runtime changes until explained.
- Cache-isolation candidates should use stricter gates than generic pruning:
  cross-project leakage and false hits must remain exactly zero.


## Experiment infrastructure contracts

TASK-84 treats the current scripts as an experiment support layer, not just
one-off prototypes. The contracts below are the minimum glue needed before
context/cache candidates graduate to broader testing.

### Manifests and registry

`scripts/context_experiment.py` records context-eval runs without making model
calls or mutating JCODE runtime state. Use it after a deterministic run to create
a manifest, validate it, and append the completed run to the JSONL registry.

```bash
python3 scripts/context_experiment.py create-manifest \
  --out target/context-eval/my-run/manifest.json \
  --title "Context cache isolation smoke" \
  --scenario-kind synthetic \
  --tool-budget-chars 4000 \
  --artifacts-dir target/context-eval/my-run

python3 scripts/context_experiment.py validate-manifest \
  target/context-eval/my-run/manifest.json

python3 scripts/context_experiment.py register-run \
  --manifest target/context-eval/my-run/manifest.json \
  --artifacts-dir target/context-eval/my-run \
  --registry target/context-eval/run_registry.jsonl \
  --status completed

python3 scripts/context_experiment.py list-runs \
  --registry target/context-eval/run_registry.jsonl
```

The helper fingerprints common artifacts from `matrix.json`, `matrix.csv`,
`scenarios/*.json`, `runs/*.context.json`, and optional `model_eval/*.json`. It
also captures git branch/commit/dirty state, Python/platform metadata, pipeline
settings, techniques, owner, and notes. Treat these fields as the minimum
registry key for comparing or re-running results:

| Field | Why it matters |
| --- | --- |
| `experiment_id`, manifest path, and manifest SHA-256 | Stable handle for reports and follow-up tasks. |
| Git commit, branch, and dirty-state flag | Prevents mixing results from different code. |
| Scenario kind, local-session inclusion, budgets, repetitions, techniques | Defines the assumption tuple. |
| Artifact paths, byte sizes, and SHA-256 hashes | Allows deterministic replay and fixture drift detection. |
| Python version, platform, and host | Explains reproducibility differences. |
| Registry status and notes | Records whether the run is planned, running, completed, or failed. |

The default registry is `target/context-eval/run_registry.jsonl`. Keep it
append-only for completed runs. If a run is superseded, register a new run and
record the relationship in `--note`; do not rewrite historical metrics.

### Reports and recommendations

`scripts/context_experiment_report.py` renders Markdown and/or HTML reports from
existing artifacts. Reports should be generated from deterministic artifacts
first, with model-eval data included only after deterministic gates pass.

```bash
python3 scripts/context_experiment_report.py \
  --artifacts target/context-eval/my-run \
  --format both \
  --out target/context-eval/my-run/report
```

The report consumes `matrix.json` by default and auto-detects
`model_eval/results.json` when present. It summarizes deterministic/model
results, gate failures, and a recommendation. Gate thresholds can be tightened or
relaxed with `--min-protected-retention`, `--max-stale-retention`,
`--min-restore-coverage`, and `--min-model-pass-rate`.

A report is ready for review when it includes:

- manifest/registry identity and git commit,
- the exact command or command template used,
- deterministic gate outcomes and top failures,
- model-eval results only if deterministic gates passed first,
- a recommendation per candidate or candidate set,
- redaction-scan result and any intentionally withheld artifact paths.

The report renderer should remain a thin view over `matrix.json` and optional
`model_eval/results.json`; it should not recompute metrics differently from the
underlying scripts.

### Fixtures and determinism

Fixture tiers should stay explicit and hashable:

- **Synthetic fixtures**: deterministic canaries for protected terms, stale or
  foreign distractors, duplicate tools, large outputs, and restore handles.
- **Cache cross-project fixtures**: two or more fake repos with overlapping file
  names, symbols, task IDs, and conflicting protected facts. These are the
  cheapest way to catch foreign cache hits before runtime integration.
- **Realistic replay fixtures**: opt-in local session samples plus controlled
  injected stale/foreign blocks. Do not vendor private logs.
- **Public benchmark fixtures**: only after deterministic gates pass and license
  or dataset versioning is documented in the manifest.

Determinism checks should compare repeated output hashes for scenario files,
`*.context.json` transformed artifacts, `matrix.json`, and `summary.json`. Any
output that includes timestamps, absolute temp paths, or host-specific paths
should either normalize those fields before hashing or list them under an
`ignored_fields` section in the manifest. A candidate with unexplained hash drift
should not move into runtime code even if the mean score is high.

### Redaction scanning before sharing artifacts

Artifact directories can contain local-session snippets, model prompts, and model
responses, so scan before publishing, committing, or pasting reports. Use the
TASK-84 scanner for context-eval artifacts and keep the broader repository
preflight for normal source-tree checks:

```bash
python3 scripts/context_artifact_secret_scan.py \
  target/context-eval/my-run \
  target/context-eval/my-run/report \
  --out target/context-eval/my-run/secret_scan

./scripts/security_preflight.sh
```

The scanner writes `findings.json` plus `summary.txt`, redacts matched values in
its own output, and exits non-zero when high-severity findings remain after
allowlist and minimum-length filtering. Use `--allowlist <json-list>` only for
documented synthetic canaries or known benign hashes, and prefer allowlisting
SHA-256 or SHA-256-16 hashes over literal values.

Treat a scanner hit as blocking unless it is a documented synthetic canary such
as `PAYMENT_SECRET_DO_NOT_USE=example-redacted-value`. Reports should record the
scanner command and whether any files were omitted, redacted, or kept because
they matched an allowlisted synthetic fixture.

### Low-risk infra gap

The most useful small follow-up is wiring `context_experiment_report.py` and
`context_artifact_secret_scan.py` together so report generation can optionally
scan the exact files it links and stamp the scan summary into `report.md`. This
keeps sharing checks reproducible without changing runtime context code.

## Opt-in real-model evaluation

The deterministic harness is the first gate. To test whether transformed
payloads actually steer a model correctly, use `scripts/context_model_eval.py`
against existing `*.context.json` artifacts. This phase is intentionally opt-in
because it spends provider tokens and introduces provider variance.

OpenAI-compatible smoke:

```bash
OPENAI_API_KEY=... \
python3 scripts/context_model_eval.py \
  --artifacts target/context-eval-matrix/sco-smoke \
  --provider openai \
  --model gpt-4o-mini \
  --technique baseline \
  --technique combined_p0 \
  --max-contexts 2 \
  --max-calls 4 \
  --out target/context-eval-model/sco-smoke-openai
```

JCODE subscription-backed smoke using `jcode run`:

```bash
python3 scripts/context_model_eval.py \
  --artifacts target/context-eval-matrix/sco-smoke \
  --provider jcode-run \
  --jcode-provider openai \
  --model gpt-5.5 \
  --technique baseline \
  --technique combined_p0 \
  --max-contexts 2 \
  --max-calls 4 \
  --out target/context-eval-model/sco-smoke-jcode-run
```

`jcode-run` is the easiest path for subscription-backed tokens because it uses
the existing JCODE CLI auth flow instead of requiring raw API keys.

Anthropic smoke:

```bash
ANTHROPIC_API_KEY=... \
python3 scripts/context_model_eval.py \
  --artifacts target/context-eval-matrix/sco-smoke \
  --provider anthropic \
  --api-key-env ANTHROPIC_API_KEY \
  --base-url https://api.anthropic.com/v1 \
  --model claude-3-5-haiku-latest \
  --max-contexts 2 \
  --max-calls 4
```

OpenRouter or another OpenAI-compatible endpoint:

```bash
OPENROUTER_API_KEY=... \
python3 scripts/context_model_eval.py \
  --artifacts target/context-eval-matrix/sco-smoke \
  --provider openrouter \
  --api-key-env OPENROUTER_API_KEY \
  --base-url https://openrouter.ai/api/v1 \
  --model openai/gpt-4o-mini
```

Environment-variable defaults:

| Variable | Purpose |
| --- | --- |
| `JCODE_CONTEXT_MODEL_EVAL_PROVIDER` | `openai`, `openrouter`, `openai-compatible`, or `anthropic`. |
| `JCODE_CONTEXT_MODEL_EVAL_MODEL` | Model name. |
| `JCODE_CONTEXT_MODEL_EVAL_BASE_URL` | API base URL. |
| `JCODE_CONTEXT_MODEL_EVAL_API_KEY_ENV` | Name of env var containing the API key. |
| `JCODE_CONTEXT_MODEL_EVAL_JCODE_BIN` | `jcode` binary for `--provider jcode-run`. |
| `JCODE_CONTEXT_MODEL_EVAL_JCODE_PROVIDER` | JCODE provider passed to `jcode run -p`. |

Safety and cost controls:

- API keys are read from environment variables and are not written to outputs.
- `--max-calls` caps total provider calls.
- `--max-contexts` caps context artifacts read.
- `--max-context-chars` truncates oversized contexts before the provider call.
- `--max-output-tokens` defaults to a small response budget.
- Outputs include prompts only indirectly through context file references;
  responses are stored for audit in `results.csv/json` and `responses/*.json`.

The default questions check active-task answerability, stale/foreign
contamination, and restore-handle awareness. For more precise benchmark runs,
pass a custom JSON question list with fields:

```json
[
  {
    "id": "task_identity",
    "prompt": "What task are we working on?",
    "expected_any": ["TASK-83"],
    "forbidden_any": ["nix-config", "wrong branch deploy"]
  }
]
```

Interpret model-eval results only after deterministic gates pass. A candidate
that fails protected retention or cache isolation should not be rescued by a
single favorable model response.
