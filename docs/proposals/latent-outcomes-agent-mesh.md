# Latent Outcomes Agent Mesh for jcode

Status: Proposal seed, adapted from the retired `loam` vision

## Summary

Latent Outcomes Agent Mesh is a research direction for turning jcode from a strong
single-session coding agent into the executor inside a larger local-first agent
system. The core bet is that execution quality improves multiplicatively when
jcode is paired with durable memory, background consolidation, cross-device
surfaces, and specialized steering agents.

This proposal is not a commitment to build a separate product. It reframes the
legacy `loam` vision as a set of jcode-compatible experiments that can be pursued
alongside the control-plane, local-background-model, computer-use, and
cross-device proposals already in `docs/proposals`.

## Motivation

Jcode is strongest when it has the right context, the right tools, and a tight
feedback loop with the operator. It is weaker at long-horizon continuity:
remembering why prior choices were made, deciding which old experiences matter
now, and coordinating work across machines or background agents without bloating
the foreground prompt.

Most inline memory systems try to solve this by adding more retrieval directly to
the turn path. That makes every response slower and often injects stale or
irrelevant facts. The more promising split is:

- keep jcode's interactive turn path fast;
- record enough structured experience to learn from sessions;
- consolidate that experience asynchronously;
- inject only small, provenance-backed context at moments where it is likely to
  help;
- make all steering observable and controllable through jcode's emerging control
  plane.

## Thesis

A strong executor and a strong memory/consolidation system are worth more
multiplied than added, but only if the memory reaches the executor at the right
moment and in the right shape.

For jcode, the executor is the current TUI/CLI agent runtime. The mesh around it
should remain optional and sidecar-oriented at first. It should not make basic
jcode usage slower, cloud-dependent, or harder to reason about.

## Relation to existing jcode proposals

This proposal ties together several ongoing threads:

- `control-plane/README.md`: provides run inventory, state, approvals, audit
  history, and controls for steering or recovering work.
- `local-background-models.md`: provides local extraction, verification,
  retrieval, prompt packing, and safety critique without blocking the main model.
- `CROSS_DEVICE_WORKTREE_SYNC.md`: informs multi-device continuity and session
  handoff.
- `computer-use-tool.md` and `computer-use-maximal-control.md`: provide richer
  observation and action streams for sessions that involve UI state.
- `nix-backed-selfdev-reload.md`: provides a concrete example of capability-aware
  routing and safe reload orchestration.

The Latent Outcomes Agent Mesh should be treated as an umbrella research frame,
not a competing architecture.

## Architecture roles

### 1. Executor: jcode as the hands

The executor is the interactive jcode session that edits files, runs tools,
validates changes, and talks to the operator. The executor should stay focused on
current work. It should receive selected context from the mesh, but it should not
be responsible for long-running consolidation or global memory maintenance.

Near-term jcode mapping:

- TUI/CLI session and tool runtime.
- Existing transcript, tool-call, todo, background-task, and selfdev events.
- `serve`/`connect`, debug socket, or future control-plane streams as observation
  and steering surfaces.

### 2. Executive: persistent operator surface

The executive is the higher-level assistant surface that tracks operator intent
across sessions. It can inspect sessions, spawn work, resume interrupted runs,
route tasks to capable machines, and decide when older context should be offered
to an executor.

Near-term jcode mapping:

- Control-plane UI/API for session inventory and actions.
- Background agents or scheduled tasks that maintain live task state.
- Future web, mobile, or desktop surfaces that attach to the same run state.

### 3. Fabric: asynchronous consolidation

The fabric is the offline, compute-heavy layer. It turns raw experience into
structured memories, tutorials, lessons, and routing hints. It should run after or
alongside sessions, not in the critical token-stream path.

Near-term jcode mapping:

- Local background model services.
- Transcript and tool-event indexing.
- Compaction coverage verification.
- Session-to-commit and session-to-outcome records.
- Memory stores with provenance, confidence, and decay.

### 4. Steering agents: specialized co-clients

Steering agents observe the executor event stream and propose or inject small
interventions. Each agent should have a narrow lens and explicit permissions.

Candidate steering agents:

- Context steering: suggests relevant memories, proposal docs, tasks, and prior
  decisions.
- Resource steering: recommends remote machines or background workers based on
  host capabilities and current workload.
- Safety steering: flags destructive commands, unrelated dirty files, hidden
  approvals, or actions likely to hang.
- Continuity steering: updates live task state and prepares handoff summaries.

## Two-plane model

The legacy vision's most useful networking lesson is the separation between the
turn/data plane and the coordination plane.

| Plane | Job | Latency requirement | jcode implication |
| --- | --- | --- | --- |
| Turn/data plane | User input, model tokens, tool calls, active UI events | Must stay short | Avoid central middleboxes in the hot path. Prefer local runtime or direct peer links. |
| Coordination plane | Presence, inventory, routing, recovery, approvals, audit | Latency-tolerant | Central or durable services are acceptable if observable and optional. |

This split keeps interactive jcode responsive while still allowing richer
coordination above individual sessions.

## Measurement stance: latent outcomes

