# Critical-path scope count refresh (provider_infrastructure 19 -> 20)

## What failed

`Quality Guardrails` / "Test critical-path budget checker" on PR #130:

    test_expected_counts_match_the_current_tree
      AssertionError: 'provider_infrastructure': 20 != 'provider_infrastructure': 19

Every other guardrail passed. The seven checkers `scripts/preflight.sh` runs all
exit 0 locally, which is why this reached CI at all: preflight does not run
`test_critical_path_budget.py`.

## Cause

D01-FIX-3 adds one net new in-scope production file,
`crates/jcode-provider-core/src/rate_limit_headers.rs`, the parser that feeds real
vendor rate-limit headers to the ambient scheduler. Measured rather than assumed,
by diffing the filter's own output against `github/main`:

    provider_infrastructure roots: crates/jcode-provider-core/,
      crates/jcode-provider-env/, crates/jcode-provider-metadata/,
      crates/jcode-auth-types/
    now in-scope: 20
    added vs merge-base:   ['crates/jcode-provider-core/src/rate_limit_headers.rs']
    dropped vs merge-base: []

Exactly one file, and the domain total moves 19 -> 20. The same commit also
extracted `crates/jcode-provider-core/src/lib_tests.rs`; that file is correctly
classified as a test by `rust_production_filter` and does not count.

## Why the enforcement gate stayed green while the test failed

`check_critical_path_budget.py` exits 0 here. Growth is normal work, and its own
test says so (`test_added_files_are_not_a_regression` adds 25 tui files and
expects no regression). The pin is not a ceiling on file count; it is the
denominator that stops a later *shrink* from reading as cleanup. Only the
self-test, which asserts the pin still equals the tree, can see staleness.

## Why this is not a re-baseline

The distinction that matters, and the one this program has gotten wrong before:
the per-domain debt ceilings (`panic`, `swallowed_error`, `oversize_files`) are
budgets that must never be raised to make a change pass. `EXPECTED_FILE_COUNTS`
is an inventory of what is in scope. No ceiling, target, or repository
high-water mark was touched, and enforcement reports unchanged headroom:

    lifecycle/swallowed_error is 2 below its ceiling
    tui/swallowed_error is 2 below its ceiling

Both were 2 before this change. The domain in fact got *healthier* as its file
count rose: the same commit shrank `jcode-provider-core/src/lib.rs` 1633 -> 1328
LOC, dropping it off the oversized-file list, and `swallowed_error` fell by 1
repository-wide. Growth in the inventory and reduction in the debt are separate
directions, and conflating them is exactly what this checker exists to prevent.

## Control: the stale pin was hiding real shrink slack

The precedent (`R08_scope_count_refresh.md`) established that a lagging count is
not merely untidy, it opens undetected deletion slack. Re-run for this domain,
simulating the deletion of one real in-scope provider file from the current tree:

    tree: 20   after simulated deletion: 19

    stale pin 19     -> NOT FLAGGED
    corrected pin 20 -> flagged: "provider_infrastructure lost in-scope
                        production files: 20 -> 19. Debt that leaves the
                        critical set is not debt that was fixed."

While the pin lagged the tree by one, a genuine deletion of an in-scope provider
file would have landed exactly on the stale expectation and passed silently. The
control fires in one direction and not the other, so the correction changes the
answer precisely in the case it was written for.

## Considered and rejected: move the module out of scope

Relocating `rate_limit_headers.rs` to a non-critical crate would make the test
pass without editing a protected path. It was rejected because it is the exact
antipattern the checker's own message names: debt that leaves the critical set is
not debt that was fixed. Folding the module back into `lib.rs` was also rejected
by measurement, not preference: 1328 + 456 lines lands far past that file's 1632
oversized-file baseline, so it would trade this gate for the code-size ratchet.

## Fix

`EXPECTED_FILE_COUNTS["provider_infrastructure"] = 20`, and the
`--expect-digest` pin in `fork-ci.yml` refreshed
`053c5c9838ae…` -> `6da4a35be70e6a8c842a1115260dd87dc23402aed698f1bca3139ce7c5ecd195`.

The digest refresh is not optional bookkeeping: `test_counts_are_pinned_by_the_digest`
makes the digest sensitive to `expected_file_counts`, which is the checker's own
guard against editing counts freely, and
`test_workflow_pin_matches_the_current_digest` fails until the workflow is
updated to match.

## Verification

    python3 -m unittest discover -s scripts -p 'test_critical_path_budget.py'
      Ran 32 tests ... OK   (exit 0)

    python3 scripts/check_critical_path_budget.py --expect-digest 6da4a35b… 
      provider_infrastructure  current= 20 expected= 20 (OK)
      Critical-path budget OK   (exit 0)

Exit statuses read directly, not through a pipe.

## Scope note

Both edited files (`scripts/check_critical_path_budget.py`,
`.github/workflows/fork-ci.yml`) are on the protected list in
`governance-root.yml`, so this requires the §4 transaction-bound maintenance
window. That the gate would trip was verified before pushing rather than
discovered from CI:

    git diff --quiet <merge-base> HEAD -- .github/workflows scripts/check_critical_path_budget.py
      exit 0 before these edits, exit 1 with them

Only a count that had fallen behind the tree was corrected, plus the digest that
pins it.
