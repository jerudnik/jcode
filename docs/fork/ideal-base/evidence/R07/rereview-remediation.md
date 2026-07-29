# R07 Remediation Targeted Re-Review

**Reviewer role:** adversarial re-verifier (independent of coordinator and of the prior Opus review)
**Repo:** jerudnik/jcode (independent hard fork)
**Branch under test:** `automation/r07-integration`
**Tip commit tested:** `4b1c9545df728b4fa34e62084fba8e5cab0b06db` (`4b1c9545d`)
**Pre-remediation control commit:** `dbcd83be0` (R07 barrier 0)
**Remediation commits:** `f428adffe`, `4b1c9545d`
**Live `origin/main` at review time:** `498249777c453c1d551aeb01fc45420d8ca0a585`
**Method:** detached worktree at `/tmp/r07-rereview/tree`, plus a standalone synthetic repo at `/tmp/r07-rereview/attack` for the attack reproduction. No pushes, no GitHub write operations. Two read-only `gh api` GETs.

**Overall verdict: REMEDIATION-PASS**

All three blocking gaps reproduce as CLOSED under adversarial test, each with a working pre-remediation control that demonstrates the gap was real and that the fix (not the test harness) is what closes it. One candidate new gap was investigated to ground and dismissed with evidence; it is recorded below as an observation, not a blocker.

---

## G1 (BYPASS) — CLOSED

**Claim:** the comparator, its tests, the fixture generator, and the fixture were unprotected and never run in CI; now all four are in an enforced 27-path protected set replicated across four artifacts, and fork-ci's `governance-contract` job runs the comparator tests.

### (a) All four artifacts name an identical 27-path set

Extracted each set mechanically and compared as sets, normalizing trailing slashes (`.github/scripts/` vs `.github/scripts`, which is a cosmetic difference in the apply-doc and seq-6 forms only):

```
required-checks.json  protected_paths.required        27 paths
apply-doc template_variables.protected_paths          27 paths   -> MATCH (normalized)
apply-doc sequence 6 git-diff assertion               27 paths, 0 duplicates -> MATCH
workflow patch governance-root.yml protected=( )      27 paths   -> MATCH (exact)
```

Set-symmetric-difference against `required-checks.json` was empty for all three others. The four target artifacts are present in the set:

```
scripts/governance_compare.py
tests/test_governance_compare.py
scripts/generate_governance_fixture.py
docs/fork/ideal-base/evidence/R07/fixtures/governance-valid.json
```

The sequence-6 assertion covers **exactly** 27 paths, no more and no less: `len(paths) == 27`, duplicate count 0, and the normalized set equals the manifest set both ways. This answers the "no more no less" question affirmatively.

`CrossArtifactCoherenceTests::test_protected_set_is_coherent_across_artifacts` exists at `tests/test_governance_compare.py:588` and enforces this, plus asserts `proposed_additions == []` and `additions_adjudicated == True`, plus pins the absence of the five ratchet baselines in all three surfaces.

### (b) Attack reproduction

Built a standalone git repo containing the base tree plus the patched `governance-root.yml`, branched, and applied a minimal malicious edit that neuters the bypass-actor comparison (the single highest-value check, since `bypass_actors: []` is what makes the ruleset non-circumventable):

```python
-    if canonical(actual_bypass) != canonical(expected["bypass_actors"]):
+    if False and canonical(actual_bypass) != canonical(expected["bypass_actors"]):
```

Ran the **exact** detection step extracted from the patched `governance-root.yml` (only substitution: `refs/remotes/origin/main` -> local `main`, since the synthetic repo has no remote):

```
::error::governance paths changed; use the recorded ruleset maintenance procedure (design.md section 4)
scripts/governance_compare.py
EXITCODE=1
```

**The attack is flagged.** Attack commit `9f019f0343077e24e2b87000ab860020a5d3a3fd`.

**Pre-remediation control.** The patch at `dbcd83be0` mentions `governance_compare.py` zero times, and mentions the fixture only as a `--fixture` CLI argument, never as a protected path:

```
$ git show dbcd83be0:.../workflow-contexts.proposed.patch | grep -c "governance_compare.py"
0
```

So the same edit would have passed the pre-remediation gate. The gap was real; the remediation is what closes it.

