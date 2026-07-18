# Fork Sync Model

Last reviewed: 2026-07-18

See also:

- [`patch-ledger.md`](./patch-ledger.md)
- [`../architecture/FORK_SUSTAINABILITY_MODEL.md`](../architecture/FORK_SUSTAINABILITY_MODEL.md)
- [`../architecture/MCP_SERVER_REGISTRATION_GUARDRAILS.md`](../architecture/MCP_SERVER_REGISTRATION_GUARDRAILS.md)
- [`../SERVER_LIFECYCLE_INVARIANTS.md`](../SERVER_LIFECYCLE_INVARIANTS.md)

## Context

The fork (`jerudnik/jcode`) tracks upstream (`1jehuang/jcode`) but has diverged
substantially. At the locally available refs on 2026-07-18, known-good baseline
commit `41e86f3c9` carried 323 fork-only commits while `upstream/master` carried
246 upstream-only commits. The fork leads the swarm, DAG, and comm subsystem.

The W1/W2 control-plane event log, fold-derived DAG state, and
artifact-dataflow are fork-authored on top of upstream's initial swarm engine.
Upstream's active work on TUI, goals, providers, and discovery barely overlaps
the fork's current focus.

## Current mechanism

The implemented sync model is an automated three-rail rebase, not exact branch
identity and not cherry-pick-only curation. Every six hours,
`.github/workflows/sync.yml` attempts to:

1. fast-forward `vendor/upstream` to `upstream/master`;
2. rebase `distro/nix` onto that vendor rail;
3. rebase `main` onto the resulting distro rail.

Tracked `rerere` recordings replay known conflicts. A genuinely new conflict
stops the workflow and opens or updates a `sync-blocked` issue for human
resolution. Successful rewritten rails are pushed with force-with-lease and
validation is dispatched explicitly.

This automation provides continuous visibility and cheap replay where the
conflict is already understood. It does not make upstream authoritative over
fork-owned behavior. New upstream changes in those surfaces still require human
review, adaptation, and a recorded conflict decision before the rails advance.

## Fork-owned subsystems

Upstream changes in these areas require explicit human adjudication when the
automated rebase cannot preserve the fork's established behavior:

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

Removing upstream's channels in-fork is a permanent divergence. The shared
`rerere` cache makes recurring conflict resolution cheap, while new conflicts
still stop rather than silently choosing upstream or the fork.

The trade is bounded automation with human authority at novel seams. That keeps
routine upstream intake inexpensive without surrendering fork-owned subsystem
decisions.

## Upstreaming

Fork-owned changes that upstream would plausibly want may be offered as pull
requests. The channel removal is a good candidate because it matches upstream's
own `SWARM_TASK_GRAPH` section 8a roadmap.

Upstreaming is best-effort. The fork does not rely on upstream accepting these
changes before continuing to own and harden its subsystem surface.
