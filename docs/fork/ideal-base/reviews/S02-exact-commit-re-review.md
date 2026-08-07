# S02: independent adversarial re-review at the exact commit

Reviewed source head: `8b5263cfaf1d04897ba15d8138341bbbe6f5a330`
(branch `automation/s01-fix-1`, worktree clean at review start and end).
Deterministic runtime subject: `ad7a7d585f77d48dedf47f9e44f6ff838f4405f1`.
Reviewer posture: read-only. The reviewer wrote no file; the coordinator preserved
this report after delivery.

## Verdict

**APPROVED. Zero blocking findings.**

All five defects that blocked the prior candidate `efb730a9a` are repaired at
this commit, and the reviewer confirmed each repair by recomputation rather
than by reading the repair's own prose. The determinism claim reproduces
independently: both committed rounds normalize to the recorded hash exactly.

## Executive summary

The prior review (`41bf14a6a`) blocked on two evidence-integrity defects; full
preflight added three more. This commit fixes all five, substantively rather
than cosmetically:

- The cited gate-3 sweep log is now tracked and checksummed.
- F03's README no longer over-claims coverage the fixture stopped asserting.
- rustfmt is clean at HEAD, verified by execution.
- The reviewed governance fix is now an ancestor of the review subject.
- The S01-FIX-1 summary was rewritten and no longer asserts stale semantics.

The two final rounds have disjoint wall-clock windows
(08:25:16 to 08:37:00 and 08:37:21 to 08:48:16), proving two real sequential
executions rather than a copied transcript. The frozen artifacts were not
changed after the fact.

## Per-requirement evidence

### Requirement 1: commit identity and ancestry

HEAD was `8b5263cfa` on `automation/s01-fix-1`; `git status --porcelain` was
empty at both review start and end. The relevant chain is:

`8b5263cfa` (record repaired signoff) -> `ad7a7d585` (repair blocked evidence
and formatting) -> `71da628a1` (merge protected-path equality fix) ->
`41bf14a6a` (blocking review) -> `efb730a9a` (superseded candidate) ->
`356476265`.

`8b5263cfa` is documentation/evidence-only relative to `ad7a7d585`, so the
runtime subject is `ad7a7d585`, exactly as both round headers record. The two
Rust files changed by `ad7a7d585` were changed before both rounds.

A post-review ref refresh corrected an initially stale ancestry observation:

- authoritative `github/main` is `6030441ab`, merge PR #135, with parents
  `c1fb14910` and `842cbd3ff`;
- `842cbd3ff` is ancestral to `8b5263cfa`;
- `git rev-list github/main ^8b5263cfa` returns only merge commit `6030441ab`;
- `git diff --name-only 842cbd3ff 6030441ab` is empty, so main contributes no
  content absent from the review subject;
- governance fix `1e356391e` is ancestral to the review subject but remains
  topic-only until S03 publication, which is correct under the two-phase
  railway protocol.

The reviewer's local `main` ref was stale at `c1fb14910`; publication checks
must use fetched `github/main`.

### Requirement 2: evidence integrity

The reviewer extracted `git archive 8b5263cfa` to a scratch directory, avoiding
working-tree and untracked-file confounds. All three relevant manifests passed
from committed bytes:

- S01 `SHA256SUMS` (15 files);
- S01-FIX-1 `SHA256SUMS` (`AMENDMENT.md`, `gate3-sweep.log`);
- F03 `SHA256SUMS` (3 files).

Every cited S01, S01-FIX-1, and F03 path is Git-tracked at the reviewed commit,
checked with `git ls-files --error-unmatch`.

### Requirement 3: determinism and controls

Both rounds contain 577 lines, `S01_ROUND N_STEP=18 N_FAIL=0`, and 36 PASS
lines. The only `FAIL` substring is the `N_FAIL=0` summary token. Both tails
record zero orphaned fixture children.

Recomputing both normalized hashes from committed bytes produced:

```
4db50a069513cc2d28c78320713101264f1e635a409b115576a61ea3299f1c52
```