**Self-neutering variant (my own addition, not requested).** An attacker who, in the same PR, also deletes `scripts/governance_compare.py` from the workflow's own `protected=( )` array still fails, because `.github/workflows` is itself protected:

```
::error::governance paths changed; ...
.github/workflows/governance-root.yml
scripts/governance_compare.py
EXITCODE=1
```

**fork-ci runs the comparator tests.** The patch's `governance-contract` job contains `python3 -m unittest tests.test_governance_compare` (verified by string search on the patch text), alongside `tests.test_ideal_base_railway`, the railway check, and fixture-mode `fork-health.sh`.

### (c) Comparator tests are load-bearing, not decorative

```
$ pytest tests/test_governance_compare.py -q
74 passed, 13 subtests passed in 23.69s
```

Matches the claimed count exactly. To prove the suite is not vacuous, I applied the same neutering edit **in the real worktree** and re-ran:

```
FAILED RulesetMutationTests::test_unexpected_bypass_actor_on_main_ruleset
FAILED RulesetMutationTests::test_unexpected_bypass_actor_on_non_main_ruleset
FAILED LiveModeTests::test_mutated_live_surface_is_observed_red
3 failed, 71 passed
```

So G1 is closed by two independent mechanisms: the path is in the protected diff array (PR-time detection), and the tests that would catch a semantic neutering are actually executed in CI. Comparator restored to pristine afterward; `git status` clean.

**G1: CLOSED.**

---

## G2 (LOCKOUT) — CLOSED

**Claim:** patched tree lacked a `docs/BRANCHING.md` CI-table row for `governance-root.yml`, failing fork-health invariant 2; row now added.

Reproduced the exact procedure requested: detached worktree at `4b1c9545d`, fetched `origin main` (`498249777c`), applied the patch unmodified, regenerated nothing, ran fixture-mode fork-health.

```
$ git apply docs/fork/ideal-base/evidence/R07/workflow-contexts.proposed.patch
$ bash scripts/fork-health.sh --fixture .../fixtures/governance-valid.json --fork-remote origin

=== Fork health: jerudnik/jcode (governance source: fixture) ===
OK:   fork-point (631935dd1d3b) is an ancestor of main
OK:   docs/BRANCHING.md documents every workflow
OK:   no Windows CI jobs (issue #19)
OK:   repository id 1238606714 matches the manifest
OK:   repository merge methods are merge-commit only
OK:   effective rules on 'main' are exactly ['deletion', 'non_fast_forward', 'pull_request', 'required_status_checks']
OK:   classic branch protection is absent
OK:   maintained rail 'main' is present
OK:   required context 'Governance Root' is uniquely defined at .github/workflows/governance-root.yml:governance-root
OK:   required context 'Fork CI Gate' is uniquely defined at .github/workflows/fork-ci.yml:fork-ci-gate
OK:   required context 'Security Gate' is uniquely defined at .github/workflows/security.yml:security-gate
OK:   required context 'Nix Gate' is uniquely defined at .github/workflows/nix.yml:nix-gate
NOTE: protected-path additions are adjudicated; enforcing 27 paths
=== Governance: snapshot matches the manifest ===
OK:   governance snapshot matches scripts/required-checks.json
INFO: main payload: 600 commit(s) over fork-point

=== Fork health: all invariants hold ===
EXITCODE=0
```

"all invariants hold", exit 0, invariant 2 green, and the enforced count is confirmed as 27 by the tool itself.

**Control that the row is load-bearing.** Removing just that line from `docs/BRANCHING.md` and re-running:

```
FAIL: workflows missing from the docs/BRANCHING.md CI table:
=== Fork health: 1 invariant violation(s) ===
EXIT=1
```

The added row is exactly what flips the invariant. Restored afterward.

Also ran `actionlint` on all five patched/added workflows: exit 0, no findings.

**G2: CLOSED.**

---

## G3 (FALSE DURABILITY) — CLOSED

**Claim:** live ruleset carries `required_reviewers: []` but every expected body omitted it, guaranteeing permanently-red live comparisons; now pinned in three artifacts plus the regenerated fixture.

### (a) All artifacts pin the key

```
docs/fork/ideal-base/evidence/R07/design.md:295              "required_reviewers": []
scripts/required-checks.json:169                             "required_reviewers": []
docs/.../github-governance.proposed.json:238                 "required_reviewers": []
docs/.../fixtures/governance-valid.json:64                   "required_reviewers": []
```

