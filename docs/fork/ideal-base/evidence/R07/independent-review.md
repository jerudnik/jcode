# R07 independent adversarial review

Date: 2026-07-28
Reviewer: independent (wrote none of the design, implementation, or gates)
Subject: `automation/r07-integration` at `2e6b13b73c90aed88d966fbf92ebed3f0387a27c`
Design baseline: `automation/r07-design` at `886f1a383d8eb5e7cd7d5bbb64f122a61e3cfbde`
Prior gate: `automation/r07-integration-gate` (`integration-gate.md`, PASS-WITH-FIXES)
Rubric: `WORK_GRAPH.json` `all_nodes[id=R07].acceptance_gates`, gate 6 —
"An independent reviewer reports no bypass, lockout, or false-durability gap"

## Verdict: **GAP-FOUND**

Three independent gaps, each reproduced against the actual integrated artifacts,
not inferred from prose:

| # | Class | Severity | Effect |
|---|---|---|---|
| G1 | **Bypass** | blocking | The judgement engine `scripts/governance_compare.py` is unprotected and un-run by any required context. A PR can make every governance comparison vacuously green while `Governance Root` stays green and `Fork CI Gate` stays green. |
| G2 | **Lockout** | blocking | Post-apply, `fork-health.sh` invariant 2 fails because `docs/BRANCHING.md` does not document the new `governance-root.yml`. That makes `Fork CI Gate` red on the bootstrap PR, contradicting barrier 3's stop condition and blocking the merge that the whole activation depends on. |
| G3 | **False durability** | blocking | GitHub's live API returns `pull_request.parameters.required_reviewers: []`, which the design's expected body, the manifest, and the fixture all omit. The comparator's exact-parameter rule turns this into a permanent false drift failure on every `--live` run and every read-back equality assertion, including sequence 8's mandatory-rollback trigger. Proved by a live round-trip of the design's exact sequence-7 body. |

The prior gate's required fix (F1) **did land completely and correctly**; that is
not the reason for this verdict. G1–G3 are new findings the prior gate explicitly
did not check.

---

## 1. Verification of the integration gate's required fix (F1) — PASS

The gate required three things. All three landed.

**(a) Apply doc `template_variables` and sequence-6 diff carry the full 23-path set.**
Independent parse of all four artifacts, normalizing trailing slashes:

```text
manifest scripts/required-checks.json protected_paths.required : 23
apply doc template_variables.protected_paths                    : 23
apply doc sequence-6 `git diff --quiet ... -- <paths>`          : 23
fixture governance-root.yml `protected=( ... )` array           : 23
required == template : True  (symmetric difference [])
required == seq6     : True  (symmetric difference [])
```

`proposed_additions` is `[]` and `additions_adjudicated` is `true`.

**(b) The coherence test is meaningful — I broke it three ways, it caught all three.**

| Planted one-sided edit | Result |
|---|---|
| Baseline, unmodified tree | `Ran 74 tests ... OK` |
| Remove `scripts/check_panic_budget.py` from apply-doc `template_variables` only | `FAILED (failures=1)` |
| Remove the same path from apply-doc sequence-6 diff command only | `FAILED (failures=1)`, message `sequence 6 diff assertion does not cover scripts/check_panic_budget.py` |
| Remove the same path from the fixture `governance-root.yml` text only | `FAILED (failures=6)` |

The test is not decorative: it pins set *equality* against the manifest for
`template_variables` (so both addition and removal fail) and membership for the
sequence-6 command and the workflow text. All planted edits were reverted; the
worktree is clean.

**(c) All 5 ratchet baselines absent everywhere.** `code_size_budget.json`,
`panic_budget.json`, `swallowed_error_budget.json`, `test_size_budget.json`,
`warning_budget.txt` appear in none of the four artifacts, and the coherence test
pins their absence, so re-adding one requires doing so in all artifacts at once.

F1 is closed. The adjudication record was also corrected to disclose the catch
rather than quietly overwriting the false claim, which is the right disposition.

---

## 2. G1 (bypass): the comparator is unprotected and untested in CI

### The claim under test

