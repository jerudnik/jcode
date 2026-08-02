# W6 integration merge blocker — Governance Root vs. the critical-path pin

Status: **blocked on user authorization.** All engineering work on PR #90 is
green; the remaining blocker is a governance rule that cannot be satisfied from
inside the PR.

## What is green

16 of 17 checks pass, including the two that were genuinely failing earlier and
were fixed by shrinking rather than re-baselining:

- **Quality Guardrails: pass.** The oversized-file ratchet, the swallowed-error
  ratchet, and the checker self-test all pass.
- Fork CI Gate, Linux Tests, Build & Test (macOS), Security Gate, Nix Gate,
  Governance Contract Gate: pass.

The single failure is **Governance Root**, at its "Detect governance-path
changes" step. That step is an audit gate: it fails whenever a PR modifies any
path in its protected list, regardless of whether the change is correct.

## Why this cannot be fixed inside the PR

It is a closed loop, and each half is individually reasonable:

1. `scripts/check_critical_path_budget.py` records an expected in-scope file
   count per domain in `EXPECTED_FILE_COUNTS`. Three self-tests (run by Quality
   Guardrails) assert **strict equality** between that record and the tree.
2. `crates/jcode-tui/**` is one of those domains. Any commit that adds a file
   there — including extracting an oversized file into a module, which the
   oversized-file ratchet actively demands — changes the tree count.
3. Satisfying (1) therefore requires editing
   `scripts/check_critical_path_budget.py`.
4. That file is in Governance Root's protected list, so editing it fails
   Governance Root.

So on this branch: leave the pin stale and Quality Guardrails fails; correct the
pin and Governance Root fails. Measured, not inferred:

```text
main pins tui = 191
tree (this branch) = 193
```

### Correction: which check actually forces the edit

An earlier revision of this memo said a **stale count fails the budget gate**.
That is false, and an independent review (sheep) caught it. Re-measured:

```text
pin 191, tree 193 -> scope_shrink_regressions() = NONE, budget gate PASSES
```

By design: `scope_shrink_regressions` filters on `count < EXPECTED` only, so a
stale **low** pin is invisible to it. The edit is forced by strict-equality
self-tests instead. Reverting the pin to 191 and running the suite gives
**32 tests, 3 failures**:

```text
test_expected_counts_match_the_current_tree      (scripts/test_critical_path_budget.py)
test_expected_counts_sum_to_the_scanned_total    (scripts/test_critical_path_budget.py)
test_workflow_pin_matches_the_current_digest     (the digest pin in fork-ci.yml)
```

The deadlock is real and the conclusion is unchanged, but the mechanism is the
self-test's strict equality, **not** the shrink gate. Recorded because a memo
that names the wrong cause is the same defect class this program exists to fix:
a true-sounding report that is not true.

### The two protected paths are mechanically coupled

The third failure above is not incidental. The workflow digest is computed over
`DIGEST_FIELDS`, which includes `expected_file_counts`, so changing only the
inventory necessarily invalidates the pin in `fork-ci.yml`:

```text
change ONLY EXPECTED_FILE_COUNTS["tui"] -> digest 053c5c9838ae -> 6e9367a924cc
```

So a single measurement change *provably requires two protected-path edits*:
`scripts/check_critical_path_budget.py` and `.github/workflows/fork-ci.yml`.
The blocker is not "an edit that happens to touch protected files"; it is a
structural coupling with no in-PR resolution.

The two extra files are `commands_poke.rs` (R08) and
`remote_history_watchdog.rs` (R09), both extractions the size ratchet required.

This is not a broken `main`. A detached worktree at `github/main` runs its own
checker self-test **32/32 OK**: main is internally consistent at 191. The
deadlock exists only for a branch that adds an in-scope tui file, which is
exactly what the size ratchet pushes work toward.

## Why the pin refresh is not itself "stale bookkeeping"

Refreshing this pin looks like the move that was already wrong twice in this
program (`--update` on a ratchet). It is not, and this was measured rather than
assumed. Growth is explicitly allowed by this budget; the pin is the
*denominator* that stops a **shrink** from reading as cleanup:

```text
pinned 192, tree 193, delete one real tui file -> flagged? False   (stale: real deletion passes silently)
pinned 193, tree 193, delete one real tui file -> flagged? True
pinned 193, tree 193, no change                 -> flagged? False
```

A stale pin is one file of undetected shrink slack, so leaving it stale would
itself be a gate reporting OK for something it never verified.

## What is actually required

`gh pr merge 90 --merge` fails with "the base branch policy prohibits the
merge". The live `protect-fork-rails` ruleset was read directly:

