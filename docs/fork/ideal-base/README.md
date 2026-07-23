# Ideal durable TUI/CLI foundation

Recorded: 2026-07-18

This directory is the active authority for moving Jcode from the current
core-runtime-validated baseline to an honest ideal TUI/CLI foundation. It is a
code-adjacent execution protocol, not a replacement for source code, tests, Git,
or live runtime observations.

## Authority order

When records disagree, use this order:

1. Current source, tests, reproducible commands, Git state, and runtime evidence.
2. [`BASELINE.md`](BASELINE.md) for the accepted starting boundary and protected assets.
3. [`ACCEPTANCE_STANDARD.md`](ACCEPTANCE_STANDARD.md) for the required exit gates.
4. [`WORK_GRAPH.json`](WORK_GRAPH.json) for workstream dependencies and node contracts.
5. [`STATE.json`](STATE.json) for durable cross-session node disposition.
6. [`DECISIONS.md`](DECISIONS.md) for append-only architecture and scope decisions.
7. Older recovery, normalization, proposal, and review documents as historical evidence.

A stale status label never overrides current source or a newer accepted artifact.

## Historical boundary

The following namespaces are frozen historical records and are archived in
place to preserve relative links, checksum manifests, citations, and forensic
value:

- [`../normalization/`](../normalization/)
- [`../recovery/`](../recovery/)

Do not refresh old counts or pending states in those trees. Do not edit evidence,
reviews, seam ledgers, or the protected
[`../recovery/ORCHESTRATOR_PROMPT.md`](../recovery/ORCHESTRATOR_PROMPT.md).
Its expected SHA-256 is recorded in [`BASELINE.md`](BASELINE.md) and enforced by
the railway validator.

## Active files

- [`BASELINE.md`](BASELINE.md): exact starting state, carried-forward facts, and protected boundaries.
- [`ACCEPTANCE_STANDARD.md`](ACCEPTANCE_STANDARD.md): deterministic and gated ideal-base exit criteria.
- [`AUDIT_COVERAGE.md`](AUDIT_COVERAGE.md): the complete 25-item source audit mapped to executable graph nodes.
- [`EXECUTION_PROTOCOL.md`](EXECUTION_PROTOCOL.md): graph-first delegation, file ownership, artifact, review, and recovery rules.
- [`COORDINATOR_BOOTSTRAP.md`](COORDINATOR_BOOTSTRAP.md): copy-paste prompt for a fresh coordinator session.
- [`WORK_GRAPH.json`](WORK_GRAPH.json): machine-readable waves, dependencies, owned paths, gates, and required evidence.
- [`STATE.json`](STATE.json): machine-readable cross-session progress checkpoint.
- [`DECISIONS.md`](DECISIONS.md): append-only decisions and reopen triggers.
- [`evidence/`](evidence/): bounded accepted evidence by node ID.
- [`reviews/`](reviews/): independent critique and verification reports.

## Start or resume

From the canonical checkout:

```bash
python3 scripts/ideal_base_railway.py check
python3 scripts/ideal_base_railway.py status
python3 scripts/ideal_base_railway.py next --json
```

Then read [`COORDINATOR_BOOTSTRAP.md`](COORDINATOR_BOOTSTRAP.md) and seed the
reported runnable wave through `swarm task_graph` in deep mode. The graph is the
execution scheduler. `STATE.json` is the durable restart checkpoint if the live
graph or session is lost.

## Scope and authorization

The deterministic foundation program may proceed without provider credentials or
network publication. Live provider requests, Apple signing/device work, release
publication, pushes, destructive archive cleanup, credential use, and unsupported
platform execution remain explicit authorization gates. Record them as `accepted`
or `authorization_blocked`; never imply they passed.

## Completion claim

The phrase **ideal TUI/CLI foundation** is permitted only when every mandatory
node is accepted, every deterministic gate in
[`ACCEPTANCE_STANDARD.md`](ACCEPTANCE_STANDARD.md) passes at one fixed commit,
gated nodes are honestly dispositioned, and an independent final reviewer reports
no unresolved blocker class.