design.md:505-508 states the residual is bounded because a silent weakening
"would require the owner-admin to also alter the manifest, **the comparator**,
and the scheduled workflow consistently, **all of which are themselves protected
paths under `Governance Root`** and tracked in Git history."

`scripts/governance_compare.py:8-11` repeats the claim in its own docstring:
"R07 design.md section 4 protects 'the comparator'; that noun covers this file as
well as the shell entry point."

### The claim is false in the integrated artifacts

```text
scripts/fork-health.sh                : PROTECTED
scripts/ideal_base_railway.py         : PROTECTED
scripts/governance_compare.py         : UNPROTECTED   <-- 1122 lines, every judgement
tests/test_governance_compare.py      : UNPROTECTED   <-- 951 lines, the only coverage
scripts/generate_governance_fixture.py: UNPROTECTED
evidence/R07/fixtures/governance-valid.json : UNPROTECTED
```

`scripts/fork-health.sh` is a mode-selection shell wrapper. It delegates every
comparison to `python3 "$comparator"` and reduces the result to
`case "$governance_status" in 0) ok ...`. Protecting the wrapper while leaving
the engine unprotected protects the doorframe, not the door.

### Attack, executed

I applied the minimal attacker edit to `compare()` — return an empty `Report`
immediately — then ran the two detectors that are supposed to catch it:

```text
$ git diff --name-only HEAD -- <the 23 protected paths>
(empty)                     -> Governance Root would be GREEN

$ git diff --name-only HEAD
scripts/governance_compare.py

$ scripts/fork-health.sh --fixture .../governance-valid.json --fork-remote github
OK:   governance snapshot matches the manifest
=== Governance: snapshot matches the manifest ===
OK:   governance snapshot matches scripts/required-checks.json
=== Fork health: all invariants hold ===
exit=0                      -> Fork CI Gate would be GREEN
```

Both required contexts pass. Post-activation, this PR is mergeable with zero
required approvals. The daily `fork-health.yml --live` run then also reports
green forever, against an unobserved server state. That is precisely the
"degrades to a warning ... green for an unobserved state" failure mode
`fork-health.sh:26-30` says R07 exists to forbid, reintroduced one directory
level down.

### Why nothing else catches it

`tests/test_governance_compare.py` would fail loudly. It is never executed by any
required context:

```text
$ grep -rn "test_governance_compare" .github/ flake.nix workflow-contexts.proposed.patch
>>> NO CI INVOCATION FOUND <<<
```

The `governance-contract` job added by the patch runs
`python3 -m unittest tests.test_ideal_base_railway` and
`scripts/fork-health.sh --fixture`, but never the comparator's own suite. The
`quality` job's only unittest invocation is
`unittest discover -s tests -p 'test_rust_production_filter.py'`, a single-file
pattern. So the 951-line suite that is the entire evidentiary basis for
"fixtures reject missing PR enforcement, wrong approval count, non-merge methods,
force-push permission, stale rails, and unexpected bypass actors"
(acceptance gate 3) runs only on a developer's laptop.

This is the same defect class Stream G itself identified and the coordinator
adjudicated for 16 other paths — "edit the gate script, not the workflow that
calls it" — applied to the one script whose neutering disables *all* governance
detection at once, rather than one budget check. Stream G's own proposal listed
21 paths and did not include the comparator; the design's §4 list did not include
it either, while §5's prose asserted it was covered. The manifest, not the prose,
is what executes.

### Also unprotected: the fixture and its generator

`governance-valid.json` is the sole input to the only governance check wired into
a required context. It is unprotected, so it can be edited freely. Editing it to
match a weakened manifest keeps `Fork CI Gate` green. (The manifest *is*
protected, so this needs a second step, but the fixture arm is free.)

Separately, the fixture is **generated from the manifest**
(`generate_governance_fixture.py:37`: `rulesets = [copy.deepcopy(body) for _, body
in sorted(manifest["rulesets"].items())]`). Fixture-mode comparison therefore
compares the manifest to a copy of itself for every ruleset field. It is a real
test of the workflow-contract half and structurally vacuous for the ruleset half.
That is not itself a bypass, but it means fixture-mode green carries no
information about ruleset shape, which matters for G3 below.