```json
{"bypass":[],"checks":["Governance Root","Fork CI Gate","Security Gate","Nix Gate"]}
```

`bypass_actors` is empty, so no actor can satisfy Governance Root while these
commits are present. The only mechanical paths forward are:

1. **`gh pr merge 90 --admin`** — administrator override of an active branch
   protection rule. This is an authority escalation beyond "merge the PR" and
   is **not** something to do unasked.
2. **The recorded ruleset maintenance procedure** (R07 `design.md` §4), which is
   the transaction-bound path the workflow comment itself points to for
   legitimate governance-path changes.
3. **Split the branch**: land the non-governance work first, then handle the two
   scope-pin commits through (1) or (2). This does not dissolve the loop — the
   pin commit still has to land somehow — but it reduces what rides on the
   override.

Option 3 is only a partial mitigation and is stated as such.

## Resolution: `--admin`, and why it was the lower-risk option

**Superseded. `--admin` was attempted with authorization and it FAILED.** See
"Correction: `--admin` does not work here" below. The risk comparison in this
section is still accurate; its conclusion is not actionable.

The user authorized the override explicitly, conditional on an independent
review agreeing it was warranted. That review (sheep) agreed, and reversed the
assumption in option 2 above. Recorded so the next reader sees *why* the flag
was warranted rather than only that it was used.

The R07 §4 maintenance window is **not** the safer-but-slower path here. It
works by PUTting a modified `protect-fork-rails` that drops `Governance Root`
from the required contexts, merging, then restoring it — its own comment says
"until it lands, `main` is unguarded", with a 5-attempt retry because the
restore must not fail.

```text
--admin      one merge, bypasses one FAILING check on ONE PR.
             The other 3 required checks are enforced and green.
             main's protection is never modified. No residue possible.
window.py    two ruleset writes; main is globally unguarded between them.
             A crash mid-window leaves main permanently weakened.
```

So the window has the strictly larger blast radius and a failure mode that
outlives the operation. Choosing it for the feel of procedure would have added
risk, not removed it. The judgement would invert if any of the other three
gates were red: `--admin` is defensible here *because* Governance Root is the
only failure and it is an audit flag correctly announcing a true fact (this PR
does edit protected paths).

Reviewer independence, stated rather than hidden: three of the five flagged
paths are sheep's own F30 commits riding in this integration branch, so it was
not a fully disinterested reviewer of two of those five files. The evidence it
gave is mechanical and independently reproducible, which is why this is
recorded as a caveat rather than treated as disqualifying.

## Correction: `--admin` does not work here

Attempted with the user's explicit authorization, at head `35c38febe`, with CI
fully settled at **15 pass / 1 fail** and the single failure confirmed to be
`Governance Root` at its `Detect governance-path changes` step. All three other
required contexts were green.

```text
$ gh pr merge 90 --merge --admin
GraphQL: Repository rule violations found
4 of 4 required status checks are expected. (mergePullRequest)

$ gh api -X PUT repos/jerudnik/jcode/pulls/90/merge -f merge_method=merge
HTTP 405  Repository rule violations found
```

The account holds `{"admin": true, "maintain": true, ...}` on this repository,
so this is not a permissions shortfall. The cause is that `main` is protected by
**rulesets**, not classic branch protection:

```text
GET /repos/jerudnik/jcode/branches/main/protection  ->  404 Branch not protected
GET /repos/jerudnik/jcode/rulesets/18509013         ->  active, bypass_actors: []
```

Classic branch protection has an `enforce_admins` flag that `--admin` keys off.
Rulesets have no such concept: the *only* way to be exempt is to appear in
`bypass_actors`, which is empty. Admin permission grants the ability to **edit
the ruleset**, not to ignore it. So `--admin` cannot bypass this rule no matter
who runs it.

**This falsifies option 1 in the list above, and the recommendation built on
it.** Recorded rather than quietly retried, because "the override is available"
was an inference from the classic-branch-protection model that nobody had
tested against this repository's actual configuration. It is exactly the failure
this program is about: a plausible mechanism asserted without running it.

### What this leaves

The remaining paths are narrower than the earlier list implied:

1. **Add a bypass actor** to `protect-fork-rails` (e.g. repository admin or the
   owner), then merge normally. This is a durable weakening of the ruleset
   unless it is added and removed around the merge, at which point it is the
   R07 §4 window with different fields.
