# Fork documentation index

Last audited: 2026-07-18

## Active authority

The active engineering authority is the
[`ideal-base/`](ideal-base/) control plane. It carries the accepted starting
boundary forward from completed normalization and defines the graph-first path to
an honest ideal durable TUI/CLI foundation.

Start here:

- [`ideal-base/README.md`](ideal-base/README.md): authority order, archive boundary,
  and operating entrypoint.
- [`ideal-base/COORDINATOR_BOOTSTRAP.md`](ideal-base/COORDINATOR_BOOTSTRAP.md):
  copy-paste prompt for a completely fresh coordinator session.
- [`ideal-base/WORK_GRAPH.json`](ideal-base/WORK_GRAPH.json): machine-readable deep
  graph with dependencies, ownership, gates, and evidence contracts.
- [`ideal-base/STATE.json`](ideal-base/STATE.json): durable cross-session node
  disposition.
- [`ideal-base/ACCEPTANCE_STANDARD.md`](ideal-base/ACCEPTANCE_STANDARD.md): binary
  exit gates and honest claim labels.
- [`ideal-base/AUDIT_COVERAGE.md`](ideal-base/AUDIT_COVERAGE.md): all 25 audited
  work items mapped to executable implementation and verification nodes.

The accepted starting label is **core-runtime validated**. The current immutable
runtime remains `8962bccb3-release`, selected by `current`, `stable`, and
`shared-server` when the railway was created. Recovery refs, four stashes,
rollback bundles, sealed evidence, and private archives remain preserved. Recheck
all live facts with `scripts/ideal_base_railway.py` before mutation.

## Continuing fork policy

These repository-wide fork policies remain active beside the ideal-base control
plane:

- [`patch-ledger.md`](patch-ledger.md): downstream patches, ownership, retirement
  conditions, and validation commands.
- [`SYNC_MODEL.md`](SYNC_MODEL.md): monitored-curation policy for upstream work.
- [`SECURITY_TRIAGE.md`](SECURITY_TRIAGE.md): fork-only security triage and advisory
  rows not already documented upstream.

## Frozen historical namespaces

[`normalization/`](normalization/) and [`recovery/`](recovery/) are archived in
place. Their current paths preserve relative links, checksum manifests, sealed
evidence, hash citations, review history, and seam ledgers. See
[`archive/README.md`](archive/README.md) for the boundary.

Do not refresh their old counts, pending states, or dated labels in place. Critical
starting facts and acceptance policy have been carried into `ideal-base/`.
Historical files remain valid evidence for their recorded moment, not active task
authority.

The retained
[`recovery/ORCHESTRATOR_PROMPT.md`](recovery/ORCHESTRATOR_PROMPT.md) is a protected
historical launch artifact at its tracked baseline. It must remain byte-identical
and must not be reused as a current prompt.