### Minimum remediation

Add `scripts/governance_compare.py`, `tests/test_governance_compare.py`,
`scripts/generate_governance_fixture.py`, and
`docs/fork/ideal-base/evidence/R07/fixtures/governance-valid.json` to
`protected_paths.required` (and thus to all four coherent artifacts), and add
`python3 -m unittest tests.test_governance_compare` to the `governance-contract`
job. Either change alone is insufficient: protection without execution leaves a
neutered comparator merged through the §4 ceremony undetected; execution without
protection leaves the test file itself editable in the same PR.

---

## 3. G2 (lockout): the bootstrap PR cannot go green

### Mechanism

`scripts/fork-health.sh:130-141` (invariant 2) iterates every
`.github/workflows/*.yml` and fails if `docs/BRANCHING.md` does not contain its
name in backticks. `workflow-contexts.proposed.patch` creates a brand-new
workflow, `.github/workflows/governance-root.yml`, and does **not** touch
`docs/BRANCHING.md`. `docs/BRANCHING.md` is not a protected path, so nothing
forces the two to move together.

### Reproduced end to end

Applied the patch to a scratch tree from the integration tip:

```text
$ git apply --check workflow-contexts.proposed.patch   -> OK
$ git apply workflow-contexts.proposed.patch           -> APPLIED
$ for wf in .github/workflows/*.yml; do ... done
UNDOCUMENTED: governance-root.yml
```

Then ran the real script against the real post-apply workflow set:

```text
=== Fork health: jerudnik/jcode (governance source: fixture) ===
OK:   fork-point (631935dd1d3b) is an ancestor of main
FAIL: workflows missing from the docs/BRANCHING.md CI table:
      governance-root.yml
OK:   no Windows CI jobs (issue #19)
... (all governance comparisons OK) ...
=== Fork health: 1 invariant violation(s) ===
EXIT=1
```

### Why this is a barrier-3 stop, not a nuisance

The `governance-contract` job runs exactly this command
(`fork-ci.yml`, patch lines 84-93) and is a hard `needs:` dependency of
`Fork CI Gate`, whose gate script does `require_success "Governance Contract
Gate"`. So on the bootstrap PR:

- `Governance Root` — red (expected; it changes governance paths).
- `Fork CI Gate` — **red** (unexpected).

design.md:836-841, barrier 3, says: confirm all four contexts are emitted, that
`Fork CI Gate`, `Security Gate`, and `Nix Gate` **are green**, and that
`Governance Root` is red. A red `Fork CI Gate` is an unmet precondition for
barrier 4 (bootstrap merge), which is the precondition for requiring the contexts
at all. Followed literally, the operator stops and the activation does not
proceed.

### Severity and shape

This is a **self-healing lockout**, not a permanent one: `docs/BRANCHING.md` is
unprotected, so the operator can add one table row to the bootstrap PR and clear
it. But:

1. It is a **pre-activation** blocker discovered at the worst moment — mid-apply,
   after the archive push (barrier 1) has already executed as an irreversible
   external write.
2. It converts a clean, reviewable barrier-3 observation into an
   improvise-under-pressure moment, which is exactly the state in which an
   operator is most likely to rationalize past a red context rather than stop.
3. It was reachable by running one existing script against the patched tree, and
   four adversarial gates plus the integration gate did not run it. The
   integration gate ran `actionlint` on the patched workflows and
   `fork-health.sh --fixture` on the *unpatched* tree, never the two together.

Remediation is one line in `docs/BRANCHING.md`'s CI table, landed in the same
bootstrap PR as the patch, plus a note in barrier 2's checklist.

---

## 4. G3 (false durability): live GitHub returns a parameter the design forbids

### Live round-trip, executed

The gates never wrote the design's target ruleset body to a live surface. I did,
safely: `POST repos/jerudnik/jcode/rulesets` with the **exact sequence-7 body**
from `github-governance.proposed.json`, altered only in name
(`r07-review-probe-DELETE-ME`), `enforcement: disabled`, and ref condition
retargeted to a nonexistent branch `refs/heads/zzz-r07-probe-nonexistent`.