for each round, with an empty normalized diff. Step arithmetic matches
`s01_matrix.sh`: 15 quality/hygiene gates + lifecycle matrix + F14 restoration
+ residue = 18.

F14 restoration is checked against F14's own pinned manifest in addition to
backup comparison, preventing a byte-identical copy of a wrong backup from
passing. Controls D1-D4 reran with 4 passing and 0 failing: sensitivity,
legitimate variation, clean specimen, and empty/truncated refusal.

### Requirement 4: normalizer chronology

`normalize.py`, `NORMALIZER_SPEC.md`, `PREDICTIONS.md`, and `controls.py` each
have exactly one commit in their history: `a30670c1f` at 2026-08-06 18:09:22
-0400, roughly fourteen hours before the final rounds. Neither `ad7a7d585` nor
`8b5263cfa` changes them.

The observed disagreements were repaired in the harness, as predicted:

- S01-F7 localized nondeterminism to libtest interleaving and the round label;
  the remedy pinned `RUST_TEST_THREADS=1` and removed the self-identifying label
  from the transcript body.
- The superseded C/D pair differed because prewarm preceded the final harness
  commit; the remedy aligned prewarm and both rounds at one unchanged HEAD.

The failing hashes and remedies remain recorded in `FINDINGS.md`.

### Requirement 5: graph coverage and external dispositions

`STATE.json` contains 75 records: 69 accepted, 2 implemented (S01,
S01-FIX-1), 2 pending (S02, S03), 1 in progress (W5), and 1 superseded
(F26-FIX-1). The railway validator reports 9 roots, 66 child nodes, 75 state
records, and an intact protected hash.

External gates G01-G05 are accepted, with main-ancestral published commits:

| Node | Reviewed commit | Published commit | Main-ancestral |
| --- | --- | --- | --- |
| G01 | `4df45d719` | `b88250783` | yes |
| G02 | `6fb703745` | `a5e1cb594` | yes |
| G02-FIX-1 | `4f95bdbf4` | `4308b6a14` | yes |
| G03 | `35c13d5d8` | `b88250783` | yes |
| G04 | `9a34ff77b` | `b88250783` | yes |
| G05 | `a632aed2c` | `b88250783` | yes |

S01 correctly has null reviewed and published commits before S02/S03.

W5's in-progress summary is stale by about five days: it says G01/G02/G03/G05
still need authorization or hardware and S01-S03 are blocked, although those
gates are accepted and S01 is implemented. This is not evidence and nothing
cites it; W5 remains open until S03, and STATE is coordinator-owned. It is
therefore non-blocking for S02 but a hard S03 precondition: S03 must replace
the stale summary before W5 can be accepted.

### Requirement 6: documentation and instruction checks

- `check_docs_references.py`: OK, 130 active documents, 0 machine-local,
  0 stale-code-path at baseline.
- `check_agent_instructions.py`: OK, projected 7433/8192, compiled 7181/8192.
- `ideal_base_railway.py check`: OK, protected hash intact.

## Prior-blocker disposition

| # | Prior blocker | Status | Independent confirmation |
| --- | --- | --- | --- |
| B1 | `gate3-sweep.log` cited but untracked | **FIXED** | Tracked at HEAD; new S01-FIX-1 manifest covers it and passes from `git archive` |
| B2 | F03 README over-claimed an 8-class post-release idle window | **FIXED** | README matches fixture: 8 classes for held-past-timeout/exit-44/residue, 7 for post-release window; manifest passes |
| B3 | Full preflight failed rustfmt in `reload.rs` and `env.rs` | **FIXED** | `cargo fmt --all -- --check` in the Nix dev shell exits 0 |
| B4 | Governance fix absent from the branch | **FIXED** | `1e356391e` is ancestral via `71da628a1`; main publication is correctly deferred to S03 |
| B5 | S01-FIX-1 asserted stale F09 semantics | **FIXED** | Full summary diff confirms rewrite; S01-F4 records that F09 moved to `jcode-app-core` and remains active |

The reviewer disclosed an initially over-narrow regex for B5, then corrected it
by diffing the complete summary rather than relying on the empty regex result.

