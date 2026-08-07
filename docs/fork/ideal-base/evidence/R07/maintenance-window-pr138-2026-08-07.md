# §4 maintenance window: PR #138 (atomic final-child/root checkpoint)

Prepared 2026-08-07 before any ruleset write. PR #138 is the bounded repair for
the S03 closeout deadlock on branch `automation/s03-atomic-checkpoint`:
<https://github.com/jerudnik/jcode/pull/138>.

**Status: EXECUTED 2026-08-07.** The repository owner explicitly authorized
opening this window at `2026-08-07T13:25:48Z`. The window opened at
`13:52:51Z`, closed at `13:52:58Z`, and merged exact reviewed head
`35fca50c71eaa1b684a3d70c91ded1abbd66995f` as
`54b6f52fb07c042a86f90bcc7dec3d9fe918b9e8`. The literal pre-window ruleset was
restored to canonical SHA-256
`43ba61a7a57ffded7a4276917192cdd6028f79d58755cae870cbb2df07494f2b`.

## Why a window is required

D02 correctly added both sides of the expansion-consistency invariant:

- a complete root cannot strand an incomplete child;
- an open root cannot remain open after every child is complete.

Those two guards make the final child/root transition impossible as two
single-node writes. Completing the child first introduces the second violation;
completing the root first introduces the first. `checkpoint` validates after
every write, so no ordering crosses the boundary.

PR #138 adds one narrow transaction: `checkpoint-batch` accepts an ordered list
limited to one expansion root and its direct children, applies the records in
memory under the existing state lock, validates the final state once, and makes
one atomic write. It does not weaken either expansion guard.

The PR changes two paths protected by the inline `protected=( ... )` array in
`.github/workflows/governance-root.yml`:

```text
scripts/ideal_base_railway.py
tests/test_ideal_base_railway.py
```

The workflow parses to 32 non-empty patterns. The preliminary PR diff at
`3d112579be54c379a493999600052315f4414b75` contained three files and produced
exactly those two protected hits. The final reviewed diff contained four files
after adding this record and still produced exactly those two hits;
`docs/fork/ideal-base/EXECUTION_PROTOCOL.md` and this record are not protected.
The workflow list and `scripts/required-checks.json` were set-equal. `Governance
Root` run `92881830385` failed naming exactly `scripts/ideal_base_railway.py` and
`tests/test_ideal_base_railway.py`.

## Validation before publication

- `python3 -m unittest tests.test_ideal_base_railway`: **29/29 pass**.
- `ruff check scripts/ideal_base_railway.py tests/test_ideal_base_railway.py`: pass.
- `python3 scripts/ideal_base_railway.py check --published-ref refs/remotes/github/main`:
  pass, 9 roots, 66 children, 75 state records, protected hash intact.
- `python3 scripts/check_docs_references.py`: pass.
- Independent read-only review: **no blockers** for state corruption, governance
  weakening, or the W5/S03 transaction.

The regression proves both single-node orderings fail without changing the
scratch STATE bytes, the ordered batch closes the final child plus root with no
expansion violation, the final repeated child controls `last_checkpoint`, and an
invalid record leaves the file byte-identical.

## Transaction and stop conditions

The window follows R07 design §4:

1. Bind PR #138 to its final reviewed head, current `main` base, repository id,
   actor, source branch, and merge-commit-only configuration.
2. Require exactly one check run for each required context, emitted by GitHub
   Actions integration id `15368`: `Governance Root: failure`, and `Fork CI Gate`,
   `Security Gate`, and `Nix Gate` all `success`.
3. Require the complete PR file list and exact protected subset above. Any extra
   protected path is a stop, not a widened window.
4. Capture and canonically hash the complete live `protect-fork-rails` body with
   the comparator loaded from the captured base commit. Require the head
   comparator to agree independently.
5. Prove the prospective body removes only `Governance Root`, with every other
   sanitized field unchanged.
