# Fork documentation index

Last audited: 2026-07-27

## Retired reference

The `ideal-base/` subtree has been retired. This index remains only as a surviving
fork reference and no longer names `ideal-base/README.md` or any `ideal-base/`
file as active authority.

If you need historical context, use the archived material under this directory and
the boundary notes in [`archive/README.md`](archive/README.md). Do not treat the
retired `ideal-base/` documents as a live starting point.

Do not restore deleted rebuildable fixtures or retired execution scripts into
`ideal-base/` to satisfy an old command. Active checks regenerate temporary
fixtures outside the archive when they need fixture-mode input.

Historical artifacts that remain relevant for provenance or audit trail are:

- [`ideal-base/COORDINATOR_BOOTSTRAP.md`](ideal-base/COORDINATOR_BOOTSTRAP.md):
  preserved bootstrap prompt for historical sessions.
- [`ideal-base/WORK_GRAPH.json`](ideal-base/WORK_GRAPH.json): preserved graph
  snapshot with dependencies, ownership, gates, and evidence contracts.
- [`ideal-base/STATE.json`](ideal-base/STATE.json): preserved cross-session node
  disposition snapshot.
- [`ideal-base/ACCEPTANCE_STANDARD.md`](ideal-base/ACCEPTANCE_STANDARD.md):
  preserved binary exit gates and claim labels.
- [`ideal-base/AUDIT_COVERAGE.md`](ideal-base/AUDIT_COVERAGE.md): preserved audit
  coverage map for the retired subtree.

## Hard-fork policy

The live repository and `main` are authoritative. There is no upstream sync,
tracking cadence, patch stack, or patch-ledger maintenance obligation.

- [`SECURITY_TRIAGE.md`](SECURITY_TRIAGE.md): fork-only security triage and advisory
  rows not already documented upstream.

## Frozen historical namespaces

[`normalization/`](normalization/) and [`recovery/`](recovery/) are archived in
place. Their current paths preserve relative links, checksum manifests, sealed
evidence, hash citations, review history, and seam ledgers. See
[`archive/README.md`](archive/README.md) for the boundary.

Do not refresh their old counts, pending states, or dated labels in place. Critical
starting facts and acceptance policy were recorded in `ideal-base/` before that
subtree was retired. Historical files remain valid evidence for their recorded
moment, not active task authority.

The retained
[`recovery/ORCHESTRATOR_PROMPT.md`](recovery/ORCHESTRATOR_PROMPT.md) is a protected
historical launch artifact at its tracked baseline. It must remain byte-identical
and must not be reused as a current prompt.