## Findings, ordered by severity

**No blocking findings.**

### O1: superseded rounds C-H are untracked

The operator's global `*.log` ignore excludes C-H. Nothing cites those paths.
`round-G` is byte-identical to canonical round A and `round-H` to round B;
C/D/E/F pin different source commits, consistent with the disclosed chronology.
A repository-level ignore negation would prevent this defect class recurring.

### O2: no committed full-preflight transcript

The final README and findings say the repaired commit passed full local
preflight, but no transcript is committed. The reviewer independently reran the
prior failing rustfmt surface; the coordinator also ran full preflight before
the final pair. This remains a documentation gap, not a false claim.

### O3: env-flag refactor ownership stretch

The prior review's non-blocking ownership observation remains: the disclosed
cross-crate env-flag consolidation touches product paths not listed under the
fix node's narrow ownership. It is mechanical and covered by the final rounds,
but belongs in the retrospective.

### O4: W5 summary is stale

W5's open-node narration contradicts completed child states. It is non-binding
for S02, but S03 must correct it before W5 acceptance.

### O5: local `main` is stale

Re-verifiers must fetch and use `github/main`; bare local `main` still names
`c1fb14910` rather than authoritative `6030441ab`.

## Edge cases considered

- Copied good hash versus recomputation: rejected by disjoint wall-clock windows
  and independent recomputation.
- Post-mismatch normalizer widening: rejected by single-commit frozen-artifact
  history predating the rounds.
- Quiet replacement of a failing round: rejected by byte equality G=A and H=B
  and distinct source headers for superseded E/F.
- Runtime change hidden by final docs commit: rejected by the docs-only
  `8b5263cfa` diff and round headers at `ad7a7d585`.
- Wrong F14 backup passing: rejected by F14's own manifest check.
- Hidden non-FAIL verdict: rejected by 36 PASS lines and no other FAIL token.
- Dirty working-tree evidence: rejected by verification from `git archive`.
- Prior dirty controls capture: no longer applies; controls log is clean.
- Main-only content drift: rejected by the merge-base-anchored empty diff.

## Confidence

**High** on evidence integrity, determinism, and prior-blocker closure because
those rest on independent recomputation from the commit tree.

**Medium** on the full-preflight assertion because the reviewer reran rustfmt,
not the entire preflight suite. The coordinator's full exact-HEAD preflight run
is recorded in the S01 findings but lacks a committed transcript.

## What the reviewer did not check

- Did not run `scripts/preflight.sh` end to end, Clippy, or a full build/test run.
- Did not rerun the 18-step matrix; verified committed transcripts and harness logic.
- Did not exercise external gates requiring authorization or hardware.
- Did not audit the substantive env-flag refactor beyond disclosure.
- Did not reproduce the remote-builder defect recorded in findings.

## Errors disclosed by the reviewer

1. An over-narrow F09 regex initially returned no mention; full-diff review
   corrected it before verdict.
2. A two-point diff initially conflated topic-only changes with possible
   main-only drift; merge-base-anchored checks corrected it.
3. W5's stale summary was initially characterized as honest; that wording was
   retracted and converted into a binding S03 action.

## Open questions and required S03 actions

1. Consider a repository-level ignore negation for evidence `.log` files.
2. Consider capturing future full-preflight transcripts as committed evidence.
3. Before W5 acceptance, replace its stale summary with the actual G01-G05 and
   S01-S03 dispositions.
4. Fetch and verify authoritative `github/main` during publication; ensure
   `1e356391e` and the reviewed S01/S02 commits become main-ancestral.
5. Populate S01/S01-FIX-1/S02 reviewed and published identities only after
   publication, never before.

## Final

**APPROVED.** Both signoff rounds independently reproduce
`4db50a069513cc2d28c78320713101264f1e635a409b115576a61ea3299f1c52`
(577 lines, empty normalized diff) over disjoint wall-clock windows, with the
normalizer frozen at `a30670c1f` about fourteen hours earlier and untouched by
either repair commit. The challenged contradictions sharpen the record and add
binding S03 actions; neither invalidates S01.