6. Run the transaction harness in dry-run mode. A mismatch is a stop condition.
7. With `--commit`, temporarily apply the dropped body, read it back, merge PR
   #138 with both exact head SHA and `merge_method: merge`, and restore the literal
   pre-window body in `finally`.
8. Prove fresh read-back hash equality, all four contexts restored, ordered merge
   parents equal the captured base and reviewed head, and exactly one first-parent
   merge occurred during the window.
9. Run `scripts/fork-health.sh --live` independently from the pre-window base and
   new merge commit. A disagreement is a governance incident.

The harness is dry-run by default. The live invocation requires the literal
`--commit` flag and this owner authorization.

## Sequence executed

The first dry run stopped before any write because the reused PR #136 harness
still asserted that the candidate comparator source must differ from the base.
That assertion was specific to #136, which changed the comparator; #138 does not.
The harness was corrected to require byte-identical base/head comparator sources,
recompiled, and rerun. The second dry run passed every stop condition. No live
governance write occurred during the failed dry run.

| # | step | result |
|---|---|---|
| 1 | bind repository, actor, PR source, reviewed head, and live base | repository id `1238606714`; owner `jerudnik`; source `jerudnik:automation/s03-atomic-checkpoint`; head `35fca50c7`; base `3317a2ca9` |
| 2 | prove exact check pattern and emitters | `Governance Root: failure`; `Fork CI Gate`, `Security Gate`, and `Nix Gate`: `success`; all GitHub Actions app id `15368` |
| 3 | prove complete diff and protected subset | four files returned for four declared; exactly the two predicted protected paths |
| 4 | capture and compare live ruleset | base/head comparator sources both `864352fd25…`; both hash the full body as `43ba61a7a5…`; active, strict, no bypass actors, four required contexts |
| 5 | validate prospective drop body | only `rules` differs; only `Governance Root` removed; dropped-body hash `7e6ba479dd…` |
| 6 | open and read back window | opened `13:52:51Z`; contexts exactly `Fork CI Gate`, `Nix Gate`, `Security Gate` |
| 7 | SHA-conditioned merge | API accepted exact head and merge method; merge `54b6f52fb07c042a86f90bcc7dec3d9fe918b9e8` |
| 8 | verify merge identity | authoritative `main` equals merge response; ordered parents exactly `[3317a2ca9…, 35fca50c7…]` |
| 9 | restore literal pre-window body | closed `13:52:58Z`; fresh read-back hash exactly `43ba61a7a5…`; all four contexts restored |
| 10 | prove no concurrent publication | first-parent range contains exactly one merge, equal to `54b6f52fb…` |

The full transaction output is
`transcripts/maintenance-window-pr138.txt`; the fail-closed harness is
`transcripts/window-pr138.py`.

## Independent post-window comparison

R07 §4 requires both comparator generations to judge restored live governance:

- base `3317a2ca9`: `scripts/fork-health.sh --live` exits 0 and reports every
  invariant holding (`transcripts/fork-health-pr138-base.txt`);
- merge `54b6f52fb`: the same command exits 0 and reports every invariant holding
  (`transcripts/fork-health-pr138-merge.txt`).

Both independently verify the 32-path workflow/manifest equality, exact required
contexts, active effective rules, absent classic protection, empty bypass actors,
and merge-commit-only repository configuration. A fresh fetch also proves
`github/main == 54b6f52fb07c042a86f90bcc7dec3d9fe918b9e8`, with ordered parents exactly
the captured base and reviewed head.

## Post-review harness hardening

Security review on the follow-up evidence PR correctly found that the archived
harness originally compiled the PR-head copy of `governance_compare.py` before
checking its source hash against the trusted base copy. The executed PR #138 head
was owner-authored, bound to an exact reviewed SHA, and the two sources were
byte-identical, so the completed transaction was not compromised. The generic
ordering was nevertheless unsafe for reuse with an untrusted head.

`transcripts/window-pr138.py` now reads both comparator sources as raw inert
bytes, logs their SHA-256 values, proves direct raw-byte equality, and only then
decodes and compiles either copy. The transaction transcript remains unchanged so
it continues to record the exact executed output.