All four present and identical in shape (`[]`).

### Live-ruleset cross-check (read-only)

`gh` auth works. Two read-only GETs:

```
$ gh api repos/jerudnik/jcode/rulesets/18509013
{"id":18509013,"name":"protect-fork-rails",...,"enforcement":"active",
 "rules":[{"type":"deletion"}], "bypass_actors":[], "current_user_can_bypass":"never", ...}

$ gh api repos/jerudnik/jcode/rules/branches/main
[{"type":"deletion","ruleset_id":18509013}]
```

**Important scoping note, stated plainly:** the live ruleset today carries only a `deletion` rule. The `pull_request` rule that would carry `required_reviewers` **has not been applied yet** (that is what apply-doc sequence 7 does). So the live surface cannot presently confirm or refute the `required_reviewers: []` shape by direct observation. The prior review's G3 finding was about the *post-apply* comparison going permanently red, and that is what I verified instead, by reproducing the failure mechanically in both directions. I did not take the coordinator's word for the live shape, and I am flagging that it is currently unobservable rather than claiming a confirmation I do not have. `bypass_actors: []` on the live ruleset *is* directly confirmed and matches the manifest.

### (b) Mechanical reproduction of the failure and the fix

Baseline (post-fix, fixture in the live-shaped form with the key present):

```
BASELINE (required_reviewers: [] present)  -> exit 0
```

**Original G3 direction reproduced** (expected/manifest side omits the key, live side has it, exactly the pre-fix condition):

```
PRE-FIX manifest (key removed) vs live-shaped fixture (key present): exit 1
   FAIL: ruleset 'protect-fork-rails' rule 'pull_request' has unexpected parameter 'required_reviewers'
```

Opposite direction, confirming the comparison is exact-equality on parameters rather than a lenient subset (so the pin is meaningful and not merely tolerated):

```
SIMULATED (live omits the key) -> exit 1
   FAIL: ruleset 'protect-fork-rails' rule 'pull_request' is missing parameter 'required_reviewers'
```

The comparator is strict in both directions. With the pin in place the comparison is green; without it, red. That is the guaranteed-red-forever condition the prior review identified, and it is now resolved.

### Fixture-mode comparator run

Green end to end, as shown in the G2 transcript: `=== Governance: snapshot matches the manifest ===`, exit 0.

**G3: CLOSED.**

---

## NEW-GAPS SECTION

I probed four specific regression vectors introduced by the remediation. Three came back clean. The fourth is a real but non-blocking observation, reported with its mitigation.

### 1. Fixture reproducibility — CLEAN

Regenerated the fixture from the current manifest plus the patched workflows and diffed against the committed artifact:

```
$ python3 scripts/generate_governance_fixture.py --workflows-dir .github/workflows --output /tmp/regen.json
$ diff <(json.tool regen.json) <(json.tool governance-valid.json)
FIXTURE REPRODUCIBLE: identical
```

Byte-identical. The regenerated fixture is not stale and not hand-edited. Additionally, all 10 workflow texts embedded in the fixture are byte-identical to what `git apply` of the patch produces on disk.

### 2. Sequence-6 coverage precision — CLEAN

Exactly 27 paths, zero duplicates, set-equal to the manifest in both directions. No over-coverage (which would create false lockouts) and no under-coverage (which would create bypasses).

Note on rigor: `CrossArtifactCoherenceTests` uses substring containment (`assertIn`) for the seq-6 and workflow-text checks rather than exact set equality, so it would not catch an *extra* path added to seq-6 alone. I verified exact equality myself by parsing, and it holds today. This is a minor test-strength observation, not a live defect.

### 3. Test premise repair kept the test load-bearing — CLEAN

The repair swapped the synthetic pending addition from `scripts/governance_compare.py` to `scripts/panic_budget.json`. The swap was *necessary*, not cosmetic: now that `governance_compare.py` is protected and named in the workflow, using it as the synthetic addition would make the test pass for the wrong reason. Direct evidence, flipping `additions_adjudicated` on both candidates:

