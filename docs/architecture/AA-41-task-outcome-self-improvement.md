# AA-41: Task Outcome Capture and Self-Improvement Over Time

Date: 2026-06-26
Status: research synthesis (research-first; feeds AA-42 TaskOutcome shape, AA-23 provenance, AA-27 self-improvement harness)
Pad: AA-41 (workspace `jcode`, collection Assistant Architecture)

## What this doc decides

A sourced, verdict-labelled landscape of task-outcome capture and learning-over-time techniques, plus a recommended first-cut `TaskOutcome` record shape and async lessons pipeline. The point is to avoid travelling a well-trodden road to nowhere before AA-42 freezes a schema. Verdicts: **PROMISING**, **TRIED-AND-FAILS**, **UNKNOWN**.

Headline recommendation is in [§7](#7-recommendation).

---

## 1. Grounding: the substrate Jcode already has

AA-41 is not greenfield. The AA-22 evidence spine already exists and is the source of truth a `TaskOutcome` must cite.

`jcode-session-types::SessionLogEvent` (crates/jcode-session-types/src/evidence.rs), append-only `*.evidence.jsonl` per session, typed events:

- `TurnStarted{user_message_index, image_count, input-sha}` / `TurnFinished{status, duration_ms, output-sha, error_class}`
- `ProviderRequest{provider, model, route, message_count, tool_count, prompt-sha}` / `ProviderResponse{..., status, duration_ms, usage{input/output/total tokens}, error_class}`
- `ToolStarted{tool_name, input-sha}` / `ToolFinished{tool_name, status, duration_ms, output-sha, error_class}`
- `RouteSelected{provider_key, model, api_method, source}`
- `MemoryInjected{memory_count, age_ms, prompt-sha}`
- `ChildSessionStarted{child_session_id, task-sha}`
- `PolicyDecision{policy, decision, attributes}`

Each row: `event_id`, `sequence`, `timestamp`, `session_id`, optional `parent/child_session_id`, `node` (id/host/pid/version/git-hash/cwd), `git` (root/head/branch/dirty), `correlation` (turn_id/provider_request_id/tool_call_id/task_id), sha256 payload summaries (no raw payloads). `SessionLogStatus` = Ok/Error/Cancelled/Interrupted. Schema is versioned (`SESSION_LOG_EVENT_SCHEMA_VERSION = 1`).

Two observations that shape the recommendation:

1. **The status vocabulary already exists** (`SessionLogStatus`), and `error_class` is already captured on turns/provider/tool. A `TaskOutcome` should reuse these, not invent a parallel taxonomy.
2. **`correlation.task_id` exists in the schema but is unused in the live infra log** (only `turn_id` is populated). This is the natural anchor for linking evidence rows to a `TaskOutcome`. Wiring it is the cheapest enabling change.

---

## 2. Q1+Q5 - Capturing outcomes/trajectories and generalizing into lessons (no fine-tuning)

- **Reflexion (verbal RL / episodic reflective memory) reliably improves agent performance** (e.g. HumanEval pass@1 gains) by storing natural-language reflections on failure and recalling them next trial. The canonical "learn from outcomes without weight updates" result. **PROMISING.** Shinn et al., arXiv:2303.11366 (verified).
- **ExpeL: agents autonomously gather experiences and extract natural-language INSIGHTS across trajectories, improving as experience accumulates, no parametric updates.** This is the model for an async lessons pipeline that distils many outcomes into reusable rules. **PROMISING.** Zhao et al., arXiv:2308.10144, AAAI-24 (verified).
- **A-MEM and 2025 agentic-memory work formalize structured episodic memories supporting retrieval of past-trial lessons without fine-tuning.** Structured > free-form for retrieval utility. **PROMISING.** A-MEM, arXiv:2502.12110.
- **Agent Workflow Memory (AWM): induce and reuse common subroutines/workflows from traces.** Practical pattern (community implementations) but more engineering-first than peer-validated; treat as a pattern, not a proven science. **UNKNOWN / PRACTICAL.** Agent Workflow Memory.
- **Trajectory-level distillation (compress thought-action-observation traces into smaller artifacts/skills) is an emerging route to package lessons.** Early results; promising but not yet robustly generalizing. **PROMISING / EARLY.** arXiv:2506.14728, arXiv:2509.14257.
- **Case-based / lesson recall (store JSONL trace + human-readable insight, cite prior sessions at runtime) is robust and low-risk.** This maps one-to-one onto Jcode's append-only evidence + derived-lesson model. **PROMISING.** ExpeL/Reflexion-style pipelines.

**Metadata that earns its keep vs collected-and-never-used:** the literature consistently uses (a) the verifiable outcome status, (b) the error/failure signal, (c) a short natural-language lesson, and (d) a retrieval key (task type/approach). Heavy per-step annotation and free-form commentary is the part that tends to be collected and never used (and actively harmful, §5). Jcode's spine already captures (a) and (b); (c) and (d) are the derived layer.

---

## 3. Q2+Q3 - Outcome signals and whether subjective/judge feedback helps

- **Verifiable outcome signals (tests pass, build success, environment reward) are the cleanest, least-gameable objective for coding/engineering agents.** For Jcode: `ToolFinished{status, error_class}` for build/test commands, and git/validation state, are first-class verifiable signals already in the spine. Prefer these over subjective scores. **PROMISING.** Reflexion/ExpeL evals; SWE-bench-style outcome signals.
- **LLM-as-judge trajectory grading approximates human judgment but is imperfect (best judges ~80% agreement on some web-agent tasks) and needs periodic human calibration.** Use a judge for the *subjective* questions (did the task form correctly, was the approach sound) where no verifiable signal exists, on a sample, not every task. **PROMISING with CAUTION.** *Evaluating Automatic Evaluations of Web Agent Trajectories*, arXiv:2504.08942.
- **Self-consistency / sampling-based consensus reduces brittle single-output judgments.** Jcode's memory consensus-rerank already applies this pattern; reuse it for judge calls. **PROMISING.** Wang et al., arXiv:2203.11171.
- **Process Reward Models (dense stepwise feedback) can outperform sparse outcome-only reward for long-horizon planning, but are heavier to build.** For a first cut, outcome-only (status + error_class) is the right cost/value point; PRM is a later option. **PROMISING / EARLY.** PRM literature, 2025.
- **Hybrid (verifiable outcome checks anchor correctness + judge/PRM for intermediate/subjective gaps) is the recommended design.** Outcome status is the spine; the judge fills only the subjective fields. **PROMISING.** ExpeL/Reflexion patterns.

**Answer to Q3/Q4 (does subjective/judge confidence help, and how to elicit "did we form the right task"):** subjective confidence helps *only where there is no verifiable signal* and *only if the judge is calibrated against humans periodically*. It should be (a) optional, (b) elicited from an LLM-judge on a sampled cadence (not per task), (c) stored as a separate, clearly-derived field with its judge/model provenance, and (d) used for weighting/gating retrieval of lessons, never as a hard training target. Eliciting "did we understand the request / form the right task" is exactly the subjective gap a judge is for; capture it as `formation_confidence` with provenance, and validate that it correlates with downstream outcome before trusting it.

---

## 4. Q6 - Identifiers/schema for later trend analysis

The spine already gives stable `session_id`, `event_id`, `sequence`, `correlation.task_id` (unused), `node`, `git`. For cross-task trend clustering we additionally need: a stable `outcome_id`, a `task_type`/`approach` tag (for clustering by what was attempted), and the `status` + `error_class` (for what worked vs not). nDCG-style graded analysis and clustering by `task_type x approach x status` is then a pure offline query over append-only records. **TRIED-AND-WORKS** (standard retrieval/trend-eval practice; BEIR-style metrics).

---

## 5. Q2 (cont.) - What is known to FAIL

- **Letting an agent rewrite its own system prompt / canonical policy without external verification -> prompt drift and self-reinforcing errors.** Do not auto-apply lessons to the live prompt; lessons are retrieved candidates, not auto-edits. **TRIED-AND-FAILS.** Agent-memory/reflection literature; ICRH arXiv:2402.06627.
- **Unbounded free-form reflection appended to memory -> drift, hallucinated causal lessons, poor retrieval utility.** Lessons must be bounded, structured, and outcome-cited. **TRIED-AND-FAILS.** Reflexion free-form risk discussion.
- **Memory poisoning: corrupted/malicious traces in the evidence spine, and in-context reward hacking, are realistic unless write-access is restricted and inputs validated.** The append-only spine + payload sha + provenance is the mitigation; derived lessons must cite verifiable evidence. **TRIED-AND-FAILS / HIGH RISK.** AgentPoison arXiv:2407.12784; ICRH arXiv:2402.06627.
- **Catastrophic forgetting / lesson churn:** continually regenerating the lesson set risks losing good lessons. Lessons need stable ids, versioning, and a no-regression gate (§6), not wholesale rewrites.

---

## 6. Measured self-improvement (shared with AA-49 §6)

The same replay/eval gate AA-49 specifies applies here: a self-improvement change (new lesson surfaced, new outcome-derived heuristic) is accepted only if it does not regress against recorded sessions / golden tasks.

- **Evaluate by replay/A-B on recorded session traces + held-out golden tasks; gate auto-updates on no-regression.** **PROMISING / REQUIRED PRACTICE.** ExpeL/Reflexion methodology.
- **Use benchmark suites + deterministic JSONL replay so offline A/B is possible.** The evidence spine is the JSONL substrate. **PROMISING.** Reflexion/ExpeL eval suites.
- **Calibrate LLM-judges against human labels periodically to avoid silent drift.** **PROMISING with CAUTION.** arXiv:2504.08942.
- **Track multi-metric dashboards (outcome correctness, cost, token usage, failure modes), gate on no-regression on golden tasks.** Token usage and duration are already in `ProviderResponse.usage` / `*_duration_ms`. **PROMISING.** ExpeL/Reflexion methodology.

This is the AA-27 self-improvement harness, and it shares the AA-49 §6 replay harness. Build once, use for both context-injection and lessons evaluation.

---

## 7. Recommendation

### 7.1 Recommended first-cut `TaskOutcome` record shape (feeds AA-42)

Versioned, append-only, citing evidence. Reuse existing types; do not invent a parallel task or status concept.

```
TaskOutcome (schema_version):
  outcome_id            stable id (trend anchor)
  task_id               = correlation.task_id (wire the existing unused field)
  session_id            owning session
  origin                { session_id, event_id }   // where the task was formed
  status                reuse SessionLogStatus (Completed/Failed/Blocked/Superseded;
                                                  Cancelled/Interrupted as needed)
  error_class           optional, reuse the spine's error_class vocabulary
  work_pointer          { git: {head, range?}, evidence_event_range: [seq_lo, seq_hi],
                          tool_runs: [event_id...] }   // cite, don't copy
  approach_ledger       [ { approach, worked: bool, note? } ]   // bounded, structured
  task_type             tag for clustering (Q6)
  formation_confidence  optional { value, source: judge|human, model?, judged_at }
  status_confidence     optional { value, source, model?, judged_at }
  annotations           optional, bounded; whole-set preferred over per-step
  created_at, node, git
```

Principles:

- **Cite, never copy.** `work_pointer` and `origin` are event-id/sequence references into the spine, keeping outcomes thin and the spine the single source of truth.
- **Reuse `SessionLogStatus` and `error_class`.** No new status taxonomy.
- **Confidence fields are optional and provenance-tagged.** Per §3, subjective scores only where no verifiable signal exists, judge-sourced on cadence, validated before trusted.
- **Bounded approach ledger, not free-form reflection.** Per §5.
- **Most fields are for measurement, not display** (matches AA-42's stance); the TUI surfaces only status + a one-line summary.

### 7.2 Async lessons pipeline

```
evidence spine (append-only, source of truth)
   -> TaskOutcome records (append-only, cite evidence)
   -> [ASYNC, offline, never hot-loop] lesson distillation (ExpeL-style insight extraction
        over clusters of {task_type, approach, status})
   -> Lessons store (structured, versioned, stable ids, each cites underlying outcomes)
   -> retrieval at runtime as CANDIDATE context (never auto-applied to the prompt)
   -> §6 replay/eval gate before any lesson is allowed to influence behavior
```

- Distillation runs offline (like the memory maintenance loop), not in the agent's turn.
- Lessons are retrieved as candidates through the existing memory/salience pipeline (AA-49 §4), subject to the precision judge.
- No lesson auto-edits the system prompt (§5). Lessons surface as retrieved context; the §6 gate measures whether surfacing them improves or degrades.

### 7.3 Smallest enabling change

Wire `correlation.task_id` (already in the schema, currently unused) to the built-in task function, and emit a `TaskOutcome` at task close that cites the evidence `sequence` range. That alone makes the spine queryable by task and unblocks AA-42's schema work without building the lessons pipeline yet.

### Gating map

- **AA-42**: adopt §7.1 shape; the schema is now grounded, not speculative. Wire `task_id` first.
- **AA-23** (provenance/confidence): the optional confidence fields + judge-provenance model in §3/§7.1 are the provenance contract.
- **AA-27** (self-improvement harness): shares the AA-49 §6 replay/eval gate. Build once.

---

## Appendix: sources

Verified directly: Generative Agents arXiv:2304.03442; ExpeL arXiv:2308.10144. Cited from research synthesis (2023-2025): Reflexion arXiv:2303.11366; A-MEM arXiv:2502.12110; trajectory/agent distillation arXiv:2506.14728, arXiv:2509.14257; LLM-judge evaluation arXiv:2504.08942; Self-Consistency arXiv:2203.11171; ICRH arXiv:2402.06627; AgentPoison arXiv:2407.12784; Agent Workflow Memory; PRM literature (ACL/arXiv 2025).
