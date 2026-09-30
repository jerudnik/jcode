---
title: "Cancelling a bash tool call leaves its cargo child alive and holding the build lock"
status: open
priority: medium
owner: unassigned
opened: 2026-09-21
related:
  - docs/issues/workspace-test-serialization.md
  - crates/jcode-app-core/src/tool/bash.rs
---

# Cancelling a bash tool call leaves its cargo child alive and holding the build lock

When a `bash` tool call hits its timeout or the turn is cancelled, the wrapper
shell is killed but the `cargo` process it started keeps running. In a
worktree that is the whole build: the orphan holds `target/debug/.cargo-lock`
and every later build in that worktree queues behind it.

## Evidence (2026-09-21, build ffa646a60)

- Worktree `jcode-port-settings-write`: a worker's `cargo test -p jcode-base`
  (PID 88504) survived its parent's timeout and kept compiling for 59
  minutes. `lsof` showed it holding `target/debug/.cargo-lock`. The
  coordinator's own test run in that worktree (started 09:26) sat on the lock
  until 10:00.
- Worktree `jcode-port-openai-continuation`: the worker reported "the
  cancelled wrapper left its child cargo alive (PID 89233, cwd this worktree,
  compiling aws-lc-sys via child 89432). The no-timeout replacement is queued
  on that build lock."

Four cold `cargo test` builds ran in parallel across four fresh worktrees, so
the machine sat at load 20 to 30 for the hour, which is why the first runs
overran their timeouts in the first place.

## Desired behavior

Tool cancellation kills the whole process group (or session) it spawned, not
just the immediate shell, so a cancelled `cargo` releases its lock. If the
tool deliberately leaves work running, it should say so in its result and
report the surviving PID.

## Workarounds used

Run long compiles with `run_in_background=true` and `bg wait` instead of a
timeout, and let an orphan finish rather than kill it, since its output is
the warm cache the next run needs.
