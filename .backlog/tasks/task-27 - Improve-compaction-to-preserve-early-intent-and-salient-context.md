---
id: TASK-27
title: Improve compaction to preserve early intent and salient context
status: To Do
assignee: []
created_date: '2026-05-28 00:41'
labels:
  - exploratory
  - compaction
  - context
dependencies: []
references:
  - crates/jcode-compaction-core/src/lib.rs
  - src
  - README.md
priority: high
ordinal: 21000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Explore compaction strategies that preserve important earlier conversation context rather than relying primarily on recency, so user intent, constraints, decisions, and durable facts survive long sessions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Existing compaction triggers, cutoffs, summaries, and emergency behavior are reviewed against examples where early intent can be lost.
- [ ] #2 Candidate salience signals are evaluated, such as explicit user goals, decisions, constraints, files changed, task state, tool outcomes, and recurring references.
- [ ] #3 A testable design is proposed with regression fixtures or metrics showing important earlier messages are retained or summarized accurately.
<!-- AC:END -->
