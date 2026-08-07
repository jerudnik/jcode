# §4 maintenance window: PR #138 (atomic final-child/root checkpoint)

Prepared 2026-08-07 before any ruleset write. PR #138 is the bounded repair for
the S03 closeout deadlock on branch `automation/s03-atomic-checkpoint`:
<https://github.com/jerudnik/jcode/pull/138>.

**Status: PLANNED.** The repository owner explicitly authorized opening this
window at `2026-08-07T13:25:48Z`. No governance write has occurred for PR #138.
The final reviewed head is captured after this record lands and is a hard
precondition of the transaction harness.

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

The workflow parses to 32 non-empty patterns. The complete preliminary PR diff at
`3d112579be54c379a493999600052315f4414b75` contains three files and produces
exactly those two protected hits; `docs/fork/ideal-base/EXECUTION_PROTOCOL.md` is
not protected. The workflow list and `scripts/required-checks.json` are currently
set-equal. The prediction must be re-run on the final head and compared with the
actual `Governance Root` failure before the window opens.

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
`--commit` flag and this owner authorization. Executed timestamps, SHAs, canonical
hashes, transcript paths, and post-window comparator results are written in a
follow-up evidence commit after governance is restored.
