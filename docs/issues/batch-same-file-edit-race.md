---
title: Batching two edit calls against the same file silently drops the first edit
status: open
priority: high
owner: maintainers
opened: 2026-08-28
related:
  - crates/jcode-app-core/src/tool/batch.rs
---

# Batching two edit calls against the same file silently drops the first edit

Observed twice in one session (2026-08-27), on two different files. A `batch`
call carried two `edit` (find and replace) calls targeting the same file. Both
calls reported success, each echoing the change it believed it made. After the
batch, the file contained only the second edit. The first was gone.

The apparent mechanism is a read-modify-write race: both edits read the
original file content, each applied its own replacement to that stale copy,
and the second write clobbered the first. Nothing failed, so nothing surfaced.
The loss was caught only because a downstream checker happened to fail on the
missing change.

## Why this is worse than a crash

The tool's own success report is false. An agent that batches edits, sees two
successes, and moves on has no reason to re-read the file. The corruption
propagates into commits until some unrelated gate trips, and the diagnosis
cost lands far from the cause.

## What a fix should do

Either is acceptable; silent last-write-wins is not:

- Serialize batched calls that target the same file path, so each edit sees
  the previous one's output.
- Reject the batch up front when two mutating calls name the same file, the
  way batch already rejects batching the `batch` tool itself.

Rejection is the smaller change and turns a silent corruption into a loud,
immediate error. Serialization is friendlier but must define ordering.

## Reproduction sketch

Issue one `batch` call containing two `edit` calls against one file, each
replacing a different unique string. Observe both report success while the
file afterward contains only the second replacement.
