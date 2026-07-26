<!--
Author's own revision of latent-outcomes-agent-mesh.md, recovered from a
long-lived stash ("wip before upstream sync") that was never applied.

Kept as a separate file rather than merged over the main proposal because the
two are not strictly better/worse than each other. This draft adds material the
polished version does not have: the concrete homelab motivation (heterogeneous
nodes, a thin laptop that is bad at compilation and parallel work), the
three-tier primary/secondary/tertiary worker taxonomy, coordinator-node routing,
and a sharper measurement stance. It also cuts prose the polished version is
better at. Choosing either wholesale would lose something, and rewriting the
author's first-person framing into house voice is not mine to do unasked.

Recoverable independently as tag `archive/stash-5`.
-->

# Latent Outcomes Agent Mesh for jcode

Status: Proposal seed, adapted from the retired `loam` vision

## Summary

Latent Outcomes Agent Mesh is a research direction for turning jcode from a strong
single-session coding agent into the executor inside a larger local-first agent
system. The core bet is execution quality, speed, efficiency, and session-over-session improvements will all benefit from further extension of the existing client-server architecture.
The idea materializes around the idea that certain hardware nodes on my homelab each have unique strengths, and that--with the proper coordination--work spread across several nodes will get done faster and with less tax on any individual system's resources than it would if the work happened on a single client (usually, I'm working from a thin laptop, which is especially not conducive to compilation and many parallel processes). The idea also encourages the development of features and techniques that would benefit jcode regardless of whether the work happens on one machine or many: new control-planes, better utilization local-background-models, computer-use - especially for testing and validation, more rigorous evaluation and reinforcement, better support for reflexive self-development features, etc.
An earlier draft of cross-device proposals is already in `docs/proposals`.

## Motivation

Jcode is strongest when it has the right context, the right tools, and a tight
feedback loop with the operator. It is weaker at long-horizon continuity:
remembering why prior choices were made, deciding which old experiences matter
now, and coordinating work across machines or background agents without overwhelming foreground context, pinning local compute, introducing conflicting edits, increasing probability of file misplacement, cache poisoning, etc.

One idea for a multi-plane split:

- fast-client (local runtime or p2p connection) gives fast interactive turn path;
- coordinator node: spins up and manages and networks up to three tiers of workers
  - primary - active foreground workers, reading/writing/tool-using (possibly through virtual filesystems, sandboxes)
  - secondary - judge/review/synthesis workers (i.e., facilitates work across 1+ primary workers)
  - tertiary - observability/memory/watchdog/evaluation pre-processing
- coordinator nodes route traffic in per-turn in real-time: optimizing paths based host/network/job conditions; possibly handles asynchronous processes if it doesn't make sense to run them in real time with the primary nodes
- asynchronous post-process, ambient self-development: scheduled work via deterministic processes or local inference (possibly coordinated by a frontier model) processes observability, evaluation, and memory data; performs administrative work to track experiments; prune or update memories/etc.

## Relation to existing jcode proposals

(possibly stale) this proposal ties together several ongoing threads:

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

## worker roles

### 1. human-agent interaction

Responsible for managing interaction between the human operator and the rest of the system. Local runtime or p2p connection with fastest available runtime if local host is not suitable. Lightweight operations ok, but shells out heavier work. SoTA models best suited for this work.

### 2. coordinator

Continuously optimizes the distribution of work across the mesh, routing processes to the places that the process itself and real-time conditions are the best match (including network latency, and speed)

It can inspect sessions, spawn work, resume interrupted runs,
route tasks to capable machines, and decide whether/when/what context should be served to a session. May transfer information asynchronously to optimize speed of foreground processes.

### 3. Fabric: asynchronous consolidation

The fabric is the offline, compute-heavy layer. It turns raw experience into
structured memories, tutorials, lessons, and routing hints. Light capture, pre-process, retrieval,while a session is running. Process and post-processing during off hours, not in the critical token-stream path.

### 4. Steering agents: specialized co-clients

Steering agents observe the interaction surface and secondary/tertiary workers. Each agent should have a narrow lens and explicit permissions to steer both tiers via timely interventions.

Candidate steering agents:

- Context steering: suggests relevant memories, proposal docs, tasks, and prior
  decisions.
- Resource steering: recommends remote machines or background workers based on
  host capabilities and current workload.
- Safety steering: flags destructive commands, unrelated dirty files, hidden
  approvals, or actions likely to hang.
- Continuity steering: updates live task state and prepares handoff summaries.

# Why "latent outcomes"?

Agents offer a value proposition that hasn't been present in other forms of automation; they can dynamically adapt or adjust in response to a given scenario, state, set of conditions. 

Measuring those outcomes is partly subjective and confounded by task difficulty, amidst many other variables: 

Therefore, the measurement system is part of the product:

- Capture structured session outcomes: problem, approach, commands, files,
  result, tests, commits, and user corrections.
- Score artifacts with confidence and provenance.
- Treat LLM-as-judge outputs as hypotheses, not truth.
- Even human labels aren't guaranteed to be entirely accurate.
- A well constructed test measures some things well, but functional tests can't measure everything.
- "Lessons" developed out of observed successful patterns/routines/actions against an inferred task type or "class", can be referenced for guidance
- Re-check lessons against later sessions before promoting them.
- Measure prompt-token cost and latency for every injected context item.
- Prefer replayable fixture tests where possible.

A useful first success criterion: can replay a small corpus of prior-session situations and demonstrate that the
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

Relevance routing is a central research problem. Consolidation is valuable only
when the right artifact reaches the right session at the right moment.

Candidate relevance signals:

- Current user request, active todos, and recent tool calls.
- Working directory, repository, branch, dirty files, and modified paths.
- Backlog task IDs, proposal docs, and milestone labels.
- Session history, prior errors, and previous successful approaches.
- Operator-facing conversations with the executive surface.
- Host capabilities, available models, network location, and resource pressure.

Routing should be budget-aware/capacity/capability aware. More memory is not always better. Each injected item should have a reason, source, confidence, and token cost.

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