The value of this system is hard to measure directly. Better memory and better
steering should reduce repeated mistakes, improve handoffs, improve context
quality, and route work to better resources. Those outcomes are partly subjective
and confounded by task difficulty.

Therefore, the measurement system is part of the product:

- Capture structured session outcomes: problem, approach, commands, files,
  result, tests, commits, and user corrections.
- Score artifacts with confidence and provenance.
- Treat LLM-as-judge outputs as hypotheses, not truth.
- Re-check lessons against later sessions before promoting them.
- Measure prompt-token cost and latency for every injected context item.
- Prefer replayable fixture tests where possible.

A useful first success criterion is not "the mesh is intelligent." It is: jcode
can replay a small corpus of prior-session situations and demonstrate that the
right compact context is retrieved, budgeted, and injected with evidence.

## Consolidation ladder

Raw experience should move through a staged ladder:

1. Session: transcript, tool events, todos, background tasks, files touched, and
   commit hashes.
2. Outcome: task summary, approach taken, validation performed, final status, and
   confidence score.
3. Tutorial: a scoped how-to distilled from one or more high-confidence outcomes.
4. Lesson: a broader pattern derived from multiple tutorials or outcomes.
5. Feedback: scheduled comparison between lessons and new ground-truth sessions.

Every step should preserve links back to evidence. Lessons without provenance
should be treated as weak hints and should not be auto-injected into jcode's
foreground prompt.

## Relevance routing

Relevance routing is the central research problem. Consolidation is valuable only
when the right artifact reaches the right session at the right moment.

Candidate relevance signals:

- Current user request, active todos, and recent tool calls.
- Working directory, repository, branch, dirty files, and modified paths.
- Backlog task IDs, proposal docs, and milestone labels.
- Session history, prior errors, and previous successful approaches.
- Operator-facing conversations with the executive surface.
- Host capabilities, available models, network location, and resource pressure.

Routing should be budget-aware. More memory is not always better. Each injected
item should have a reason, source, confidence, and token cost.

## Safety and control model

The mesh should not silently steer jcode in ways the operator cannot inspect or
undo.

Rules for early experiments:

- Default to advisory suggestions before automatic injection.
- Require provenance for every memory-derived recommendation.
- Log why a context item or steering action was selected.
- Keep destructive actions behind explicit operator approval.
- Give the operator controls to pause, disable, or inspect steering agents.
- Avoid background agents competing to write directly into the same session.
- Treat local capture and consolidation stores as sensitive data.

## Phased exploration plan

### Phase 1: instrument outcomes inside jcode

Build the fuel before building intelligence.

- Persist structured session and task outcome records.
- Link sessions to commits and modified files where possible.
- Record validation commands and results.
- Add fixtures for representative successful and failed sessions.

Success metric: a later process can answer "what happened, what changed, and how
was it validated?" without rereading the full transcript.

### Phase 2: local consolidation sidecar

Use local background models and deterministic extraction to produce candidate
memories.

- Extract structured facts from sessions.
- Verify compaction coverage.
- Generate outcome records and scoped tutorials.
- Store confidence, provenance, and decay metadata.

Success metric: fixture tests catch omitted facts and produce useful retrieval
candidates without blocking interactive responses.

### Phase 3: retrieval-backed context injection

Connect consolidated artifacts back to jcode.

- Build a local index over sessions, outcomes, docs, tasks, and summaries.
- Retrieve and rerank candidates during message preparation or session start.
- Enforce a strict token budget.
- Log selection reasons.
- Start with suggestions, then graduate narrow high-confidence injections.

Success metric: old but relevant context can be surfaced in replayed tasks while
irrelevant memories remain out of the prompt.

### Phase 4: control-plane steering

Expose steering through jcode's control plane.

- Let co-clients observe session state and propose actions.
- Add operator controls for pause, approve, reject, retry, and inspect.
- Add arbitration so multiple steering agents do not fight.
- Record an audit trail for every intervention.

Success metric: a human can understand and control why a session received a
context injection or routing recommendation.

### Phase 5: distributed local-first mesh

Only after the local loop works at n=1, scale across machines.

- Move heavy consolidation to capable local inference nodes.
- Route expensive jobs based on host capabilities.
- Keep turn traffic direct or local wherever possible.
- Use coordination services for presence, routing, and recovery rather than token
  streaming.

Success metric: distributed execution improves throughput or reliability without
adding noticeable foreground latency.

## Open questions

- What is the minimal event schema needed for useful session-to-outcome records?
- Should outcome records live per worktree, globally, or both?
- What context-injection threshold is safe enough for automatic use?
- How should jcode expose steering proposals in the TUI without adding noise?
- Which memories should decay, and which should require explicit human review?
- How do we evaluate false-positive injections that subtly derail a session?
- What belongs in jcode versus host configuration managed outside jcode?

## First recommendation

Start with Phase 1 and Phase 2. Specifically, implement structured
session-to-outcome records and a local consolidation sidecar that can verify
compaction coverage and emit provenance-backed memory candidates.

This gives jcode a measurable substrate for later relevance routing while keeping
the current interactive executor fast and understandable.