```text
POST -> created id 19933729, enforcement disabled
GET  -> read back
DELETE -> exit 0; rulesets now exactly: no-stray-branches, protect-fork-rails
```

GitHub **accepted** the body verbatim — good news for the design: all four rule
types round-trip, `bypass_actors` comes back exactly `[]`,
`allowed_merge_methods: ["merge"]`, `required_approving_review_count: 0`,
`required_review_thread_resolution: true`, and
`strict_required_status_checks_policy: true` are all preserved. `deletion`,
`non_fast_forward` and `required_status_checks` matched byte-for-byte after
volatile-key sanitization.

But `pull_request` did not:

```text
  pull_request: DIFF
    live: {"allowed_merge_methods":["merge"], ..., "required_review_thread_resolution":true,
           "required_reviewers":[]}
    want: {"allowed_merge_methods":["merge"], ..., "required_review_thread_resolution":true}
```

GitHub injects `required_reviewers: []` into the response. It is not in the
design's expected body (design.md:280-291), not in the manifest
(`required-checks.json:154-163`), not in the fixture, and not in `VOLATILE_KEYS`
(`governance_compare.py:55-67`).

### Consequences, both proved

**(a) The comparator goes permanently red on live mode.** `_compare_rule_parameters`
iterates `set(expected) | set(actual)` and fails on any key not in `expected`.
Injecting exactly the observed live shape into the fixture:

```text
exit 1
FAIL: ruleset 'protect-fork-rails' rule 'pull_request' has unexpected parameter 'required_reviewers'
=== Governance: 1 mismatch(es) against the manifest ===
```

So after a successful apply, every `fork-health.sh --live` run and the scheduled
daily `fork-health.yml --live` run exit non-zero and open a drift issue —
reporting drift that does not exist. A drift detector that cries wolf every day
is a drift detector that gets muted, which is the same false-durability outcome
as one that never fires.

**(b) Sequence 8 may fire a spurious mandatory rollback.** Sequence 8's assertion
is "sanitized body **exactly equals** sequence 7, including ... every rule
parameter", and its `on_mismatch` is an unconditional
"immediately PUT rollback_body ... and stop." If the operator applies that
assertion as literally as it is written, the very first read-back after the very
first write mismatches on `required_reviewers`, triggering a rollback and halting
activation. If instead the operator waves it through as "obviously benign", the
strictness that makes sequence 8 the TOCTOU guard is gone on the one occasion it
matters.

### Why every prior gate missed it

The fixture is generated *from* the manifest, so fixture-mode can never surface a
manifest-vs-GitHub divergence in ruleset bodies (§2 above). Live mode was only
ever run **pre-apply**, where the integration gate observed "exit 1 with the
expected 14 pre-apply governance mismatches" — a 15th, structural mismatch is
indistinguishable from the expected pre-apply noise. Nobody wrote the target body
to a live surface. The design's own D029-style rule ("the drift detector must be
observed red before it is trusted") has an unstated dual that R07 never satisfied:
the detector must also be observed **green against a real post-apply surface**.

### Remediation

Either add `required_reviewers: []` to the expected `pull_request` parameters in
the manifest, the design body, the apply doc's sequence-7 body, and the fixture
(preferred: it is the server's actual state, and pinning it empty is a real
assertion), or add it to `VOLATILE_KEYS` (weaker: it silently stops asserting
that no reviewers are pinned). Embedded sequence hashes for sequences 3-5 are
unaffected — those are pre-write baselines of the *current* ruleset, which has no
`pull_request` rule.

---

## 5. Bootstrap sequence, barriers 0-7 — reviewed, one lockout (G2)

Traced every barrier against the actual artifacts.

**Passing structure.** Step sequences are contiguous 1-17; sequences 1-6 are
reads and assertions with the first write at 7; the ordering rule "never require a
context before its definition is on `refs/heads/main` and has been observed
emitting" is honored by barriers 2-4; sequence 6 re-verifies current main
immediately before the first write and correctly handles renames in both
directions via the path-limited `git diff`; sequence 8 read-back plus repeat of
sequence 6 has a mandatory, hash-verified rollback; the classic-protection DELETE
is checkpoint-blocked until the new ruleset, effective main rules, no-stray
ruleset, and merge methods all verify. The half-execution risk is genuinely
covered: the only interval where one surface is changed and another is not is
between sequence 7 and 8, and that is exactly what the sequence-3-body rollback
restores.

