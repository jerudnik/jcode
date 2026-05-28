---
id: TASK-48
title: Resolve memory architecture design TBDs
status: To Do
assignee: []
created_date: '2026-05-28 05:04'
updated_date: '2026-05-28 12:20'
labels:
  - architecture
  - docs
  - memory
  - design
dependencies: []
references:
  - 'docs/MEMORY_ARCHITECTURE.md:802@0aea41ac'
  - 'docs/MEMORY_ARCHITECTURE.md:850@0aea41ac'
  - 'commit:0aea41ac'
  - 'Serena memory: compaction/dcp_research_task27'
priority: medium
ordinal: 42000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Difficulty: M

Resolve the two TBD sections in MEMORY_ARCHITECTURE.md (Status: TODO - Design pending; Architecture Options TBD).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Design pending section has a chosen design with rationale
- [ ] #2 Architecture options section has a selected option and trade-offs
- [ ] #3 Updates merged into the doc
- [ ] #4 Memory architecture explicitly covers canonical transcript versus provider-visible context projection, including handles for summaries, skeletons, placeholders, and protected facts.
- [ ] #5 Chosen design addresses trust tiers/quarantine, just-in-time context selection, graph/block-level retrieval, and how TASK-27 compaction summaries feed durable memory with provenance.
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Expanded from TASK-27 research: memory design should account for context projections, protected facts, summary provenance, trust tiers, graph/block retrieval, and explicit selection rather than always-on memory injection.
<!-- SECTION:NOTES:END -->
