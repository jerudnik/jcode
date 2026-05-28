---
id: TASK-8
title: Fix emergency compaction count drift and repeated marker loop
status: Done
assignee:
  - '@jcode-agent'
created_date: '2026-05-18 04:41'
updated_date: '2026-05-28 02:27'
labels:
  - upstream
  - owner-interest
dependencies: []
references:
  - 'https://github.com/1jehuang/jcode/issues/175'
priority: high
ordinal: 8000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Upstream #175 received strong owner endorsement. Prevent compacted_count from drifting past messages.len(), stop active_messages from replaying the full transcript on stale state, and prevent repeated emergency marker accumulation from wedging long sessions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Stale compacted_count is clamped or recovered without replaying the full transcript
- [x] #2 Repeated emergency compaction does not append duplicate markers indefinitely
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Self-improvement loop:
1. Inspect compaction state model and existing tests around compacted_count, active_messages, and emergency markers.
2. Build a targeted reproduction for stale compacted_count and duplicate emergency marker accumulation.
3. Implement the smallest recovery/deduplication fix.
4. Critically review invariants and upstream issue scope.
5. Run targeted regression tests and broader relevant tests.
6. Record final summary and commit.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Fixed the emergency compaction regression from upstream issue #175 by preserving compacted history invariants and preventing emergency marker accumulation.

Changes:
- Stale compacted_count states are covered by existing clamp/recovery tests that ensure the full transcript is not replayed after restore or new turns.
- Emergency summary generation now strips prior emergency marker paragraphs before appending the current marker, so repeated hard compaction keeps one marker instead of growing indefinitely.
- Strengthened hard-compaction regression coverage in both jcode-compaction-core and the manager tests.

Validation:
- cargo test -p jcode-compaction-core --lib
- cargo test -p jcode test_hard_compact_twice --lib
- cargo test -p jcode test_hard_compact_clamps_pathological_compacted_count --lib
- cargo test -p jcode test_invalid_compacted_count_does_not_resurrect_full_transcript_after_new_turn --lib
- selfdev build target=tui

Upstream reference preserved: https://github.com/1jehuang/jcode/issues/175
<!-- SECTION:FINAL_SUMMARY:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 Regression or validation added where applicable
- [x] #2 Upstream issue/PR reference preserved in final notes
<!-- DOD:END -->