**Lockout analysis.** No state in which a *required* context cannot be emitted.
All four contexts have `if: always() && github.event_name == 'pull_request'` or an
unconditional `pull_request:` trigger, the patch removes `nix.yml`'s workflow-level
`paths:` filter (the one genuine lockout source), and the comparator actively
fails any required context whose workflow carries `paths:`/`paths-ignore:`
(`governance_compare.py:820-828`) with the correct reasoning ("the branch would be
permanently unmergeable"). That is the right control and it is real.

The lockout that exists is not context-emission but context-*success*: G2 makes an
always-emitted `Fork CI Gate` deterministically red at the moment barrier 3
requires it green. Emission is proved; greenness is not.

**Half-execution.** One residual the design does not enumerate: barrier 1 (archive
push, 39 refs) executes **before** barrier 2, and is an irreversible external
write. If activation later halts at barrier 3 or 4 — which G2 makes likely — the
archive is already pushed while governance is unchanged. That is not unsafe
(the archive is additive, private, verified, and moves/deletes nothing), but the
ordering means the first irreversible step happens before the first step that can
reveal an artifact defect. Worth stating explicitly in the operator checklist.

---

## 6. STATE schema-v2 migration — PASS, no loss, matches §8

Independent diff of design-tip v1 (`886f1a383`) against the integrated v2:

```text
top-level keys              : identical sets
node ID sets                : equal (57/57)
per-record field changes    : NONE (state, evidence, summary, updated_at all byte-identical)
extra keys beyond the split : NONE
reviewed_commit prefix check: 0 mismatches (every value extends its v1 abbreviated `commit`)
last_checkpoint             : {commit} -> {reviewed_commit, published_commit}, lossless
```

Validator semantics versus design §8's five numbered steps
(`ideal_base_railway.py:513-590`):

| §8 step | Implementation | Verdict |
|---|---|---|
| 1. reviewed object existence | `git_commit_object_exists` | present, and named *existence*, not reachability |
| 2. published object existence | `git_commit_object_exists` | present |
| 3. explicit published ref resolution | `git_ref_resolves(published_ref)`, hoisted to a whole-validator precondition | present |
| 4. non-shallow assertion | `git_repository_is_shallow()` -> hard error | present, **no `allow_shallow` escape hatch** anywhere in the file |
| 5. ancestry | `git merge-base --is-ancestor` | present, with the correct comment that step 5, not step 2, is what proves publication |

Also correct: `existence != reachability` is enforced by naming
(`git_commit_object_exists` vs `git_commit_is_ancestor`, with a docstring on the
former pointing at the latter); non-terminal records must have `null` for both
identities, closing the "ancestry-scanning tool thinks an incomplete node is
published" hole; `--commit` is gone in favor of `--reviewed-commit` /
`--published-commit`; CI depth is enforced by
`test "$(git rev-parse --is-shallow-repository)" = false` in the
`governance-contract` job *before* the validator runs.

**One observation, not a gap.** `validate_state` still dispatches to
`_validate_state_v1` when `schema_version == 1`. I confirmed a full downgrade of
the live `STATE.json` to v1 passes `ideal_base_railway.py check` with exit 0 and
"protected hash intact", losing all ancestry semantics. This is *caught* by
`tests/test_ideal_base_railway.py::test_live_state_json_is_schema_v2_and_validates`
(I verified: `FAILED (failures=1)` on the downgraded file), and that suite **is**
wired into the `governance-contract` required job, and both the validator and its
test are protected paths. So the downgrade path is closed by a protected,
CI-executed test rather than by the validator itself. That is adequate, and it is
notably the exact defense-in-depth pairing that G1 shows is *missing* for the
comparator. Retaining v1 for historical replay is defensible; I flag it only so
the asymmetry is deliberate rather than accidental.

---

## 7. Accepted residual risks — accurately stated, with two caveats

| Residual | Stated | Actual | Verdict |
|---|---|---|---|
| TOCTOU after sequence-8 repeat | design.md:410-418: PUT has no expected-SHA precondition, "a smaller residual race remains after the repeat and before the apply finishes"; explicitly does not claim the platform serializes it | Matches the artifact. Sequence 8 does read-back then full sequence-6 repeat with a fresh live main SHA | **accurate** |
| Detect-not-prevent maintenance window | design.md:420-450, §4 transaction-bound: PR/head/base capture, exact pre-change body + hash, literal restoration, `rev-list --first-parent --merges` proof, pre/post `--live` transcripts | Matches. Notably stronger than "detect only" | **accurate** |
| Protected paths cannot cover transitive deps | stream-g proposal, "Adjudication notes" 2: "closes one hole, not the class... a sufficiently determined change can still route around any fixed list" | True, and honestly stated. **But** G1 is not a transitive-dependency escape — the comparator is the direct, first-order engine, named in the design prose as protected. G1 is not covered by this accepted residual | **accurate as written; does not absolve G1** |
| Archive push not yet executed | `ls-remote` shows zero refs in both managed namespaces | Confirmed: `refs/heads/archive/reviewed/*` and `refs/tags/archive/stash-*` return 0 lines. All 39 manifest objects exist locally (0 missing). Target repo confirmed `private: true, fork: false` | **accurate** |
| Owner-admin is the root of trust (D031) | design.md:922-930: R07 is self-*checking*, not self-*protecting*; ruleset changes are caught only by audit | Accurate as a statement about *ruleset* changes. **Silently worse than stated** for *repository-content* changes: §5 claims the comparator is protected and it is not, so the audit arm D031 substitutes for the missing trust root is itself editable through a normal PR (G1) | **one claim is worse than stated** |

The only residual that is worse than documented is the §5 sentence
"...the comparator... all of which are themselves protected paths under
`Governance Root`". That sentence is the load-bearing justification for accepting
D031, and it is false against the integrated manifest.

---

## 8. Items prior gates said they did not check — what I checked

| Prior gate's stated blind spot | My action |
|---|---|
| "live four-context emission still unobserved" | Still unobserved. Not checkable without opening the bootstrap PR, which is an external write outside my authorization. Structurally reviewed instead: all four triggers/`if:` conditions verified emission-safe, and the comparator actively rejects `paths:` filters on required contexts |
| "steady-state ruleset body never round-tripped through a live write" | **Checked. Found G3.** Live POST/GET/DELETE round-trip of the exact sequence-7 body on a disabled ruleset targeting a nonexistent branch |
| "did not prove the archive repository is private" | **Checked.** `gh api repos/jerudnik/jcode-recovery-archive` -> `private: true, visibility: private, fork: false` |
| "did not execute the atomic push / post-write fsck" | Not done (external write, out of scope). Verified pre-state instead: both managed namespaces empty, 39/39 source objects present locally |
| "tests never compare either artifact to the apply document" | Now they do (F1's coherence test) — and I broke it three ways to confirm it works |
| Not stated by any gate | **Found G1** by asking which files the *engine* consists of rather than which the design says are protected |
| Not stated by any gate | **Found G2** by running the existing `fork-health.sh` against the *patched* tree; every prior gate ran it against the unpatched tree |

Also re-confirmed independently: `gh api repos/jerudnik/jcode` returns
`id: 1238606714` (matches manifest) and current merge methods
`merge/squash/rebase` all `true` (matches the design's stated diff-from-current);
both ruleset ids `18509013`/`18509016` resolve to the expected names with
`enforcement: active`; live `no-stray-branches` rules are `[{"type":"creation"}]`
as recon recorded. Both required suites pass unmodified at the claimed counts
(74/74 governance, 25/25 state).

---

## 9. Attacks walked

1. **Neuter a check script** (`scripts/check_panic_budget.py`, the design's own
   example) — **defeated**. It is one of the 16 adjudicated additions, present in
   all four artifacts; `Governance Root` fires on the PR and names the path.
2. **Neuter the comparator** (`scripts/governance_compare.py`) — **succeeds**.
   G1. Both required contexts green, comparison vacuous.
3. **Edit the fixture to match a weakened state** — **succeeds** on the fixture
   arm; the fixture is unprotected and is the sole input to the only governance
   check in a required context.
4. **Downgrade `STATE.json` to schema v1** to escape ancestry semantics —
   **defeated**, by a protected, CI-executed test (not by the validator).
5. **One-sided protected-path edit** across the four artifacts — **defeated** by
   F1's coherence test, verified in all three directions.
6. **Rename a protected path out of its prefix** — **defeated** by design;
   `git diff` path-limiting reports the deletion of the old protected path.
7. **Merge an unrelated PR between sequence 6 and sequence 7** — **defeated for
   protected paths** by the sequence-8 repeat plus mandatory rollback; the
   residual race is accurately documented.

---

## 10. Validation performed

- `python3 -m unittest tests.test_governance_compare` — 74/74 OK (baseline),
  and deliberately failed 3× under planted one-sided edits.
- `python3 -m unittest tests.test_ideal_base_railway` — 25/25 OK (baseline),
  and deliberately failed 1× under a planted schema-v1 downgrade.
- `python3 scripts/ideal_base_railway.py check --published-ref 498249777...` — OK.
- `git apply --check` + real apply of `workflow-contexts.proposed.patch` to a
  scratch tree from the integration tip — both succeeded.
- `scripts/fork-health.sh --fixture` on the unpatched tree — exit 0; on the
  **patched** tree — exit 1 (G2).
- Independent four-way parse of the protected-path set — 23/23/23/23, equal.
- Independent v1-vs-v2 `STATE.json` field-level diff — 0 losses.
- Live read-only `gh api`: repository, both rulesets, archive repo visibility,
  `ls-remote` on both managed archive namespaces.
- One live write probe: disabled ruleset, nonexistent target branch, immediately
  deleted, output sanitized. Token supplied inline from `rbw`, never echoed,
  stored, or committed. Post-probe ruleset list verified to contain exactly the
  two pre-existing rulesets.
- Every planted mutation reverted; `git status --short` clean before commit.

---

## 11. What I did not check

- No bootstrap PR was opened, no context was observed emitting live, no merge,
  no ruleset mutation on `18509013`/`18509016`, no classic-protection deletion,
  no archive push. All are external writes outside my authorization.
- I did not re-derive the 26 unique patch IDs or the 3 merge-payload mappings in
  the state ledger; the integration gate reproduced those and I found no reason
  to doubt it. I independently re-verified the migration's losslessness and all
  ancestry-relevant properties.
- I did not verify the `RULESET_AUDIT_TOKEN` secret exists in repository settings.
  The patch makes `fork-health.yml --live` depend on it; if it is unset, the
  scheduled run exits 2 daily. Worth confirming before barrier 2.
- I did not test whether GitHub injects response-only fields into the
  `no-stray-branches` `creation` rule (it currently returns bare
  `[{"type":"creation"}]`, so the G3 class likely does not apply there, but I did
  not round-trip that body).
- I did not run the Rust, Nix, or preflight suites; the integration surface is
  Python, JSON, shell, workflow text, and evidence.
- I did not audit whether any of the 23 protected paths has a transitive Python
  import outside the protected set (the accepted-residual class).

---

## 12. Confidence

**High, 96%.**

G1, G2, and G3 were each reproduced by executing the actual integrated artifacts
and observing the exit codes and messages quoted above, not by reading prose.
G3 additionally rests on a live GitHub API response for the design's exact target
body. The 4% reflects the barriers I could not execute (live four-context
emission, the real apply sequence), where a further gap could exist that only
execution would reveal.

The three gaps are all bounded and coordinator-fixable without redesign:
four manifest additions plus one CI line (G1), one documentation table row (G2),
and one parameter added to four expected bodies (G3). None invalidates the design's
architecture, the STATE migration, the archive plan, or F1's fix. But acceptance
gate 6 asks whether an independent reviewer finds **no** bypass, lockout, or
false-durability gap, and the honest answer is that there is one of each.

**Verdict: GAP-FOUND.** R07 should not execute barrier 0 until G1, G2, and G3 are
remediated and re-gated.