```
scripts/panic_budget.json      adjudicated=False -> exit 0   (pending, green)
scripts/panic_budget.json      adjudicated=True  -> exit 1   (enforced, red)   <- load-bearing
scripts/governance_compare.py  adjudicated=False -> exit 0
scripts/governance_compare.py  adjudicated=True  -> exit 0   <- would be VACUOUS
```

The old premise would have gone silently vacuous post-remediation. The repair is correct and the flag remains load-bearing (0 -> 1 on flip). `scripts/panic_budget.json` exists in the tree (so the schema check passes) and is deliberately unprotected (so enforcement fails), which is precisely the property the test needs.

### 4. OBSERVATION (non-blocking): the patch file itself is not in the protected set

`docs/fork/ideal-base/evidence/R07/workflow-contexts.proposed.patch` is not one of the 27 protected paths, and no test reads it. I confirmed that editing the patch alone leaves all 74 tests green.

I pursued this to ground rather than reporting it as a finding, and it does **not** constitute an exploitable gap:

- A naive tamper is self-corrupting: removing a line without fixing the hunk header yields `error: corrupt patch at ...:70`, so `git apply` refuses it.
- A carefully well-formed tamper (line removed *and* hunk header corrected from `+1,65` to `+1,64`) does apply. But the resulting on-disk workflow then no longer matches the committed fixture, and fixture regeneration diverges: `Files regen2.json and governance-valid.json differ`.
- Disk-reading (live) comparator mode catches the tampered workflow directly: `FAIL: 'Governance Root' at .github/workflows/governance-root.yml does not name protected path(s) ['scripts/governance_compare.py']`.
- Once the patch lands, `.github/workflows` is protected, so the post-apply state is governed regardless of how it was produced.
- The patch is a coordinator-owned one-shot apply artifact, explicitly documented as such at `tests/test_governance_compare.py:788` and `scripts/generate_governance_fixture.py:16`. It is not a durable enforcement surface.

The residual exposure is a one-shot window where an operator applies a tampered patch *and* skips fixture regeneration *and* skips live-mode verification. The apply procedure's sequence-6 assertion plus the post-apply fixture check both close it. I judge this acceptable and record it only so the coordinator can decide whether to add the patch to the protected set as belt-and-braces after apply.

**Blocking new gaps found: none.**

---

## Validation performed

| Check | Result |
|---|---|
| `pytest tests/test_governance_compare.py` @ 4b1c9545d clean | 74 passed, 13 subtests |
| `pytest tests/test_ideal_base_railway.py tests/test_nix_distribution_policy.py` | 34 passed, 11858 subtests |
| `actionlint` on 5 patched/added workflows | exit 0, no findings |
| `git apply` of the patch onto 4b1c9545d | clean, no fuzz |
| Fixture regeneration diff | byte-identical |
| Fixture workflow text vs patched disk (10 files) | byte-identical |
| `fork-health.sh --fixture` on patched tree | all invariants hold, exit 0 |
| Attack reproduction (neutered comparator) | flagged, exit 1 |
| Self-neutering attack variant | flagged, exit 1 |
| Pre-remediation control (`dbcd83be0` patch) | comparator absent from protected set, gap confirmed real |
| G2 control (row removed) | 1 invariant violation, exit 1 |
| G3 both-direction parameter comparison | strict, red without the pin |
| `additions_adjudicated` flip discrimination | 0 -> 1, load-bearing |
| Live `gh api` reads (read-only) | 2 GETs, no writes |

Worktree left clean at `4b1c9545df728b4fa34e62084fba8e5cab0b06db`; all scratch mutations reverted and verified with `git status --short`.

## Verdict

| Gap | Verdict |
|---|---|
| G1 (bypass: unprotected comparator) | **CLOSED** |
| G2 (lockout: missing BRANCHING.md row) | **CLOSED** |
| G3 (false durability: `required_reviewers`) | **CLOSED** |
| New blocking gaps | **none found** |

**OVERALL: REMEDIATION-PASS**

Caveat carried forward, not a blocker: G3's live shape is currently unobservable because the `pull_request` rule is not yet applied to the live ruleset. The fix is verified correct by mechanical reproduction in both comparison directions and by internal consistency across all four artifacts. The first live comparison after apply-doc sequence 7 executes is the true confirmation, and the apply procedure should treat a `required_reviewers` mismatch there as the expected signal to re-check rather than as drift.
