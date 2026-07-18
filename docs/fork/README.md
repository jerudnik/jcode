# Fork documentation index

Last audited: 2026-07-18

This directory mixes current operating guidance with immutable recovery and
normalization evidence. Use the categories below instead of treating every
dated status statement as current.

## Current operating state

- [`normalization/STATUS.md`](normalization/STATUS.md): current source, runtime,
  cleanup, and completion boundary.
- [`normalization/KNOWN_GOOD_BASELINE.md`](normalization/KNOWN_GOOD_BASELINE.md):
  exact core-runtime baseline, fixes completed in commit `41e86f3c9`, and ranked
  remaining lifecycle seams.
- [`patch-ledger.md`](patch-ledger.md): downstream patches, ownership, retirement
  conditions, and validation commands.
- [`SYNC_MODEL.md`](SYNC_MODEL.md): monitored-curation policy for upstream work.
- [`SECURITY_TRIAGE.md`](SECURITY_TRIAGE.md): fork-only security-triage policy and
  any advisory rows not already documented by upstream.

The canonical checkout has one worktree. The promoted `current`, `stable`, and
`shared-server` channels remain on immutable `8962bccb3-release`. Recovery refs,
four stashes, rollback bundles, sealed evidence, and private archives remain
preserved.

## Normative standards and runbooks

- [`normalization/COMPLETION_STANDARD.md`](normalization/COMPLETION_STANDARD.md):
  the binary definition of fully normalized versus honestly core-runtime
  validated.
- [`normalization/RUNTIME_AND_NIX_RUNBOOK.md`](normalization/RUNTIME_AND_NIX_RUNBOOK.md):
  build, immutable-channel, handoff, verification, and rollback procedure.
- [`normalization/QUALITY_DEBT.md`](normalization/QUALITY_DEBT.md): no-growth
  policy and its dated N2 measurement.

## Historical records

- [`recovery/README.md`](recovery/README.md): archived six-phase recovery record.
- [`normalization/BASELINE.md`](normalization/BASELINE.md): append-only
  pre-normalization host and repository snapshots.
- [`normalization/N2_SIGNOFF.md`](normalization/N2_SIGNOFF.md): fixed-candidate
  promotion-readiness evidence before promotion and cleanup.
- `recovery/evidence/`, `recovery/reviews/`, `recovery/seams/`,
  `normalization/evidence/`, and `normalization/reviews/`: dated evidence and
  independent reviews. Old counts and pending states remain valid historical
  observations unless a current page explicitly adopts them.

The retained [`recovery/ORCHESTRATOR_PROMPT.md`](recovery/ORCHESTRATOR_PROMPT.md)
is a historical launch artifact at its tracked baseline. Do not reuse it as a
current task prompt without revalidation.