2. **The R07 §4 transaction-bound procedure**, which is now the *only* recorded
   path that was designed for this case. Its own text anticipates precisely this
   situation: "A subsequent legitimate change to `.github/workflows/**` [...]
   will make `Governance Root` red, and a red required context blocks the merge
   for that PR alone." It binds the transaction with an 8-step protocol
   (capture identity, hash the pre-change body, drop exactly one context, merge
   conditioned on `sha: head_sha`, prove the parents, restore the literal
   captured body, prove no other merge landed in the window, and confirm with
   two independent comparator runs).
3. **Dissolve the loop structurally** by moving `EXPECTED_FILE_COUNTS` out of
   the protected file, so this class of PR stops being a governance event. This
   does not unblock the current PR, since making that change is itself a
   protected-path edit.

The earlier framing of §4 as "the slower, more ceremonial option" was wrong in
both directions: it is not optional, and it is not merely ceremony. It is the
only designed mechanism, and its residual risk (another ordinary PR merging
during the window) is explicitly scoped and detected by step 7 rather than
ignored.

## Window attempt 1: the protocol worked, the merge still failed

Authorized by the user. Ran the §4 protocol as a single script with every
assertion enforced in code, at head `d8e04baa5`, base `8286131d9`, with CI
settled at 15 pass / 1 fail and the failing step confirmed as
`Detect governance-path changes`.

Steps 1-2 passed all preconditions, including the check that only
`Governance Root` was red. Step 3 dropped exactly one context and read it back:

```text
window_start = 2026-08-02T21:38:16Z
live contexts now = ['Fork CI Gate', 'Security Gate', 'Nix Gate']
```

Step 4 then failed, with a **different** message than the `--admin` attempt:

```text
PUT /repos/jerudnik/jcode/pulls/90/merge  {"sha": head_sha, "merge_method": "merge"}
HTTP 405  Repository rule violations found
3 of 3 required status checks are expected.
```

"3 of 3" rather than "4 of 4" proves the drop took effect. The protocol did its
job; something else blocks. Step 6 restored the literal captured body and the
sanitized hash matched step 2 exactly on the first attempt:

```text
restore sha256 = 43ba61a7a57ffded7a4276917192cdd6028f79d58755cae870cbb2df07494f2b  (== step 2)
window_end     = 2026-08-02T21:38:20Z
```

**Total window: 4 seconds**, and `main` ended with all four contexts required.
The `finally` block is what made that true: the merge raised, and the restore
ran anyway. Had it been sequential code after the merge call, `main` would have
been left with a context dropped.

### The real second blocker, and it is the answer to question (a)

```text
gh pr view 90 --json mergeStateStatus  ->  BEHIND
compare base...head  ->  ahead_by=55  behind_by=11
```

The branch was 11 commits behind `main`, because PR #86 merged earlier in the
session. `strict_required_status_checks_policy: true` therefore refuses the
merge until the branch absorbs those commits and **reruns the entire ~11 minute
pipeline**, even though the 11 commits are documentation-only and no gate result
can change.

This is the exact cost described in `W6_CI_FRICTION.md`, encountered as a
blocker rather than as a theory. It is worth recording that the two questions
the user asked turned out to be the same system: the governance loop made this
PR expensive to land, and `strict` then made every attempt to land it cost a
full pipeline.

Merged `main` in (clean, one docs file), re-ran the gates locally before
spending CI (critical-path self-test 32/32, the four ratchets report no
violations), and pushed `99678e8f3`.

## Not verified

- ~~I did not attempt `--admin`, and did not attempt the §4 maintenance
  procedure.~~ **Both were subsequently attempted with the user's
  authorization.** `--admin` is structurally unavailable under rulesets; the §4
  window executed correctly and was blocked by `strict` instead. See the two
  correction sections above.
- I did not verify whether the protected-path list *intends* to cover
  `check_critical_path_budget.py` for count refreshes specifically, or whether
  that inclusion is over-broad. The list treats "the checker's logic" and "the
  checker's measured inventory" as the same asset; separating them (e.g.
  moving `EXPECTED_FILE_COUNTS` into a generated, unprotected data file with the
  digest still pinned) would dissolve the loop, but that is a governance design
  change and is out of scope for this PR.

## Follow-up this exposed

The loop is a symptom of `EXPECTED_FILE_COUNTS` being a **measured inventory**
stored inside a **protected policy file**. `fork-ci.yml` already draws the right
distinction for every other ratchet: the checkers are protected, but their
baselines (`swallowed_error_budget.json`, `code_size_budget.json`,
`panic_budget.json`, `test_size_budget.json`) are deliberately unprotected "so
that routine tightening needs no maintenance window". The critical-path budget
is the one gate that puts its baseline on the protected side, which is what
converts routine growth into a governance event. See `W6_CI_FRICTION.md`.
