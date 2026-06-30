# Local Background Models for Context Quality (Proposed)

Status: Proposed (design only, not implemented)

## Summary

Use local models in the background as low-risk support systems for jcode's main
agent loop. The goal is not to replace the primary model. The goal is to improve
compaction, context injection, tool-calling safety, and continuity by producing
verifiable sidecar artifacts: structured memories, ranked context candidates,
coverage checks, and tool-call critiques.

Local model outputs should be treated as candidates with evidence spans, not as
authoritative mutations. High-confidence artifacts can be injected into the main
prompt or persisted as structured state. Low-confidence artifacts should be kept
as diagnostics only.

## Motivation

The native-auto compaction fallback fixed one failure mode where jcode had no
fallback if a provider did not emit native compaction output. The next class of
risk is quality: even when compaction runs, it can omit important facts, lose
active tasks, or over-inject irrelevant history.

Local background models can help because they are cheap, private, cancellable,
and do not block the main model response path. They are best used as critics,
indexers, and compressors.

## Candidate uses

### 1. Compaction coverage verifier

After a compaction summary is produced, a local model reviews the source
transcript chunks and the proposed summary. It emits a checklist of missing or
weakly represented items:

- User preferences and constraints.
- Active tasks, blockers, and unresolved questions.
- Tool outputs that affect future decisions.
- File paths, commands, test results, and commits.
- Architecture decisions and rejected alternatives.
- Safety or coordination constraints, such as unrelated dirty files.

Each finding should include evidence spans from the original transcript. The
main compaction pipeline can then append the missing facts or record a warning.

### 2. Continuous structured state extraction

Every few turns, a local model extracts structured facts into a sidecar store:

```json
{
  "user_preferences": [],
  "active_tasks": [],
  "decisions": [],
  "files_touched": [],
  "test_results": [],
  "blockers": [],
  "followups": []
}
```

Compaction can preserve this state directly instead of relying only on prose
summaries. Message preparation can inject the subset that is relevant to the
current turn.

### 3. Local embeddings and reranking for injection

Maintain a local semantic index over:

- Recent transcript chunks.
- Prior summaries.
- Structured state records.
- Tool outputs.
- Files modified in the current task.
- Backlog tasks and proposal documents.

At message-prep time, retrieve candidate context and rerank it by relevance,
recency, novelty, and risk. The prompt builder can then keep exact text,
summarize it, or omit it under a strict token budget.

### 4. Tool-call preflight critic

Before expensive or risky tool calls, a local model can lint the proposed action:

- Is the command destructive or irreversible?
- Is it likely to hang because it is interactive?
- Does it touch unrelated dirty files?
- Does it violate a user instruction or repo convention?
- Are the tool arguments malformed or ambiguous?

The critic should advise by default. It should block only on narrow,
high-confidence cases, such as obvious deletion of user data or a malformed tool
schema.

### 5. Background task planner

A local model can maintain a compact live plan:

- Current objective.
- What changed.
- What validation remains.
- Relevant files.
- Likely next commands.
- Risks and coordination notes.

This improves continuity after reloads, interruptions, and compaction.

### 6. Prompt budget packer

Given candidate context items, a local model can choose how each item should be
represented:

- Keep exact transcript.
- Keep exact tool output excerpt.
- Replace with structured fact.
- Replace with summary.
- Omit.

This provides a separate optimization layer between retrieval and final prompt
construction.

## Safety model

Local models should not silently mutate core state. They should produce artifacts
with provenance:

| Artifact | Required provenance |
| --- | --- |
| Missing compaction fact | Source transcript span and summary span, if any |
| Structured memory | Source message or tool output ID |
| Injection candidate | Retrieval score and source chunk ID |
| Tool-call warning | Proposed tool call and rationale |
| Planner update | Source event IDs |

A background model result should be accepted automatically only when the consumer
can validate it mechanically or when the result is low-risk. Otherwise it should
be advisory.

## Architecture sketch

```text
main agent loop
  ├─ emits transcript/tool events
  ├─ prepares prompts with context budget
  └─ performs compaction when needed

background local model service
  ├─ subscribes to transcript/tool events
  ├─ writes structured state candidates
  ├─ builds embeddings / retrieval index
  ├─ verifies compaction coverage
  └─ critiques risky tool calls

sidecar stores
  ├─ structured_state.jsonl
  ├─ context_index
  ├─ compaction_audit.jsonl
  └─ planner_state.json
```

The local service should be optional. If no local runtime is available, jcode
continues with the current behavior.

## Model/runtime options

The implementation should support pluggable local runtimes:

- Ollama-compatible HTTP APIs.
- llama.cpp server-compatible APIs.
- Future native embedding backends.

The first implementation should require only a small instruction-following model
for extraction and verification. Embeddings can be added separately.

## Phased plan

### Phase 1: compaction verifier and structured extraction

Build the smallest useful loop:

1. Persist compaction source chunks and produced summary metadata.
2. Ask a local model to identify missing facts with evidence spans.
3. Append high-confidence missing facts to the compaction summary or write them
   to a compaction audit file.
4. Extract structured state records from recent turns.
5. Add fixture-based tests with transcripts that intentionally omit critical
   facts from summaries.

Success metric: fixture tests catch missing preferences, file paths, active
tasks, and tool results that the baseline summary omitted.

### Phase 2: retrieval-backed injection

1. Build a local index over transcript chunks, summaries, and structured state.
2. Retrieve and rerank candidates during message prep.
3. Enforce a configurable token budget.
4. Log why each injected item was selected.

Success metric: relevant state is injected from older transcript regions without
needing to keep the entire conversation in context.

### Phase 3: tool-call preflight critic

1. Run only for high-risk tools or high-risk commands.
2. Return advisory warnings first.
3. Add hard blocks only for mechanically obvious hazards.
4. Evaluate against a corpus of safe and unsafe command examples.

Success metric: catches destructive or unrelated-file operations without adding
noticeable latency to ordinary tool use.

### Phase 4: prompt budget packer and planner

1. Maintain live planner state from event stream.
2. Let the budget packer choose exact text vs summary vs omission.
3. Compare token usage and task success across replayed sessions.

Success metric: lower prompt tokens with no loss in task-critical context.

## Open questions

- Where should sidecar artifacts live: per-session, per-worktree, or both?
- What confidence threshold is safe for auto-accepting extracted facts?
- Should verifier findings modify the compaction summary directly or be injected
  as separate audit facts?
- How should local model latency be scheduled so it never blocks interactive
  responses?
- What is the minimal local model size that reliably catches compaction
  omissions in jcode transcripts?

## First recommendation

Start with **Phase 1: compaction verifier and structured extraction**. It is the
closest follow-up to the native-auto compaction fallback, it is easy to test with
transcript fixtures, and it creates reusable state for later injection and
planning work.
