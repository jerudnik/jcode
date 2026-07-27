# Fork Sync Model

Last reviewed: 2026-07-27

See also:

- [`../BRANCHING.md`](../BRANCHING.md) — the rail model and the `fork-point` tag
- [`patch-ledger.md`](./patch-ledger.md)
- [`../architecture/FORK_SUSTAINABILITY_MODEL.md`](../architecture/FORK_SUSTAINABILITY_MODEL.md)

## Current model: hard fork

**There is no sync.** As of 2026-07-27 this is a hard fork of `1jehuang/jcode`.
No branch tracks upstream, no scheduled job rebases onto it, and no rail is
expected to converge with it. The divergence point is frozen as the immutable
`fork-point` tag (`631935dd1d`).

`upstream` stays configured as a read-only reference remote so individual fixes
can still be read and cherry-picked. See "Taking a specific fix from upstream"
in [`../BRANCHING.md`](../BRANCHING.md).

## What this replaced

The previous model was an automated three-rail rebase: every six hours
`sync.yml` fast-forwarded `vendor/upstream` to `upstream/master`, rebased
`distro/nix` onto it, then rebased `main`, with a tracked `rerere` cache
replaying known conflicts and a `sync-blocked` issue opened for novel ones.

It was retired on measured evidence, not fatigue:

| Signal | Value at retirement |
|---|---|
| `sync.yml` runs succeeded | 1 of last 30 (last success July 4) |
| Time blocked on a one-line `release.yml` conflict | ~23 days |
| Failure alerting | also broken (`Resource not accessible by integration`), so it failed silently |
| Cost of one sync | 247 conflicted files, 651 hunks, 387 semantic Rust |
| `.rerere-cache` size | 202k lines, 60% of all fork-new files |
| Upstream commits touching only files we never modified | 52 of 678 |
| Of those, cherry-picking cleanly | 20 |

The automation had already stopped delivering: it was failing silently, and its
accounting cost (the rerere cache) had grown larger than the code it protected.
Of the 20 cleanly-applying commits, 8 were judged worth taking and were
harvested before the remote was demoted; the rest were desktop2 scaffolding,
Windows/release plumbing owned by `distro/nix`, upstream's own telemetry
worker, or an incoherent half-refactor whose other half lived in a
fork-modified file.

## Fork-owned subsystems

These diverged deliberately and permanently. Upstream's versions are not a
target to converge on:

- swarm and comm; the W1/W2 control-plane event log, fold-derived DAG state,
  and artifact dataflow are fork-authored on top of upstream's initial engine
- channel and shared-context removal (removed in-fork ahead of and independent
  of upstream)
- `mcp-serve`, a fork addition; see
  [`../architecture/MCP_SERVER_REGISTRATION_GUARDRAILS.md`](../architecture/MCP_SERVER_REGISTRATION_GUARDRAILS.md)
- supervision and lifecycle hardening; see
  [`../SERVER_LIFECYCLE_INVARIANTS.md`](../SERVER_LIFECYCLE_INVARIANTS.md)
- ambient storage roots, routed through `jcode-storage`
- telemetry consent and destination; see [`../TELEMETRY.md`](../TELEMETRY.md)

## Quality asymmetry

Upstream code generally does not satisfy this fork's gates: the warning budget,
swallowed-error and panic ratchets, code-size ceilings, dependency boundaries,
and the config env lease. Any harvested commit must be brought up to standard,
never accommodated by raising a budget. The eight commits harvested at fork
declaration needed three such fixes between them.

This asymmetry is why the fork-touched clippy and rustfmt gates exist: lints are
blocking in files this fork owns and advisory in untouched upstream files, with
the boundary computed against `fork-point`.

## Upstreaming

Fork-owned changes that upstream would plausibly want may still be offered as
pull requests, best-effort. The fork does not wait on upstream acceptance before
continuing to own and harden its surface.
