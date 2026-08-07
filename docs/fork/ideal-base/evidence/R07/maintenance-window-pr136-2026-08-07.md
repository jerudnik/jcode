# §4 maintenance window: PR #136 (ideal-base final integration)

Prepared 2026-08-07 before any ruleset write. PR #136 is the final integration
of the ideal durable TUI/CLI foundation on branch
`automation/s01-fix-1`.

**Status: PLANNED.** The repository owner authorized the requested merge and
necessary follow-up work in the session that opened PR #136. The window must
not open until the final live PR head has `Governance Root: failure` and the
other three required contexts at `success`, and the transaction harness passes
its read-only dry run.

## Why a window is required

`Governance Root` is deliberately red for a pull request that changes a path
capable of changing governance's own judgment. PR #136 includes the separately
reviewed protected-path equality repair, so ordinary required-check merging is
correctly blocked and R07 design §4 is the only authorized merge path.

The prediction is derived from the inline `protected=( ... )` array in
`.github/workflows/governance-root.yml`, the list the workflow actually runs.
It parses to 32 non-empty patterns. Comparing the complete 41-file PR diff to
that list yields exactly five protected paths:

```text
docs/fork/ideal-base/evidence/R07/github-governance.proposed.json
scripts/ambient_roots_allowlist.txt
scripts/governance_compare.py
scripts/required-checks.json
tests/test_governance_compare.py
```

All five belong to the reviewed ideal-base work. The first observed head,
`e028b4b734be656166479a4eedb18392ff20057d`, produced `Governance Root:
failure` in check run `92859093608`; its failure annotation says
`governance paths changed; use the recorded ruleset maintenance procedure
(design.md section 4)`. `Security Gate` was green on that head. `Fork CI Gate`
and `Nix Gate` had not emitted their summary checks yet, so this observation is
evidence of the expected failure class, not authorization to open the window.

Committing this record advances the PR head. The SHA above is therefore a
scored precursor, not the `head_sha` permitted for the merge transaction. The
transaction harness must re-read the final live head, all four check results,
the complete PR file list, live `main`, repository identity, and the full
ruleset body immediately before any write. Any mismatch is a stop.

## Transaction and stop conditions

The execution follows R07 design §4 literally:

1. Bind PR #136 to its final reviewed head and current `main` base.
2. Require `Governance Root: failure` and the other three required contexts at
   `success`, all emitted by GitHub Actions integration id 15368.
3. Capture and canonically hash the complete live `protect-fork-rails` body.
4. Prove the prospective body drops only `Governance Root`, with every other
   sanitized field unchanged.
5. Temporarily apply that body, read it back, and start the window clock.
6. Merge PR #136 with both `sha: head_sha` and `merge_method: merge`.
7. Restore the literal pre-window body in a `finally` path and prove its fresh
   read-back hash equals the captured hash.
8. Prove `main` equals the merge response, its ordered parents are exactly the
   captured base and reviewed head, and the first-parent range contains exactly
   that one merge.
9. Run `scripts/fork-health.sh --live` from both the pre-window base and the new
   merge commit. A disagreement is a governance incident, not a retry.

The harness is dry-run by default and requires an explicit `--commit` to make
the two ruleset writes and the SHA-conditioned merge. The executed transcript,
window timestamps, merge SHA, restored ruleset hash, and both comparator
results will be recorded after the transaction; none is pre-written here as if
it had already happened.
