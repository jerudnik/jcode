# Phase 1 focused unclassified-gap review

- Repository: `/Users/jrudnik/labs/jcode`, read-only. No repo files, refs, branches, worktrees, or stashes changed.
- Role: swarm `verify`. Adversarial, evidence-first. I read the two completed artifacts (`/tmp/jcode-recovery-mapper.md`, `/tmp/jcode-recovery-map-critic.md`) and then independently inspected source, not file buckets.
- Fixed refs (confirmed): fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`, upstream `802f6909825809e882d9c2d575b7e478dce57d3b`, merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` (verified `git merge-base fork upstream == 631935dd1`).
- Budget: focused, ~8 evidence checkpoints. No builds, tests, or external services.
- Confidence: **medium-high** that two genuinely new behavioral responsibilities are missing from both maps; **high** on the two-sided divergence measurements; **medium** on final seam-count implications (research, not authority).

## Bottom line for the coordinator

The seed classifier's unclassified set hides **two genuinely new behavioral responsibilities that neither map fully owns**, plus **one hidden cross-seam invariant** that both maps mis-located:

1. **Agent turn execution and durable evidence emission** (turn loop that produces the provider request/response and writes the `SessionLogEventKind::ProviderRequest/ProviderResponse` evidence + `herdr` liveness status). This is where the pilot's "observable request/result" is actually produced. Neither map owns it; the Mapper implicitly folded "emitted wire metadata" into R02, but the emission machinery lives in the agent turn loop.
2. **Compaction policy and its provider-session side effect** (token-budget estimation, threshold triggering, emergency truncation, image token costing). Strongly fork-only. Critically, the compaction completion path performs `provider_session_id = None` (`agent/compaction.rs`), which is the same "reset incompatible provider session state" invariant R02 claims to own but does not implement.
3. **Subscription-tier model gating** is two-sided and **upstream-heavy** (`subscription_catalog.rs` upstream 87+/19- vs fork 2+/1-). This is a real fork/upstream reconcile point that R02 must explicitly own; it is currently unnamed.

None of these should *displace* a top-six seam, but the six should be adjusted (see Q3). The other unclassified clusters (usage accounting, sponsored discovery) are largely absorbed by existing seams.

## Evidence

### Cluster inventory (reproducible)

```bash
base=631935dd1d3b2e31e167e2b12ad463e54bcf4b8d
fork=7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4
up=802f6909825809e882d9c2d575b7e478dce57d3b
git diff --name-only $base $fork | wc -l   # 943 fork-changed paths
git diff --name-only $base $up   | wc -l   # 425 upstream-changed paths
```

Per-file two-sided divergence (numstat, fork vs upstream):

| Path | fork | upstream | Read |
|---|---|---|---|
| `crates/jcode-app-core/src/agent.rs` | 147+/22- | 29+/3- | **strongly two-sided**, fork-dominant |
| `crates/jcode-app-core/src/agent/turn_loops.rs` | 88+/0- | 5+/0- | fork-dominant |
| `crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs` | 126+/6- | 8+/6- | fork-dominant |
| `crates/jcode-app-core/src/agent/turn_execution.rs` | 54+/9- | 23+/5- | two-sided |
| `crates/jcode-app-core/src/agent/compaction.rs` | present | **absent upstream** | fork-only |
| `crates/jcode-app-core/src/agent/prompting.rs` | 10+/0- | **absent** | fork-only |
| `crates/jcode-app-core/src/agent/evidence.rs` | present | **absent** | fork-only |
| `crates/jcode-base/src/compaction.rs` | 119+/31- | **none** | fork-only |
| `crates/jcode-compaction-core/src/lib.rs` | 22+/0- | **none** | fork-only |
| `crates/jcode-base/src/subscription_catalog.rs` | 2+/1- | **87+/19-** | **upstream-heavy** |
| `crates/jcode-base/src/subscription_api.rs` | 2+/2- | 3+/3- | small two-sided |
| `crates/jcode-base/src/sponsors.rs` | 54+/41- | 63+/50- | two-sided |
| `crates/jcode-usage-types/src/lib.rs` | 41+/0- | 41+/0- | two-sided (likely same commit) |

