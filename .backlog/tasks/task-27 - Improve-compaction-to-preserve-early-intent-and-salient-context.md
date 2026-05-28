---
id: TASK-27
title: Improve compaction to preserve early intent and salient context
status: Done
assignee:
  - '@jcode'
created_date: '2026-05-28 00:41'
updated_date: '2026-05-28 12:18'
labels:
  - exploratory
  - compaction
  - context
  - reliability
  - context-preservation
  - session
  - salience
dependencies: []
references:
  - crates/jcode-compaction-core/src/lib.rs
  - src
  - README.md
  - >-
    .backlog/tasks/task-27 -
    Improve-compaction-to-preserve-early-intent-and-salient-context.md:28@0aea41ac
  - 'commit:0aea41ac'
  - 'https://github.com/Opencode-DCP/opencode-dynamic-context-pruning'
  - 'https://doi.org/10.48550/arxiv.2406.11927'
  - 'https://doi.org/10.1145/3643991.3644897'
  - 'https://doi.org/10.48550/arxiv.2410.18251'
  - >-
    https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents
  - 'https://www.langchain.com/blog/context-engineering-for-agents'
  - 'https://developers.openai.com/cookbook/examples/agents_sdk/session_memory'
  - 'https://aider.chat/2023/10/22/repomap.html'
priority: high
ordinal: 21000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Explore compaction strategies that preserve important earlier conversation context rather than relying primarily on recency, so user intent, constraints, decisions, and durable facts survive long sessions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Existing compaction triggers, cutoffs, summaries, and emergency behavior are reviewed against examples where early intent can be lost.
- [x] #2 Candidate salience signals are evaluated, such as explicit user goals, decisions, constraints, files changed, task state, tool outcomes, and recurring references.
- [x] #3 A testable design is proposed with regression fixtures or metrics showing important earlier messages are retained or summarized accurately.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Dispatch parallel research agents over DCP documentation, source code, and public discussions/issues.
2. Inspect JCODE compaction/context-memory surfaces enough to map external ideas to native integration points.
3. Synthesize candidate native designs, risks, and test fixtures against TASK-27/TASK-1/TASK-34/TASK-48/TASK-62/TASK-63.
4. Persist findings in a Serena MCP memory for future implementation work.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Started DCP comparative research and added the upstream DCP repository as a reference.

Dispatched four subagents covering DCP documentation, DCP code, public issues/discussions/releases, and local JCODE compaction integration points.

Wrote consolidated Serena memory `compaction/dcp_research_task27` with DCP design findings, JCODE opportunities, failure-mode cautions, salience signals, regression fixtures, and phased implementation recommendations.

Tracked follow-up Rust-native context-intake suggestions in Serena memory `compaction/dcp_research_task27`, emphasizing low-overhead boundary inspection, structured placeholders, skeleton fallback, lazy repo maps, provider-aware token estimates, and ledger metrics.

Tracked final research-backed context-management suggestions in Serena memory `compaction/dcp_research_task27`, retaining deterministic graph/retrieval/pruning ideas and deferring heavyweight ML/model-architecture approaches.

Ran an agent-toolbox/web exploratory pass over Anthropic, LangChain, OpenAI cookbook, Aider repo-map, and search results. Tracked additional low-risk context-management candidates in Serena memory `compaction/dcp_research_task27`.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Completed exploratory DCP research for TASK-27 and persisted the synthesis as Serena memory `compaction/dcp_research_task27`.

Findings:
- DCP’s strongest transferable idea is separating canonical session history from the provider-visible context projection.
- Native JCODE should evolve toward deterministic salience-aware context planning with protected facts, durable summary provenance, compaction ledgers, and debug visibility.
- High-value near-term work: strengthen summary/emergency prompts, preserve goals/constraints/ACs/tool outcomes, add early-intent regression fixtures, deduplicate repeated tool outputs, and prune stale errored tool inputs.
- DCP issue history warns against fragile model-facing boundary IDs, non-durable compaction metadata, opaque threshold triggers, provider-specific reasoning metadata leaks, and complex block lifecycle bugs.

Research sources included DCP docs, source structure, public issues/PRs/releases, and local JCODE compaction surfaces.

Follow-up context-intake suggestions were appended to Serena memory `compaction/dcp_research_task27`: boundary inspector for file reads/tool outputs, structured placeholders with restore handles, Rust/tree-sitter skeleton fallback for oversized source files, cached/lazy repo context map, provider-aware token estimation, and context-intake ledger metrics.

Additional research-backed ideas were appended to Serena memory `compaction/dcp_research_task27`: typed/code-aware retrieval before materialization, function/block-level graph retrieval, iterative retrieval-generation loops, deterministic entropy/repetition pruning for verbose logs, and call/reference graph edges as optional salience signals. Deferred model-architecture early exiting and heavy ML compressors/pruners from the near-term path.

Agent-toolbox exploratory search additions were appended to Serena memory `compaction/dcp_research_task27`: just-in-time context references, tool-output contracts with budgets/pagination, context quarantine/trust tiers, dynamic toolset/context selection, stable context ordering for prompt caching/debuggability, scratchpad/offloaded working state with explicit selection, and context-failure taxonomy labels. Marketing-only compression claims and heavyweight learned frameworks were deferred.
<!-- SECTION:FINAL_SUMMARY:END -->
