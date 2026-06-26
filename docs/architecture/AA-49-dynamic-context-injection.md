# AA-49: Dynamic Context Injection, Reactive-Steering Stability, Relevance vs Salience

Date: 2026-06-26
Status: research synthesis (research-first; gates AA-45 reactive part, AA-51 feedback loop)
Pad: AA-49 (workspace `jcode`, collection Assistant Architecture)

## What this doc decides

1. Whether (and how) Jcode should ever inject context **in reaction to the previous turn's outcome**.
2. How to make the existing per-turn dynamic context mechanism stable rather than thrash-prone.
3. How to close the gap between *embedding-space relevance* and *user salience*.
4. A measurement method (replay/eval over the AA-22 evidence spine) that must exist **before** any outcome-reactive injection ships.

The headline recommendation is up front in [§7](#7-recommendation). The body is sourced; each external claim carries a verdict label of **PROMISING**, **TRIED-AND-FAILS**, or **UNKNOWN**.

---

## 1. Grounding: what Jcode actually does today (verified in code)

This is not a greenfield design. Jcode already has a per-turn dynamic context seam and a memory-retrieval pipeline. The research has to improve *these*, not invent from scratch.

### 1.1 The per-turn dynamic injection seam

`jcode-message-types::messages_with_dynamic_system_context` (crates/jcode-message-types/src/lib.rs:439) inserts a recomputed `<system-reminder>` user message **immediately after the latest fresh user-text message**, every turn:

```rust
pub fn messages_with_dynamic_system_context(messages: &[Message], system_dynamic: &str) -> Vec<Message> {
    // ... wraps system_dynamic in <system-reminder>...</system-reminder>
    // inserts after rposition(is_fresh_user_text_message)
}
```

Critically, it inserts **after** the cached history prefix, so it does not invalidate the KV cache of the stable prefix. This is the right place to put deterministic per-turn context (time, env, persona). It is also the place an outcome-reactive injector would live, which is exactly why this item is research-first.

The system prompt itself is split (`prompt.rs::build_system_prompt_split`, crates/jcode-base/src/prompt.rs:291) into:

- `static_part` (cached): `DEFAULT_SYSTEM_PROMPT`, selfdev guidance, AGENTS.md, `~/.jcode` + `./.jcode` overlays, preferred-tools, skills list.
- `dynamic_part` (not cached): memory prompt, active skill prompt.

So Jcode already has a clean two-layer model: **static deterministic rails (cached)** vs **dynamic turn context (uncached)**. The question AA-49 answers is what is allowed to live in the dynamic layer and under what stability discipline.

### 1.2 The memory retrieval pipeline (the relevance/salience problem)

Per turn (`memory_agent.rs`):

1. **Embed** the context window (jcode-embedding, all-MiniLM-L6-v2 ONNX) -> query embedding.
2. **Hybrid retrieve**: `find_similar_hybrid` = dense cosine + BM25, fused with RRF (memory_agent.rs:652). (Pure dense with a 0.5 cosine floor was shown to surface ~nothing on real windows; hybrid recovers recall.)
3. **Filter** already-surfaced / already-injected memories per session.
4. **Select** which to surface:
   - **Mode 1 (no LLM):** `dynamic_gate_select` (memory_agent.rs:71) keeps the top candidate, then keeps each next candidate only while its score stays within `GATE_REL_FLOOR=0.90` of the top **and** within `GATE_DROP_RATIO=0.95` of the previous kept score. First gap truncates. Variable k in `1..=MAX_MEMORIES_PER_TURN(5)`. Bench (self-dev corpus, 150 windows): precision@5 0.23 -> 0.36 (+56%), avg injected 5.0 -> ~2.25/turn, zero added cost. **Cannot reach 0 injection** on no-memory turns (proven: no zero-cost score separates them).
   - **Mode 2 (sidecar/LLM):** a single **listwise consensus rerank** (`memory_rerank::rerank_candidates_consensus_attributed`, memory_rerank.rs) over a **focused query** (`format_focused_query_for_relevance`), with `memory_rerank_votes` / `memory_rerank_min_agree` as a precision judge. Cadence-gated by `should_run_rerank` (memory_agent.rs:314): fires on first turn, on topic change (`TOPIC_CHANGE_THRESHOLD=0.3`), or every `memory_rerank_cadence` turns; skipped turns re-surface only `last_verified_ids` (never raw hybrid).

This is already a sophisticated relevance pipeline. The honest critique in the pad item stands: **it sometimes injects geometrically-near but contextually-inane facts**, because every stage above is a *similarity* estimator. None of them model *user salience* (what the user pinned, corrected, or kept returning to). That is the gap §4 addresses.

### 1.3 The evidence spine (the measurement substrate)

`jcode-session-types::SessionLogEvent` + `jcode-base/src/session/evidence.rs`: append-only `*.evidence.jsonl` per session, typed events (`TurnStarted/Finished`, `ProviderRequest/Response` with token usage, `ToolStarted/Finished` with status+duration+error_class, `RouteSelected`, `MemoryInjected{memory_count, age_ms, prompt}`, `ChildSessionStarted`, `PolicyDecision`). Each row carries node/git/correlation(turn_id)/sequence/timestamp, sha256 payload summaries (no raw payloads). This already records, for the live infra session, every turn and provider call. It is the right replay substrate for §6.

---

## 2. Question 1 - Outcome-responsive context steering: where it helps vs thrashes

The fear in the pad item is correct and well-supported: **injecting context as a direct reaction to the previous turn's findings is likely to degrade more than it improves.** The literature is consistent.

- **Closing the loop on your own outputs causes in-context reward hacking (ICRH).** An agent that feeds its own prior outcomes back into context optimizes the local signal and drifts into harmful side-effects; static-dataset evals miss it. **TRIED-AND-FAILS** (the naive reactive loop). Pan, Jones, Jagadeesan, Steinhardt, *Feedback Loops With Language Models Drive In-Context Reward Hacking*, ICML 2024, https://arxiv.org/abs/2402.06627
- **Intrinsic self-correction (react to your own critique each turn, no external signal) often does not help and can degrade reasoning.** **TRIED-AND-FAILS.** Huang et al., *Large Language Models Cannot Self-Correct Reasoning Yet*, ICLR 2024, https://arxiv.org/abs/2310.01798
- **Recursive conditioning on self-generated content -> model collapse: tails vanish, variance shrinks, errors compound.** About training-time recursion, transfers by analogy to an in-context self-loop. **TRIED-AND-FAILS** (uncurated self-loop). Shumailov et al., Nature 2024, https://www.nature.com/articles/s41586-024-07566-y
- **Reacting to prior-turn feedback DOES help when the signal is grounded/external** (a real test pass/fail, a build result), stored as episodic reflection. **PROMISING** (with external/grounded signal). Shinn et al., *Reflexion*, https://arxiv.org/abs/2303.11366
- **Self-Refine (iterative self-feedback) improves generation/formatting tasks ~20% absolute**, but this is contested for pure reasoning (see Huang above). **PROMISING (generation) / contested (reasoning).** Madaan et al., https://arxiv.org/abs/2303.17651
- **Compounding per-step error bounds long-horizon success**; recent frontier gains came mainly from improved *reliability / mistake recovery*, not raw reasoning. A small per-turn reactive error compounds over the horizon. **PROMISING (empirical).** Kwa et al. (METR), *Measuring AI Ability to Complete Long Software Tasks*, NeurIPS 2025, https://arxiv.org/abs/2503.14499

**Synthesis for Jcode:** outcome-reactive injection is safe only when the "outcome" is an *external, verifiable* signal (test result, build status, tool exit code, explicit user correction) and the reaction is *episodic and grounded*, not "the model decided last turn felt off, so re-steer the prompt." Reacting to the agent's own soft signals (its prior reasoning, its own retrieved memories) is the documented failure mode.

---

## 3. Question 2 - Stability techniques: how to avoid rocking the context

If anything reactive ships, it must be damped. Two classes of technique apply: **prefix stability (cache + attention)** and **signal damping (control theory transferred)**.

### 3.1 Prefix stability is a hard, measurable cost

- **KV-cache hit rate is the dominant production metric; a single-token prefix change (e.g. a per-second timestamp in the system prompt) invalidates the cache from that point, ~10x cost.** Strongest argument against mutating cached context per turn. **TRIED-AND-FAILS** (frequent prefix mutation). Manus, *Context Engineering for AI Agents*, https://manus.im/blog/Context-Engineering-for-AI-Agents-Lessons-from-Building-Manus
- **Make context append-only; mask tool logits instead of adding/removing tools mid-loop** (mutating tool defs near the prefix invalidates cache and causes schema violations/hallucinations when prior actions reference now-absent tools). **TRIED-AND-FAILS (dynamic mutation) -> PROMISING (masking).** Manus, same.
- **Attention budget is finite and superlinear** (n^2 pairwise); "smallest set of high-signal tokens" beats ever-growing context. **PROMISING (authoritative synthesis).** Anthropic, *Effective Context Engineering for AI Agents*, https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents
- **Context rot is architectural**: across 18 frontier models accuracy degrades as input grows even at small lengths; coherent-but-irrelevant structure hurts *more* than shuffled text. **PROMISING/established (empirical).** Chroma, *Context Rot*, https://research.trychroma.com/context-rot

Jcode already respects this: dynamic context inserts *after* the cached prefix (§1.1), and the static/dynamic split (§1.2) keeps persona/memory out of the cached prefix. **Conclusion: keep deterministic rails in `static_part` (cached); keep all per-turn variation in the post-prefix dynamic message; never put reactive content in the cached prefix.**

### 3.2 Signal damping (control theory, transferred by analogy)

No source offers a formal control-theory treatment of prompt-context injection; this transfer is an open contribution, supported by analogy to the noise/variance results. Candidate primitives, ordered by how well they map:

- **Average over samples before committing (self-consistency)** is the cleanest variance-reduction analog (+17.9% GSM8K). For Jcode: the consensus rerank (`memory_rerank_votes`/`min_agree`) is *already* an ensemble-damping mechanism. **PROMISING (empirical).** Wang et al., *Self-Consistency*, ICLR 2023, https://arxiv.org/abs/2203.11171
- **Act only on sustained/corroborated signal.** A single distractor already degrades performance; require N-of-M corroboration before injecting. Maps to a **change budget** and an **EMA/hysteresis band** on any reactive score. **PROMISING (empirical support for the principle).** Chroma, *Context Rot* (distractors).
- **Proactive compaction beats reactive correction** (prevent the bad signal from integrating; post-hoc cleanup is irreversible-damage-prone). Maps to debounce / anti-windup. **PROMISING (vendor-empirical).** Morph, *Context Rot: The Complete Guide*, https://www.morphllm.com/context-rot
- **Write-rarely / externalize state (sticky state):** durable notes/files + just-in-time references rather than mutating the live prompt every turn. **PROMISING (production-reported).** Anthropic, *Effective Context Engineering*.
- **Recitation as a low-pass attention bias:** re-state a stable plan/todo at the end of context to damp goal drift over long loops. **PROMISING (anecdotal).** Manus, *Context Engineering* (recitation).
- **Keep failures in context rather than scrubbing** (leaving wrong action+observation shifts the prior away from repeating it). Selectively *not reacting* aids stability. **PROMISING (anecdotal).** Manus, same.
- **Caution: over-uniform/sticky context causes behavioral mode collapse** ("don't few-shot yourself into a rut"); damping must preserve some diversity. **PROMISING but double-edged (anecdotal).** Manus, same.

**Concrete damping recipe for any future reactive signal in Jcode:**

1. Compute a per-turn reactive score `s_t` from an *external* signal only (tool/build/test outcome, explicit user correction), never from the model's own soft self-assessment.
2. Smooth it: `e_t = alpha*s_t + (1-alpha)*e_{t-1}` (EMA), small `alpha`.
3. Hysteresis band: only *enter* a reactive state when `e_t > high`, only *leave* when `e_t < low` (`low < high`), so it cannot oscillate turn-to-turn.
4. Change budget: cap reactive edits to <= K per N turns; otherwise hold the last decision (sticky).
5. Append-only, post-prefix: the reactive content goes in the dynamic `<system-reminder>`, never in the cached prefix.
6. Always-revertable: every reactive injection is a pure function of recorded evidence, so a replay can reproduce or remove it (see §6).

---

## 4. Questions 3 & 4 - Relevance precision and salience vs similarity

### 4.A What Jcode has vs what it lacks

Jcode's pipeline is a strong *similarity/relevance* estimator (dense+BM25 RRF, dynamic gate, LLM listwise consensus rerank). It has **no first-class salience channel**. Every signal it uses is "how similar is this memory to the current window," none is "how much did the human emphasize this."

### 4.B Recommended salience model (additive, auditable)

Borrowing the Generative Agents memory-stream decomposition (retrieval = recency + importance + relevance, combined, not relevance alone), Jcode should compute a final surface score as an **explicit weighted sum of named, separately-loggable components**, not a single opaque cosine:

```
score = w_rel * relevance      // current hybrid/rerank score (have)
      + w_rec * recency         // time/turn decay since last mention (cheap, deterministic)
      + w_imp * importance      // pin / correction / explicit-emphasis flag (NEW salience channel)
      + w_freq * frequency      // how often the user returned to this (cheap)
```

- **recency**: deterministic decay; the agent already tracks turn counts and last-mention. Cheap, no LLM.
- **importance/pin**: a first-class boost when the user *pinned*, *corrected* ("no, use X"), or repeated emphasis. This is the "user emphasized X even at low embedding similarity" lever the pad item asks for. It is metadata, not geometry, so it survives low cosine similarity.
- **frequency**: count of distinct turns/sessions that referenced the memory.

Each component is logged separately (provenance), so a surfaced memory can answer "why was I shown." This composes with, and is gated by, the existing rerank precision judge.

### 4.C Verdicts (salience/relevance literature)

**Relevance precision (thresholds vs gates vs LLM rerank):**

- **Large LLM/cross-encoder rerankers (Cohere Rerank, bge-reranker) beat raw cosine on RAG-style ranking, but add 10s-100s ms P95 latency.** Use them only when top-k precision materially changes downstream output. **PROMISING (cost-sensitive).** Cohere Rerank 3.5 / community latency benchmarks.
- **Learned/score-relative gating that conditionally invokes the reranker only for borderline candidates is the cost/quality sweet spot.** This is exactly what Jcode's `dynamic_gate_select` + cadence-gated consensus rerank already do; the literature endorses the existing architecture. **PROMISING.** Hybrid-gating RAG engineering synthesis.
- **Whether a listwise LLM rerank (RankGPT-style) beats a cosine threshold is task- and model-dependent; peer-reviewed head-to-heads are thin, vendor benchmarks dominate.** Jcode should not assume the rerank is always a win; it must be measured (see §6). **UNKNOWN.** RankGPT / bge-reranker community benchmarks.

**Salience vs similarity (the core gap):**

- **The Generative Agents retrieval heuristic (recency + importance + relevance, combined) is a proven baseline for surfacing salient memories, not relevance alone.** This is the direct model for §4.B. **TRIED-AND-WORKS.** Park et al., *Generative Agents*, arXiv:2304.03442 (verified).
- **MemGPT/Letta and Mem0 treat pins, edits, frequency, and recency as first-class metadata, and report it materially helps long-session coherence.** **PROMISING.** MemGPT/Letta/Mem0 project docs.
- **Explicit user actions (pins, manual corrections) are the strongest single salience signal and should be modeled as HARD boosts, not soft similarity features.** This is the direct answer to "user emphasized X even at low embedding similarity." **TRIED-AND-WORKS (engineering best-practice).** MemGPT/Letta design + Generative Agents commentary.
- **LLM/learned importance scoring at write-time (or via periodic reflection) surfaces infrequent-but-high-salience memories**, trading cost vs freshness. **PROMISING.** Generative Agents + MemGPT/Letta.

**Expressing emphasis as a first-class boost:**

- **Learned-sparse expansion (SPLADE family) surfaces lexically-important items dense embeddings miss; it is a practical first-stage retriever / learned query expansion.** **PROMISING.** SPLADE (SIGIR).
- **Hybrid sparse+dense with RRF reduces misses where the user emphasized a low-semantic-match token.** Jcode already does dense+BM25 RRF; SPLADE would be an upgrade path, not a prerequisite. **TRIED-AND-WORKS.** Hybrid RAG / SPLADE experiments.
- **Metadata/recency boosts (time-decay multipliers, pinned-item overrides) are low-cost, high-impact, deterministic interventions.** This is the cheapest first step for Jcode (no model needed). **TRIED-AND-WORKS.** Generative Agents heuristic.

**Offline measurement (feeds §6):**

- **precision@k, recall@k, nDCG (graded/salience-weighted), MRR are the core metrics; nDCG best captures graded salience.** **TRIED-AND-WORKS.** Retrieval-eval surveys / BEIR.
- **Layered eval set: small high-quality manual seed + teacher-model synthetic labels for scale + human/adversarial spot-checks** is the practical cheap pipeline. **PROMISING.** Retrieval-eval guides.
- **Label with task-specific cues ("would this memory be useful given user cue X?") rather than raw topical relevance, to align offline labels with salience.** **PROMISING.** RAG-eval advice.
- **Track downstream LLM failure modes (hallucination, faithfulness) as the operational signal that offline retrieval gains are real.** Mirrors §6.2's outcome-correlated metric. **PROMISING.** RAG faithfulness writeups.

---

## 5. Question 5 - Static vs dynamic: where the line is

| Layer | Lives in | Cached? | Reactive? | Examples |
|---|---|---|---|---|
| **Static rails** | `prompt.rs::static_part` | yes (shared prefix) | no | base prompt, AGENTS.md, skills, `.jcode` overlays |
| **Dynamic turn context** | post-prefix `<system-reminder>` + `dynamic_part` | no | deterministic-per-turn only (today) | time/env, memory prompt, active skill, **assistant persona/startup_reminder** |
| **Reactive steering** | (does not exist yet) | no | yes - GATED | only external-signal-driven, EMA+hysteresis damped |

**The line:** dynamism earns its latency/instability cost only when (a) the content genuinely changes per turn and (b) it is *deterministic given recorded inputs* (so it is replayable) **or** (c) it is reactive but external-signal-driven and damped per §3.2.

**Where persona goes (decided in implementation, AA-45):** persona/startup_reminder is deterministic-per-profile, but it does **not** go in `static_part`. The static prefix is the *shared, cache-stable* rails; per-profile persona there would fork the prompt cache across profiles and bleed into plain non-assistant sessions. Instead persona lands in the **dynamic (uncached) part** (`SplitSystemPrompt::append_assistant_persona`), which keeps the cached prefix byte-identical across profiles and plain sessions while still steering the model. It is deterministic (no prior-turn reactivity), so it is safe to ship now. Anything outcome-reactive waits on §6.

**AA-45 implication:** the static (deterministic) persona injection is safe to ship immediately and is implemented. The "reactive part" of AA-45 (adjusting persona/context in response to prior-turn behavior) is gated on the measurement method below.

---

## 6. Question 6 - Measurement: replay/eval over the evidence spine

**Non-negotiable gate: no outcome-reactive injection ships before this exists.** Without an offline improve-vs-degrade measurement, any reactive change is unfalsifiable and the literature says it will most likely degrade silently.

### 6.1 Substrate

The AA-22 evidence spine (`*.evidence.jsonl`) already records, per session, the turn boundaries, provider requests/responses (with token usage), tool outcomes, route selections, and `MemoryInjected{memory_count, age_ms, prompt-sha}`. The live infra session is already producing this. This is the replay corpus.

### 6.2 Replay harness design (smallest viable)

1. **Recorded sessions as fixtures.** Take N real `*.evidence.jsonl` (plus the paired session snapshot for message content) as a frozen golden set.
2. **Deterministic re-derivation.** For a context-injection change, re-run *only the injection decision* (which memories/persona/reactive content would be injected at each turn) against the recorded inputs, holding the model responses fixed from the recording. The injection function must be pure over recorded inputs (this is why §3.2 step 6 matters).
3. **Two metrics, no human in the loop per turn:**
   - **Injection diff metrics** (deterministic, free): per turn, count injected items, churn (how many changed vs prev turn), and a stability score (1 - churn_rate). A reactive change that increases churn without evidence of benefit is rejected.
   - **Outcome-correlated metrics** (uses recorded outcomes): join injected content to the *next* `TurnFinished.status` / `ToolFinished.status` / build-test outcome in the spine. A good injection should not precede more errors. This is a *correlational* guardrail, not proof of causation, but it catches regressions.
4. **LLM-judge spot check (cadence, not per-turn).** For a sampled subset, an LLM-as-judge grades "was the injected context relevant and non-distracting given the turn," reusing the existing rerank/judge infra. This is the precision oracle; it is expensive so it runs on a sample, mirroring `should_run_rerank` cadence.
5. **Regression gate.** A context-injection change is accepted only if: stability score does not drop materially, outcome-correlated error rate does not rise, and judged precision@k does not drop. Mirrors the existing memory bench (precision@5 0.23->0.36) but generalized and run over real session replays.

### 6.3 Why this is sufficient as a gate

It gives an A/B over recorded sessions without re-calling the model for every turn (cheap, deterministic where possible), with one expensive LLM-judge sample for precision. It directly answers "did this injection change improve or degrade," which is the precondition the pad item sets.

---

## 7. Recommendation

1. **Ship now (no measurement needed):** AA-45 *deterministic* persona/startup_reminder injection. **Implemented:** placed in the post-prefix dynamic part (`SplitSystemPrompt::append_assistant_persona`), not the cached prefix, so plain non-assistant sessions and the shared prompt cache are byte-unaffected. Low-risk, high-leverage.
2. **Build before any reactive work:** the §6 replay/eval harness over the evidence spine. This is the gate. It is also reusable by AA-41/AA-42 (self-improvement) and AA-51 (feedback loop).
3. **Add a salience channel (§4.B)** as an explicit, separately-logged additive component (recency + importance/pin + frequency) on top of the existing relevance pipeline, validated through the §6 harness against the current memory bench. This is the concrete answer to "embedding relevance != user salience" and is *not* outcome-reactive, so it is safe once measured.
4. **Defer / gate:** any outcome-responsive steering (per-turn reaction to prior-turn findings). Allowed only when (a) the §6 harness exists, (b) the trigger is an *external verifiable* signal, and (c) it is EMA+hysteresis+change-budget damped and append-only post-prefix (§3.2). Reacting to the model's own soft self-signal is rejected by the literature (§2).

### Gating map

- **AA-45**: static subset -> ship now. Reactive subset -> gated on §6 harness.
- **AA-51** (feedback loop): blocked on §6 harness; editable-surface design may proceed.
- **AA-41/AA-42** (self-improvement): the §6 replay harness is the shared measurement substrate.

---

## Appendix A: Stability/damping sources (Question 1-2)

(Full list inline in §2-§3. Strongest empirical anchors: Pan et al. ICRH, Chroma Context Rot, Shumailov collapse, Cuconasu RAG noise, Huang self-correction, Wang self-consistency, METR horizon. KV-cache/recitation/keep-failures are credible production anecdote, not controlled experiment.)

- Cuconasu et al., *The Power of Noise: Redefining Retrieval for RAG Systems*, SIGIR 2024, https://arxiv.org/abs/2401.14887 - retrieving high-scoring-but-irrelevant near-miss passages actively hurts; the most query-similar wrong items are the most damaging. **TRIED-AND-FAILS (naive similarity retrieval).** Directly motivates the salience channel.
- Chen et al., *AgentPoison*, https://arxiv.org/abs/2407.12784 - poisoning <0.1% of agent memory yields >80% attack success via the retrieve-into-context pathway. **TRIED-AND-FAILS (unverified reactive memory).** Motivates provenance + the precision judge.
- Cognition, *Don't Build Multi-Agents*, https://cognition.ai/blog/dont-build-multi-agents - reacting to sibling-turn partial outputs without shared context diverges. **TRIED-AND-FAILS (parallel reactive agents).**

## Appendix B: Salience/relevance sources (Question 3-4)

*(Inserted from parallel-cli salience report.)*