`agent.rs` declares these as first-class submodules (`agent.rs:3,5,9,15,16,17`): `mod compaction; mod evidence; mod prompting; mod turn_execution; mod turn_loops; mod turn_streaming_mpsc;`. This is a cohesive fork-authored subsystem, not scattered edits.

### 1. Agent turn execution + evidence emission (NEW responsibility)

`agent/turn_loops.rs` fork additions wrap every provider call in durable evidence with correlation IDs:

```
+ self.append_session_evidence_with_correlation(
+   SessionLogEventKind::ProviderRequest { provider, model, route: self.session.route_api_method, message_count, tool_count, prompt: ... }, provider_correlation.clone());
... on error -> SessionLogEventKind::ProviderResponse { status: Error, duration_ms, error_class }
... on ok    -> SessionLogEventKind::ProviderResponse { status: Ok, duration_ms, output, usage: TokenUsageSummary { input_tokens, output_tokens, total_tokens } }
```

`agent/turn_execution.rs` fork additions bracket each turn with evidence + liveness status:
`start_evidence_turn(...)` / `report_herdr_session("working","thinking")` / `finish_evidence_turn(...)` / `report_herdr_session("idle","idle")`.

`agent.rs` fork additions add the `herdr` liveness/status reporting surface: `report_herdr_session`, `report_herdr_tool`, `herdr_tool_status`, `herdr_channel_guardrail_status`, `herdr_session_path`, `herdr_custom_status`, `mark_active_with_client_pid`.

`agent/evidence.rs` (fork-only) owns `start_evidence_turn` / `finish_evidence_turn` / `append_session_evidence_with_correlation` producing `TurnStarted`, `ProviderRequest`, `ProviderResponse` records with `CorrelationIds { turn_id, ... }`.

**Interpretation:** the fork adds a turn-execution layer whose behavioral responsibility is "run a provider turn and emit correlated, durable evidence of request/response/usage plus external liveness status." This is exactly the substrate that produces the pilot's observable `request/result`. The Mapper's R02 invariant "emitted wire metadata" and R06A's "evidence carries session/parent/child identity" both *depend on* this layer but neither owns it.

### 2. Compaction policy + provider-session side effect (NEW responsibility)

`compaction-core` is a fork-only crate with the whole policy surface: `DEFAULT_TOKEN_BUDGET=200_000`, `COMPACTION_THRESHOLD=0.80`, `CRITICAL_THRESHOLD=0.95`, `MANUAL_COMPACT_MIN_THRESHOLD`, `RECENT_TURNS_TO_KEEP`, `EMERGENCY_TOOL_RESULT_MAX_CHARS`, `EMERGENCY_IMAGE_MAX_CHARS`, `PAYLOAD_IMAGE_CHAR_BUDGET`, `IMAGE_TOKEN_COST=1_600` (with a documented incident: raw base64 length ballooned the estimate and caused "triple" back-to-back compactions), `is_request_payload_too_large_error` (413 recovery). `base/compaction.rs` (fork-only, 119+/31-) implements reactive/proactive/semantic modes and hard-threshold synchronous compaction.

Critical cross-seam evidence in `agent/compaction.rs::note_compaction_applied`:

```
self.cache_tracker.reset();
self.locked_tools = None;
self.provider_session_id = None;
self.session.provider_session_id = None;
```

**This is the actual implementation of R02's claimed invariant** "model switch resets incompatible provider session state." R02 does not own it; the compaction completion path does. So there are at least two writers of provider-session identity (model switch in R02's surface and compaction completion here), which is a hidden invariant the Mapper's R02 ledger does not enumerate. This mirrors the critic's finding-9 pattern (multiple sources of truth for one identity) but in a different subsystem the critic did not examine.

### 3. Subscription-tier model gating (upstream-heavy, must be named in R02)

`subscription_catalog.rs` is upstream-dominant (87+/19- upstream vs 2+/1- fork). Upstream added the gating ladder and its ordering invariant:

```
JcodeTier::ALL = [Plus, Pro, Max, Ultra, Flagship]
min_tier: JcodeTier::Plus  // per-model floor
fn tier_gating_follows_catalog_order() { ... required_index <= account_index ... }
```

