---
title: "Capability tier follow-ups deferred from the initial enforcement layer"
status: open
priority: medium
owner: maintainers
opened: 2026-08-30
related:
  - crates/jcode-app-core/src/tool/grant.rs
  - docs/issues/scheduled-wake-and-background-resume-broken.md
---

# Capability tier deferred gaps

PR for `automation/w3-capability-tiers-impl` added server-derived capability
tiers: the server maps a task-graph node kind to a tier
(`crates/jcode-app-core/src/tool/grant.rs`), installs it at
assignment, and denies tool calls above the tier before execution. Layers
only deny, never grant, and unknown tools fail closed. These gaps were
deliberately deferred:

## 1. Verify-tier shell is not sandboxed (RETIRED 2026-08-31)

Verify-tier workers may run shell commands so they can execute tests, and
nothing constrains those commands to be read-only. Retired: command
classification or an OS sandbox is large machinery for modest gain over
the existing directory scoping and tool-level denial. Accepted loss: a
verify worker can mutate its assigned directory through `bash`. Revisit
only if an incident shows this path being abused.

## 2. No approval flow for gate-origin escalation (RETIRED 2026-08-31)

A gate worker that concludes a fix is needed cannot request a tier raise;
the coordinator injects a fix node so a new worker gets the higher tier.
Retired: no usage evidence shows the inject-node path is too slow, and an
approval flow would break the invariant that a live session's tier never
rises. Accepted loss: gate-to-fix handoff costs one node injection.

## 3. Wake/resume outside an assignment carries no tier

Tiers now bind at assignment time for every assignee, headed or headless,
and persist until the assignment ends (terminal node, reassignment,
reclaim, member removal, or disconnect). A worker woken or resumed while
it still holds its assignment therefore keeps the correct binding.

What remains: a session revived through `swarm resume`/`swarm wake` after
its assignment was reclaimed, or one that never passed through an assign
path, runs unbound. That is the pre-tier default (full authority), so the
exposure is a stale worker acting outside any node, not a wrong tier for a
held node. Related to the broken wake/resume provider seam
(`scheduled-wake-and-background-resume-broken.md`); fix the seam first,
then decide whether revival without an assignment should re-run task
selection (and tier install) instead of resuming bare.
