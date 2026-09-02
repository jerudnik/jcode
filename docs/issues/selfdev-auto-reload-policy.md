---
title: "Self-dev auto reload policy (deterministic build-reload on graph node completion)"
status: open
priority: medium
owner: maintainers
opened: 2026-09-02
related:
  - crates/jcode-app-core/src/tool/selfdev/reload.rs
  - crates/jcode-build-support/src/paths.rs
  - docs/issues/scheduled-wake-and-background-resume-broken.md
---

# Self-dev auto reload policy (deterministic build-reload on graph node completion)

## Problem

Reloading onto a freshly built binary during long self-dev campaigns is currently a
model-driven action: the coordinator decides to run `selfdev build-reload`, which makes it
(a) easy to forget at natural boundaries, (b) non-deterministic in timing, and (c) coupled
to model judgment rather than a policy. We want completion of an appropriate work unit to
trigger a rebuild + reload that is **anticipated** (no in-flight work lost), **deterministic**
(enforced in code, not a polite request to a model), **safe** (automatic rollback on failure),
and **self-healing** (work continues seamlessly on the new binary).

## Desired behavior

A reload policy that can be declared per graph node (or per session) and is enforced by the
harness:

- Node completion with a reload policy queues a `selfdev build-reload` **after quiescing**:
  no in-flight workers share the target worktree; the existing
  `~/.jcode/selfdev-build-locks/<scope>.lock` flock serializes concurrent builds.
- Build failure → old binary keeps running (already true today); no reload attempted.
- Post-reload smoke check (reuse `smoke_test_binary` in `jcode-build-support`) → on failure,
  restore the stashed previous dev binary and reload back.
- After reload, session state is resumed from the durable on-disk store (the W23 durable
  session/inbox work is the substrate), so the coordinator and workers re-enter without
  losing graph state.

## Open questions (this issue needs its own planning session)

1. **Configurability / encoding.** Should the reload trigger be:
   - a per-node flag in the task graph (`on_complete: "reload"`), resolved by the
     task-graph runner?
   - a session-level switch ("reload at every milestone")?
   - an instruction the coordinator model writes into the graph at plan time, with the
     harness merely enforcing quiesce + lock semantics?
   Likely some layering of all three; the plan should pick which layer owns the decision.
2. **Quiesce definition.** Is "no running worker in the worktree scope" sufficient, or do we
   also need to wait for queued-but-unassigned nodes, pending DMs, or unacked inbox
   deliveries?
3. **Rollback scope.** Binary-only rollback (stash/restore dev binary + reload) vs. also
   reverting graph state if a checkpoint was advanced by the new binary.
4. **Model instructions.** If any part remains model-driven (e.g. choosing *which* node gets
   the flag), what guidance goes into the swarm prompt / planning docs so flags are set at
   natural boundaries (merged chunks, seams) rather than arbitrarily?
5. **Interaction with remote builds.** Builds already run remotely via
   `scripts/dev_cargo.sh` (`JCODE_REMOTE_CARGO=1`, fallback=error). Reload must handle the
   remote-artifact sync step and the `FALLBACK=error` failure mode explicitly.

## Key code touchpoints

- `crates/jcode-build-support/src/paths.rs` — `selfdev_build_command_for_target`
- `crates/jcode-app-core/src/tool/selfdev/build_queue.rs`, `reload.rs` — build/queue/reload
- `crates/jcode-app-core/src/tool/selfdev/mod.rs` — build lock (flock) plumbing
- `crates/jcode-swarm-core` — task-graph node execution (flag handling)
- W23 durable session store (`crates/jcode-base/src/inbox/`, `jcode-storage/src/artifacts.rs`)
  for post-reload state resume

## Suggested staging

1. Flag semantics + quiesce + smoke-check rollback (selfdev crate only, no swarm changes).
2. Task-graph wiring (`on_complete` flag) + post-reload session resume.
3. Docs: when to set the flag (chunk boundaries, before coordination-heavy chunks).
