# S03 final ideal-base disposition review

**Reviewed commit:** `af46b27ede6e1cee2389cc48c882d26526c7777b`
(branch `automation/s01-fix-1`)

**Worktree:** clean at review start and end (`git status --porcelain` empty)

**Reviewer posture:** read-only and adversarial. No file written or commit made
by the reviewer.

## Verdict

**APPROVED. Zero blocking findings.**

## 1. Commit identity, worktree, and authoritative main

- HEAD was exactly `af46b27ed`; worktree clean; branch
  `automation/s01-fix-1`.
- The candidate touches only `STATE.json` and `evidence/S03/README.md`, with no
  hidden runtime change.
- Local `main` is stale at `c1fb14910`; authoritative fetched `github/main` is
  `6030441ab` (merge PR #135). All publication checks used the fetched ref.

Pre-publication ancestry:

| Commit | In candidate | In `github/main` |
| --- | --- | --- |
| `1e356391e` governance fix | yes | no |
| `8b5263cfa` S01 subject | yes | no |
| `8eba1833c` S02 checkpoint | yes | no |
| `af46b27ed` S03 candidate | self | no |
| `6030441ab` authoritative main | no | yes |

None of the topic commits are yet on main, which is correct for the two-phase
protocol. Merge-base is `842cbd3ff`. Main contributes no hidden content:
`git diff --name-only 842cbd3ff 6030441ab` is empty, and
`git rev-list github/main ^8b5263cfa` returns only merge node `6030441ab`.

## 2. Label defensibility against A0-A9

Candidate label: **Unqualified ideal-base signoff for the advertised surfaces,
pending merge-only publication and post-merge checkpoint.**

`ACCEPTANCE_STANDARD.md` permits that label when mandatory work and all
advertised external gates are accepted. That condition is satisfied by the
candidate and its post-merge transition:

- all mandatory deterministic work is complete;
- G01-G05 and G02-FIX-1 are accepted;
- the full matrix passed twice at one commit with zero residue;
- S02's independent Opus-class review found zero blockers.

The label is not too strong because "advertised" is load-bearing and the S03
disposition explicitly refuses to expand platform, packaging, provider,
browser, or distribution claims. It is not too weak because no advertised gate
remains blocked.

## 3. Determinism independently recomputed

From `git archive 8b5263cfa`:

- round A and round B contain 577 lines each;
- both report `N_STEP=18 N_FAIL=0`;
- committed `normalize.py` produces for both:

```
4db50a069513cc2d28c78320713101264f1e635a409b115576a61ea3299f1c52
```

- the value matches `NORMALIZED_SHA256SUMS` exactly;
- all 15 S01 manifest entries pass;
- the normalizer was frozen at single commit `a30670c1f` before the rounds.

The final result reproduces from committed bytes rather than trusting the
recorded number.

## 4. Graph state, counts, and proposed checkpoint

Candidate STATE contains:

- 69 accepted;
- 4 implemented: S01, S01-FIX-1, S02, S03;
- 1 in progress: W5;
- 1 superseded: F26-FIX-1;
- total: 75.

The candidate correctly keeps W5 in progress, moves S03 from pending to
implemented with null identities, and leaves the other pre-publication
identities null.

Post-merge target is 74 accepted + 1 superseded = 75. The proposed mappings are
defensible:

- reviewed `8b5263cfa` for S01 and S01-FIX-1;
- reviewed `af46b27ed` for W5, S02, and S03;
- final merge SHA as published identity for all five;
- `last_checkpoint` updated to S03;
- coordinator checkpoint local-only and unpushed.

Accepted gate published commits (`b882507834`, `a5e1cb594`, `4308b6a14`) are
main-ancestral. No accepted gate is omitted and no blocked gate remains.

## 5. W5 stale-summary correction

The candidate replaces W5's stale summary, which said external gates still
needed authorization/hardware and S01-S03 were blocked, with the accurate
in-progress disposition while keeping W5 in progress. This satisfies S02's
binding S03 precondition without prematurely accepting W5.

## 6. Support-limitation honesty

Each external statement matches accepted gate evidence:

- G01: aarch64-linux is best-effort, not parity.
- G02: Claude and Bifrost full tiers; Gemini/Copilot missing-login next action;
  Perplexity defect repaired by accepted G02-FIX-1.
- G03: packaged browser control surface, explicitly not an installable or secure
  remote PWA.
- G04: Windows/FreeBSD unsupported; no installer smoke promised.
- G05: qemu binfmt emulation, not native x86_64 silicon; cache trust documented.
- Distribution: Nix-only; iOS retired; GitHub release artifacts and native
  installers outside the supported surface.

No blocked gate is described as passing.

## 7. Narrow non-destructive checks

All green:

- `ideal_base_railway.py check`: 9 roots, 66 child nodes, 75 records, protected
  hash intact.
- `check_docs_references.py`: 130 active docs, 0 machine-local, 0 stale-code-path.
- `check_agent_instructions.py`: projected 7433/8192, compiled 7181/8192.
- S01-FIX-1 manifest: `AMENDMENT.md` and `gate3-sweep.log` OK.
- F03 manifest: README, fixture, and run log OK.
- `cargo-machete` absent, matching the S03 claim.
- C-H round logs untracked, matching the global-ignore residual.

The reviewer did not rerun the matrix, provider spends, hardware/G05
acquisition, full preflight, or any push.

## 8. Publication plan

The authorization-gated two-phase protocol is complete and unambiguous:

- every required check must be green on the final head; skipped or absent is not
  green;
- merge commit only, never squash/rebase/force-push/reset;
- branch push, PR creation, and merge require explicit authorization;
- delete only the merged topic branch and preserve archive/recovery refs;
- refetch `github/main` and prove ancestry of `af46b27ed`, `8b5263cfa`,
  `8eba1833c`, and `1e356391e`;
- perform the post-merge coordinator checkpoint locally without pushing.

Leaving the reviewed S03 identity and final merge SHA undeclared before the
review/merge is correct. This report establishes `af46b27ed` as the reviewed
S03 identity.

## 9. Residual and deferred list

All residuals are outside or narrower than the advertised surface:

1. `JCODE_TEST_SESSION` / `ReloadEnvironment` seam deferred; current fallback is
   tested and matrix-covered.
2. Env-flag refactor is a disclosed cross-crate ownership stretch; future
   cross-cutting refactors should get their own node.
3. Superseded C-H logs are globally ignored, uncited, and represented by tracked
   canonical G/H bytes as A/B; ignore negation recommended.
4. Full-preflight transcript is not committed; this is the one documentation
   gap, but the failing surface was rerun and full preflight was coordinator-run.
5. External state is fixed-time evidence, not a perpetual claim.
6. PWA work remains explicitly deferred.

## Findings by severity

**Blocking:** none.

**Non-blocking:**

- full-preflight transcript is uncommitted;
- env-flag refactor ownership stretched beyond the original node paths;
- repository-level evidence `.log` ignore negation is recommended.

## Validation performed

Independent hash recomputation from `git archive`; S01, S01-FIX-1, and F03
manifest verification; live ancestry and merge-base content-equality; railway,
documentation, and instruction checks; STATE diff/count arithmetic; accepted
gate publication ancestry; residual-claim spot checks.

## Edge cases considered

Stale local versus fetched main; main-only content hidden by a merge node;
premature accepted identities; skipped checks treated as green; squash/rebase
mapping loss; stale W5 narration; normalizer widening; copied transcripts;
ignored logs cited as tracked; unsupported platforms in final label; external
state drift.

## Confidence

**High** for the advertised-surface disposition because the reviewer independently
recomputed load-bearing claims from committed bytes.

**Medium** only for the uncommitted full-preflight assertion because the reviewer
did not rerun the entire suite.

## What was not checked

The reviewer did not rerun the 18-step matrix, provider spends, G05 acquisition,
full `preflight.sh`/Clippy end to end, or substantive env-flag implementation.
No push, PR, or merge was performed.

## Exact required publication and checkpoint actions

1. Record this report with reviewed commit `af46b27ed`.
2. With explicit authorization only, push `automation/s01-fix-1`, open a PR
   against fetched `github/main`, wait for every required check green, and merge
   with a merge commit only.
3. Refetch `github/main`; verify the merge SHA is authoritative and that
   `af46b27ed`, `8b5263cfa`, `8eba1833c`, and `1e356391e` are main-ancestral,
   with no red or incomplete check.
4. Delete only the merged topic branch; preserve archive/recovery refs.
5. Apply one local-only coordinator checkpoint: accept W5/S01/S01-FIX-1/S02/S03;
   reviewed=`8b5263cfa` for S01/S01-FIX-1;
   reviewed=`af46b27ed` for W5/S02/S03; published=final merge SHA for all five;
   `last_checkpoint` -> S03; leave worktree clean; do not push.

## Final disposition

**APPROVED for publication under the stated authorization-gated protocol.**
