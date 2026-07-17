# N2 promotion-readiness handoff

Date: 2026-07-17

Validated candidate: `62b3946b63eac0a5082b52fed98087ccafc2160c`

Branch: `normalize/integration`

## Verdict

**READY FOR APPROVAL-GATED FAST-FORWARD.**

The fixed candidate passes the trusted N2 matrix, carries an exact matching
binary build identity, preserves all frozen quality baselines, has rollback
archives verified, and received independent architecture and operations PASS
reviews. This verdict establishes candidate quality and recoverability. It does
not authorize promotion or cleanup actions.

## Fixed-head validation

The accepted evidence is in
`evidence/2026-07-16-n2-readiness/accepted/`.

- 54/54 manifest gates produced the expected exit code.
- All 55 original package checksums verify.
- Every raw log is stamped with candidate HEAD `62b3946b6` and the integration
  checkout path.
- `jcode --version` reports `jcode v0.46.0-dev (62b3946b6)`.
- `binary_hash_exact` passes, closing the stale development-build metadata gap.
- Recovery product-tree equivalence passes.
- Actual and expected 15-file product-delta lists are identical.
- All 14 focused R04 lifecycle fixtures pass.
- R12: 11 passed.
- W7 provenance: 3 passed.
- app-core library: 1,101 passed, 23 ignored.
- base library: 1,169 passed, 3 ignored.
- storage library: 10 passed.
- TUI and workspace compilation pass.
- Workspace library clippy passes with `-D warnings`.
- Binary build and protocol checks pass.

Original package hashes:

- `manifest.tsv`:
  `2f0752a6218ec93c563a94f5821cef13fa942fda8ad7231cf3f16b23e4c3b091`
- `SHA256SUMS`:
  `64ad9d6d7f6307ee9f163ec0f9c54119e3db0ef02214f0494f11f0cdd2e8b48e`

## Frozen R09 debt

No baseline file was updated. All four baseline JSON files are byte-identical
to `refs/archive/recovery/2026-07-15`.

The exact expected-red state remains:

- panic-prone usage: `31 -> 48`
- swallowed-error-like usage: `2987 -> 3074`
- production-size detector: expected red with exact raw output preserved
- test-size detector: expected red with exact raw output preserved

The matrix contains a guard proving it does not invoke the baseline update mode.
Ownership, no-growth policy, and reopen conditions remain in `QUALITY_DEBT.md`.

## Candidate decisions

- W7a typed error-chain semantics: complete.
- W7b deterministic interrupted-status mapping: complete.
- W7c evidence-helper extraction: complete.
- W7d bounded UTF-8-safe provenance retention: complete.
- R03A verdict centralization: closed as unwarranted with reopen triggers.
- R02 blanket file splitting: closed as unwarranted with reopen triggers.
- App-core churn fixture: corrected to measure unique lost worker sessions, not
  provider retry attempts.
- Two warning-only dead items: removed.
- Swallowed-error count: genuinely restored to 3,074 by keeping comments neutral
  to the repository's simple detector.

## Independent reviews

Two independent Opus review lanes evaluated the fixed product and exact
identity-enforced candidate:

- `reviews/2026-07-17-n2-architecture-62b3946b6-pass.md`: PASS, zero open
  CRITICAL, IMPORTANT, or MINOR architecture findings.
- `reviews/2026-07-17-n2-operations-62b3946b6-pass.md`: PASS, prior stale-binary
  IMPORTANT resolved, zero open CRITICAL or IMPORTANT findings.

The operations review noted that the two full-TUI diagnostics were not yet
bundled at review time. They are now preserved under `accepted/diagnostics/`.
They are explicitly non-gating because Phase 6's accepted TUI gate is affected
compilation and the N2 product delta contains no `jcode-tui` source.

## Rollback and host state

Rollback bundles were rehashed and verified:

- `jcode-stashes.bundle`:
  `f102fa69511ea31f261442b5e68e8621a02d2c88f884c34acbf47e502dcb7583`
- `jcode-all-refs.bundle`:
  `a29110621aa37b9ad142850305d6b72f5fd7ff9c84d9ba046a4eb4ec74254b82`

Both pass `git bundle verify`. Recovery and stash archive refs remain present.
Four stashes and 30 worktrees remain intentionally preserved.

Local `main` is `6ca1fcf2ec2366c7abc99664a485c40d60cec80e` and is an ancestor of the
candidate, so a fast-forward is possible. No remote contains the candidate.

The canonical checkout `/Users/jrudnik/labs/jcode` remains isolated on
`recovery/2026-07-15` at `8a81c60b25b2da911d4493b14d91b48002468549`
with the user's unrelated state untouched:

- modified `docs/fork/recovery/ORCHESTRATOR_PROMPT.md`
- untracked `opencode.json`

## Approval gates

None of the following actions are authorized by this readiness verdict:

1. move or fast-forward `main`
2. push the candidate or `main` to a remote
3. repoint a runtime binary, symlink, service, or launcher
4. delete recovery or archive refs
5. drop stashes
6. remove worktrees or prune worktree metadata
7. remove rollback bundles or other recovery artifacts

When approval is granted, the operator must first recheck that local `main`, the
candidate branch, rollback hashes, canonical dirty state, and remote refs have
not drifted. The canonical checkout must not be switched or reset over its
unrelated local changes.

## Remaining normal debt and cautions

- R09 remains expected-red normal quality debt. It is not promotion laundering.
- Full `jcode-tui --lib` diagnostics report existing host/config-sensitive
  fixture failures on unchanged TUI product code. Their raw outputs are retained.
- Git bundle object identity uses the repository's SHA-1 object format; separate
  SHA-256 hashes protect the bundle files themselves.
- Promotion and later recovery cleanup are separate decisions. Fast-forwarding
  does not imply permission to erase recovery state.
