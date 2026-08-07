# §4 maintenance window: PR #136 (ideal-base final integration)

Prepared 2026-08-07 before any ruleset write. PR #136 is the final integration
of the ideal durable TUI/CLI foundation on branch
`automation/s01-fix-1`.

**Status: EXECUTED 2026-08-07.** The repository owner authorized the requested
merge and necessary follow-up work in the session that opened PR #136. The
window opened at `12:54:29Z`, closed at `12:54:40Z`, and merged exact reviewed
head `e8ef0d131a337f8335d11f6d3f365ffb689b97d7` as
`f403a878ac64ef841d0a328c1d01b081fbf33dd7`. The literal pre-window ruleset was
restored to canonical SHA-256
`43ba61a7a57ffded7a4276917192cdd6028f79d58755cae870cbb2df07494f2b`.

## Why a window is required

`Governance Root` is deliberately red for a pull request that changes a path
capable of changing governance's own judgment. PR #136 includes the separately
reviewed protected-path equality repair, so ordinary required-check merging is
correctly blocked and R07 design §4 is the only authorized merge path.

The prediction is derived from the inline `protected=( ... )` array in
`.github/workflows/governance-root.yml`, the list the workflow actually runs.
It parses to 32 non-empty patterns. The first observed head had 41 changed
files; the final reviewed head had 42 after adding this pre-window record.
Comparing either complete diff to the list yields exactly five protected paths:

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

Committing this record advanced the PR head, and strict required checks then
exposed that the continued branch did not contain main's prior PR #135 merge
commit. A predicted merge-tree check proved that merging current main into the
branch changed ancestry only: the prospective merge tree and reviewed head tree
were both `9e6c582dff7e8a942a65e3a4bd940499e4d7cdc0`. Commit `e8ef0d131` bound
current main into the branch, after which GitHub reported the PR structurally
mergeable and blocked only by the expected `Governance Root` failure.

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
the two ruleset writes and the SHA-conditioned merge. Because this PR changes
`scripts/governance_compare.py`, the harness loads `sanitize()` and
`canonical()` from the captured base commit rather than the candidate worktree;
it separately loads the head implementation and requires both to hash the
complete live body identically before any write.

## Sequence executed

| # | step | result |
|---|---|---|
| 1 | bind repository, PR, source, head, base, and actor identities | repository id `1238606714`; owner `jerudnik`; source `jerudnik:automation/s01-fix-1`; head `e8ef0d131`; base/main `6030441ab` |
| 2 | prove required-check pattern on the exact head | `Governance Root: failure`; `Fork CI Gate`, `Security Gate`, and `Nix Gate`: `success`; each emitted exactly once by GitHub Actions app id `15368` |
| 3 | prove complete diff and protected subset | 42 files returned for 42 declared; five protected paths, exactly the predicted set |
| 4 | capture and compare the live ruleset | base and head comparator sources differ, but both hash the complete live body as `43ba61a7a5…`; active, strict, no bypass actors, four required contexts |
| 5 | validate prospective drop body | only `rules` differs; only `Governance Root` is removed; dropped-body hash `7e6ba479dd…` |
| 6 | open and read back the window | opened `12:54:29Z`; contexts exactly `Fork CI Gate`, `Nix Gate`, `Security Gate` |
| 7 | SHA-conditioned merge | API accepted `sha: e8ef0d131…`, `merge_method: merge`; merge `f403a878ac64ef841d0a328c1d01b081fbf33dd7` |
| 8 | verify merge identity | authoritative main equals merge response; ordered parents exactly `[6030441ab…, e8ef0d131…]` |
| 9 | restore literal pre-window ruleset | closed `12:54:40Z`; fresh read-back hash exactly `43ba61a7a5…`; all four contexts restored |
| 10 | prove no concurrent publication | first-parent range contains exactly one merge, equal to `f403a878…`; fresh main still equals it |

Full transaction output is
`transcripts/maintenance-window-pr136.txt`. The harness is
`transcripts/window-pr136.py`.

## Independent post-window comparison

R07 §4 requires both the pre-window and post-merge comparator implementations
to judge the restored live state independently:

- base `6030441ab`: `scripts/fork-health.sh --live` exits 0 and reports every
  invariant holding (`transcripts/fork-health-pr136-base.txt`);
- merge `f403a878a`: the same command exits 0 and reports every invariant
  holding (`transcripts/fork-health-pr136-merge.txt`).

Their protected-path detail differs in the intended way. The old comparator
prints the historical 31-path manifest and does not notice that the workflow
already contains 32 entries. The merged comparator proves the workflow and
manifest contain exactly the same 32 paths. Both agree on server governance and
the exact restored ruleset; the new output demonstrates the self-attestation gap
closed by this PR rather than hiding it.

A separate fresh API read after restoration loaded the comparator from base,
reviewed head, and merge commits. All three independently produced ruleset hash
`43ba61a7a5…`; the live body was `active`, retained `bypass_actors: []`, and
required exactly `Governance Root`, `Fork CI Gate`, `Security Gate`, and
`Nix Gate`.
