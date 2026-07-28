# R07 integration adjudication — protected-path additions

Date: 2026-07-28. Decider: coordinator, per Stream G's deferred adjudication
(`stream-g-protected-paths-proposal.md`, itself answering the v4 gate's
non-blocking residual).

## Decision

Of the 21 proposed protected-path additions:

- **16 enforced** (moved to `protected_paths.required` in
  `scripts/required-checks.json`, `additions_adjudicated` set `true`,
  `proposed_additions` emptied): the ten `scripts/check_*` gate scripts,
  `scripts/security_preflight.sh`, `scripts/docs_impact_advisory.py`,
  `scripts/rust_production_filter.py`, `scripts/ambient_roots_allowlist.txt`,
  `tests/test_nix_distribution_policy.py`, and
  `tests/test_rust_production_filter.py`. These can make a required check
  vacuous without touching any previously protected path; a change to any of
  them now turns `Governance Root` red and forces the recorded maintenance
  procedure.
- **5 deliberately left unprotected** (the ratchet baselines:
  `scripts/code_size_budget.json`, `scripts/panic_budget.json`,
  `scripts/swallowed_error_budget.json`, `scripts/test_size_budget.json`,
  `scripts/warning_budget.txt`). Raising a baseline is the intended escape
  valve for those gates and happens routinely; the maintenance transaction is
  too heavy for a routine ratchet, and baseline raises are one-line,
  review-visible diffs. Stream G's own adjudication note recommended this
  split.

## Consistency changes (all three lists updated together)

- `scripts/required-checks.json` — comparator manifest (above).
- `docs/fork/ideal-base/evidence/R07/workflow-contexts.proposed.patch` —
  the same 16 paths added to `governance-root.yml`'s `protected` array; the
  new-file hunk header was corrected (`+1,45` → `+1,61`) after the insertion
  changed the body length. `git apply --check` passes; the patch was re-applied
  to a scratch tree and `governance-root.yml` verified to carry all 23 paths.
- `docs/fork/ideal-base/evidence/R07/github-governance.proposed.json` —
  `template_variables.protected_paths` extended with the same 16, so the
  sequence-6 current-main diff asserts on the full enforced set.
- `docs/fork/ideal-base/evidence/R07/fixtures/governance-valid.json` —
  regenerated via `scripts/generate_governance_fixture.py --workflows-dir`
  against the patched scratch tree.

## Test updates (premises changed by the migration/adjudication, mechanisms unchanged)

- `tests/test_governance_compare.py`: the two adjudication-polarity tests now
  construct their own pending/enforced scenario
  (`scripts/governance_compare.py` as the synthetic addition) instead of
  relying on repo state; both polarities still verified.
- `tests/test_ideal_base_railway.py`: `test_live_state_json_is_schema_v1...`
  replaced by `test_live_state_json_is_schema_v2_and_validates`, asserting the
  migrated live file validates under v2 rules against the real published ref.

## Coordinator STATE migration (same commit)

`docs/fork/ideal-base/STATE.json` migrated schema v1 → v2 by applying
`evidence/R07/STATE.proposed.json`: verified beforehand that every
`reviewed_commit` is a lossless 40-hex expansion of the live abbreviated
`commit` (0 non-prefix mismatches), state/evidence/summary/updated_at values
identical for all 57 records, and `last_checkpoint` differs only by the same
key split. Post-migration:
`python3 scripts/ideal_base_railway.py check --published-ref 498249777c453c1d551aeb01fc45420d8ca0a585`
→ "ideal-base railway OK: 7 roots, 50 child nodes, 57 state records,
protected hash intact".

## Validation

- `tests.test_governance_compare`: 73/73 pass.
- `tests.test_ideal_base_railway`: 25/25 pass.
- `git apply --check` on the workflow patch; real apply to scratch tree;
  actionlint clean on all patched workflows (see below).

## Honest scope note (per Stream G and the v4 gate)

This closes the enumerated hole, not the class: a sufficiently indirect
dependency can still route around any fixed list. The durable control remains
the required-context contract plus the comparator's vacuous-gate detection;
this list is defence in depth.
