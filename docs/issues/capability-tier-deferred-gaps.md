---
title: "Capability tier follow-ups deferred from the initial enforcement layer"
status: open
priority: medium
owner: maintainers
opened: 2026-08-30
related:
  - crates/jcode-app-core/src/tool/capability_tier.rs
  - docs/issues/scheduled-wake-and-background-resume-broken.md
---

# Capability tier deferred gaps

PR for `automation/w3-capability-tiers-impl` added server-derived capability
tiers: the server maps a task-graph node kind to a tier
(`crates/jcode-app-core/src/tool/capability_tier.rs`), installs it at
assignment, and denies tool calls above the tier before execution. Layers
only deny, never grant, and unknown tools fail closed. These gaps were
deliberately deferred:

## 1. Verify-tier shell is not sandboxed

Verify-tier workers may run shell commands so they can execute tests, but
nothing constrains those commands to be read-only. A verify worker can
mutate the repo through `bash`. Closing this needs command classification
or an OS sandbox, both out of scope for the first layer.

## 2. No approval flow for gate-origin escalation

When a gate (critique/verify) node concludes a fix is needed, the worker
cannot request a temporary tier raise. Today the coordinator must inject a
fix node so a new worker gets the higher tier. An explicit
escalate-with-approval flow may be worth having once real usage shows the
inject-node path is too slow.

## 3. Resume/wake paths do not install tiers

Tiers install at task assignment (`assign_task`/`assign_next`/spawn-driven
assignment). A worker resumed or woken outside those paths
(`swarm resume`, `swarm wake`, scheduled wakeups) keeps whatever tier it
had, or none for pre-tier sessions. Related to the broken wake/resume
provider seam (`scheduled-wake-and-background-resume-broken.md`); fix the
seam first, then route tier install through it.
