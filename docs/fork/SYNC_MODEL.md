# Fork Sync Model

See also:

- [`patch-ledger.md`](./patch-ledger.md)
- [`../architecture/FORK_SUSTAINABILITY_MODEL.md`](../architecture/FORK_SUSTAINABILITY_MODEL.md)
- [`../architecture/MCP_SERVER_REGISTRATION_GUARDRAILS.md`](../architecture/MCP_SERVER_REGISTRATION_GUARDRAILS.md)
- [`../SERVER_LIFECYCLE_INVARIANTS.md`](../SERVER_LIFECYCLE_INVARIANTS.md)

## Context

The fork (`jerudnik/jcode`) tracks upstream (`1jehuang/jcode`) but has diverged
substantially. It carries 190+ fork-only commits and now leads the swarm, DAG,
and comm subsystem.

The W1/W2 control-plane event log, fold-derived DAG state, and
artifact-dataflow are fork-authored on top of upstream's initial swarm engine.
Upstream's active work on TUI, goals, providers, and discovery barely overlaps
the fork's current focus.

## The shift

The sync model is monitored curation, not exact tracking.

The old model tries to auto-rebase everything: a 6-hour CI rebase, shared
`rerere`, and automatic replay across all upstream changes. That works while the
fork is a small additive patch stack, but it becomes expensive once the fork
owns major behavior that upstream is also reshaping.

The current model keeps `vendor/upstream` for visibility, reviews upstream
commits, and cherry-picks or adapts the changes worth taking. Provider updates,
TUI improvements, and bug fixes remain good candidates. Fork-owned subsystems
are curated manually.

## Fork-owned subsystems

Upstream changes in these areas are manually rewritten to fit the fork. They are
not merged mechanically:

- swarm and comm
- channel and shared-context removal
- `mcp-serve`
- supervision and lifecycle hardening

Channels and shared context are already removed in this fork ahead of, and
independent of, upstream. That removal is part of the fork's control-plane
direction rather than a temporary sync artifact.

`mcp-serve` is a fork addition. Its daemon-side safety contract is documented in
[`../architecture/MCP_SERVER_REGISTRATION_GUARDRAILS.md`](../architecture/MCP_SERVER_REGISTRATION_GUARDRAILS.md).

Supervision and lifecycle hardening are fork-owned operational guardrails. Their
invariants are documented in
[`../SERVER_LIFECYCLE_INVARIANTS.md`](../SERVER_LIFECYCLE_INVARIANTS.md).

## Why

Removing upstream's channels in-fork is a permanent divergence. Under
exact-tracking, that decision fights every rebase. Under curation, it is simply
fork-owned surface.

The trade is free auto-updates for control and no thrash. That is the correct
trade once the fork leads a subsystem.

## Upstreaming

Fork-owned changes that upstream would plausibly want may be offered as pull
requests. The channel removal is a good candidate because it matches upstream's
own `SWARM_TASK_GRAPH` section 8a roadmap.

Upstreaming is best-effort. The fork does not rely on upstream accepting these
changes before continuing to own and harden its subsystem surface.