`subscription_api.rs` docstring: `GET /v1/me` is source of truth for tier/usage; last-known tier is cached so "model gating works offline (unknown/absent tier behaves like Plus)."

**Interpretation:** which models an account may select is gated by subscription tier, resolved from a network account endpoint with an offline cache fallback. This is squarely a *provider/model selection* authority (R02's domain) but is currently unnamed and is the strongest **upstream-side** divergence in this whole cluster, so it is a real reconcile point, not a fork-only add. R02's "owns" must explicitly include tier-gated model admission and the offline-cache fallback rule.

### 4. Absorbed clusters (not new seams)

- **Usage accounting** (`jcode-usage-types`, `usage/*.rs`, `usage_openai.rs`, `info_widget_usage.rs`, `tui-usage-overlay`): `ProviderUsage`, `UsageLimit`, `CopilotUsageTracker`, most-recently-used ordering. Two-sided but small and cohesive; it is provider/account usage telemetry -> belongs with R02 (credential/account state) for the data model and R08B for display. Not an independent authority.
- **Sponsored discovery** (`sponsors.rs`, `sponsors/provenance.rs`, `sponsor_disclosure.rs`, `SPONSORED_DISCOVERY_*` docs): two-sided. Owns discovery-tool admission, `(sponsored discovery)` disclosure, opt-out (`[sponsors] enabled=false`), and "requests carry only discovery fields, never session content." This is discovery + consent + provenance -> **absorbed by R07B** (discovery/telemetry/consent policy). It reinforces R07B's invariants; it is not a seventh seam.
- **Prompting text assets** (`prompt/system_prompt.md`, `swarm_prompt.md`, `selfdev_hint.txt`, `todo_confidence_*.txt`, `prompt.rs` two-sided 102+/53- fork vs 82+/53- upstream): system-prompt composition. Two-sided and non-trivial. Belongs with the new agent-execution responsibility (prompt assembly) but the *text* is low runtime-invariant risk; the `SplitSystemPrompt`/token-accounting logic (`agent/prompting.rs`) is the behavioral part.

## Answers to the four questions

**Q1 - Do these paths reveal missing behavioral responsibilities?**
Yes, two. (a) Agent turn execution + durable evidence/liveness emission, and (b) compaction policy with its provider-session reset side effect. Both are cohesive, fork-authored, invariant-bearing subsystems (`agent.rs` declares them as modules) and neither map owns them. Subscription-tier gating is a third responsibility that is real but belongs inside R02 once R02's "owns" is expanded.

**Q2 - owns / excludes / invariants / dependencies / pilot relevance**

New seam **RA - Agent turn execution and durable evidence emission**
- Owns: the turn loop that builds the provider request, invokes the provider, streams/collects the response, and emits `TurnStarted`/`ProviderRequest`/`ProviderResponse` evidence with `CorrelationIds`, token-usage summaries, and `herdr` liveness/tool status; prompt-prefix token accounting (`agent/prompting.rs`).
- Excludes: provider route/credential selection (R02), wire handshake (R03A), persistence *format* of evidence (R06A owns the schema/replay), and TUI rendering (R08B).
- Invariants: every provider call emits exactly one request record and one terminal response record with correct status/duration/usage/error_class; correlation IDs link request->response->turn; liveness transitions working<->idle bracket each turn; usage totals are input+output when both present; failure emits Error evidence, not a silent gap.
- Dependencies: R02 (provider identity/model), R06A (evidence schema + durable write), R01B/R04 (turn is interruptible by reload/cancel), and the compaction seam below (compaction runs inside the loop).
- Pilot relevance: **mandatory for the proposed provider pilot.** The pilot's "observable request/result" is literally the `ProviderRequest`/`ProviderResponse` evidence this seam emits. Comparing fork vs upstream request/result without owning the emission layer would attribute emission differences to R02 incorrectly.

New seam **RB - Compaction policy and provider-session side effect**
- Owns: token-budget estimation (incl. image token costing), threshold triggering (0.80/0.95), reactive/proactive/semantic mode selection, emergency truncation and 413 payload recovery, and the post-compaction reset of `provider_session_id`/`locked_tools`/`cache_tracker`.
- Excludes: raw persistence (R06A), provider routing (R02), memory recall (R06B).
- Invariants: compaction never raises the estimate it is trying to lower (the documented image-cost incident); recent N turns are kept verbatim; hard-threshold path is synchronous so the next API call fits; **compaction completion invalidates provider session identity exactly once and consistently in both `agent` and `session` copies**; manual compaction refused when provider does not support it.
- Dependencies: RA (runs inside the loop), R02 (shares ownership of `provider_session_id` truth), R06A (persists on completion via `persist_session_best_effort`).
- Pilot relevance: **light/conditional.** Only mandatory if the pilot's transcript can cross the compaction threshold or switch models. For a short deterministic route it is a smoke check, but the shared `provider_session_id` invariant means R02's pilot ledger must acknowledge RB as a co-writer.

Expanded ownership for **R02** (no new seam): add "subscription-tier model admission (`JcodeTier::ALL` ordering, per-model `min_tier`, `GET /v1/me` source of truth with offline cached-tier fallback = Plus)" to R02's owns, and record that the "reset incompatible provider session state" invariant is *implemented in RB*, not R02.

**Q3 - Should either alter the six full-review seams?**
- Do **not** displace any of R00/R01A/R02/R09B/R05B/R03A. Their incident-backed authority stands.
- **Add RA as a full or high-light seam and make it a pilot prerequisite**, because it produces the pilot's primary observable. Practically it can share R02's full review slot as a paired seam ("R02 selects the provider; RA produces and records the turn"), but it must not be silently absorbed: the Mapper's assumption that R02 emits wire metadata is wrong at the code level.
- **Add RB as a light seam** with one hard cross-seam gate: R02's and RB's ownership of `provider_session_id` must be validated together (the critic's multi-writer failure mode, here in the compaction path). This is the one place the six-seam set has a genuine composition risk the Mapper missed.
- **Strengthen R02's "owns"** with tier gating; this raises, not lowers, R02's full justification and confirms its rank-3 full status against the upstream-heavy divergence.
- Sponsored discovery reinforces **R07B** (keep deferred/light); usage accounting reinforces R02 data-model + R08B display.

**Q4 - Other unclassified clusters: new vs absorbed?**
- New (not absorbed): agent turn execution/evidence (RA), compaction (RB). Tier gating is new *within* R02.
- Absorbed: usage accounting -> R02 + R08B; sponsored discovery -> R07B; prompt text assets -> RA (logic) with low-risk text; `memory_agent.rs`/`turn_memory.rs` -> R06B; `agentgrep*` (two-sided tool) -> R07A; provider runtime streams (`openai_stream_runtime.rs`, `openrouter_sse_stream.rs`, `agent_transport.rs`, two-sided) -> R02 transport-adjacent but really RA's streaming consumer; latex/markdown streaming regressions -> R08B render.

## Explicit confidence and gaps

- Confidence high: the two-sided/one-sided numstat measurements and the fact that `agent/{compaction,evidence,prompting}.rs` are fork-only modules declared in `agent.rs`.
- Confidence medium-high: RA and RB are missing responsibilities not owned by the six seams.
- Confidence medium: exact review depth (RA full vs high-light) and whether RB should merge into R04 vs stand alone; I did not read `turn_streaming_mpsc.rs` internals (126+ fork lines) in full, only its magnitude and role, so the streaming-vs-compaction boundary inside RA/RB is not fully resolved.
- Not checked: no fork-vs-upstream *symbol-level* semantic diff of `agent.rs`/`turn_execution.rs` (the two-sided ones); I confirmed divergence magnitude and fork-added function names, not behavioral equivalence. No build/test run. Did not exhaustively grep every crate for additional `provider_session_id` writers beyond `agent/compaction.rs` and R02's model-switch path, so a third writer is possible. Did not open `subscription_catalog.rs` upstream body beyond the gating additions. Did not read the mapper's or critic's forbidden external incident notes.
- I did not modify any repo file, ref, branch, worktree, or stash.
