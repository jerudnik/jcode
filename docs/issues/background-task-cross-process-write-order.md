---
title: "Background task status writes can lose terminal state across processes"
status: open
priority: high
owner: maintainers
opened: 2026-08-28
related:
  - crates/jcode-base/src/background/store.rs
  - docs/architecture/GOVERNANCE_DECISIONS.md
---

# Background task status writes can lose terminal state across processes

`TaskStatusStore` makes each JSON replacement atomic and serializes
read-modify-write cycles through a path-keyed mutex. That mutex is process-wide,
not cross-process. Two processes sharing the same status directory can read the
same running state, then write in the wrong order: a terminal writer can finish
first and a stale non-terminal writer can replace its result.

## Required work

- Add cross-process ordering or conditional writes for each task status file.
- Preserve terminal precedence while still merging delivery flags and event
  history.
- Add a real multi-process test that fails when a stale non-terminal writer can
  replace terminal state.
- Keep the existing atomic-rename, fsync, malformed-file recovery, and surfaced
  error guarantees.

## Acceptance

A deterministic multi-process test proves that once terminal state is visible,
no stale writer from another process can restore `Running` or replace the
terminal fields.
