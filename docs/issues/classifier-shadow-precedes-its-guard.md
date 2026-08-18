---
title: The PR routing classifier is shadowable, and the guard that detects shadowing runs downstream of it
status: open
priority: high
owner: unassigned
opened: 2026-08-18
---

# The PR routing classifier is shadowable, and the guard that detects shadowing runs downstream of it

## Summary

`.github/workflows/pr.yml` decides which checks a pull request runs by invoking
`python3 scripts/classify_pr_paths.py`. Python places a script's own directory
first on `sys.path`, so adding a file `scripts/<name>.py` for any module that
script imports rebinds that import for this invocation. A shadowed classifier
can print `docs_only=true`, which skips `Fork CI`, `Nix`, `Security` and
`Smoke`.

`scripts/check_guard_nonvacuity.py` already detects this class and rejects the
shadow. It runs from `just check`, inside `Fork CI` -- one of the legs the
shadowed classifier turns off. The control is real; it is sequenced after the
step it protects.

This is an ordering defect, not a missing mechanism. Nothing here is a claim
that any such pull request has been opened.

## What was verified

Reproduced against `scripts/classify_pr_paths.py` as it stands, using the real
base and head of merged PR #178 (`87c9355ff` -> `a10c9f9f0`, changing
`crates/jcode-app-core/src/agent_tests.rs` and one file under `docs/`). The
script was copied to a scratch directory; the repository was not modified.

| Arm | Tree | Interpreter | Output |
| --- | --- | --- | --- |
| 1 | clean | `python3` | `docs_only=false`, `product_impacting=true` |
| 2 | one added `subprocess.py` in the scripts directory | `python3` | `docs_only=true`, `product_impacting=false` |
| 3 | same file still present | `python3 -I` | `docs_only=false`, `product_impacting=true` |

Arm 3 is the control. The shadow file is still on disk and only the interpreter
flag differs, so the change in arm 2 is attributable to `sys.path[0]` rather
than to anything else about the tree.

The remaining links were read from the workflow definitions:

- `.github/workflows/ci.yml` gates `fork-ci`, `security` and `nix` on
  `if: ${{ !inputs.docs_only }}`, and `smoke` on
  `!inputs.docs_only && inputs.product_impacting`.
- `just check` -- the only invocation of the non-vacuity harness -- runs at
  `.github/workflows/fork-ci.yml:61`, inside the leg that gate controls.

And the last link was observed on a merged pull request rather than reasoned
about. PR #171 was genuinely docs-only. Its check runs conclude:

```
skipped | Checks / Fork CI
skipped | Checks / Nix
skipped | Checks / Security
skipped | Checks / Smoke
success | PR Gate
```

`PR Gate` requires `needs.checks.result = success`. A leg that skips satisfies
that, so the skipped and the passing outcome render identically.

## What was checked and found not to be the problem

Two narrower hypotheses were tested and refuted, and are recorded so they are
not re-investigated.

`classify_pr_paths.py` is not in the harness's guard registry, which suggested
its imports were uncovered. They are covered anyway, as collateral: every name
it imports -- `argparse`, `subprocess`, `sys`, `typing` -- is also imported by a
registered gating guard. Each was planted in turn as an empty
`scripts/<name>.py`; the harness exited 1 in all four cases, against `rc=0` and
`14 claim(s) hold` on the clean tree. Name coverage is not the gap.

The `-I` hardening recorded in D036 is correctly scoped to `justfile`, and every
`python3 scripts/...` line there does carry it. The classifier is invoked from a
workflow, not from `justfile`, so that hardening does not reach it. The D035
sentence describing bare `python3 scripts/...` invocations as exposed remains
accurate at the workflow call sites.

## Not verified

No pull request was opened to drive this end to end. Arms 1-3 and the PR #171
conclusions are measured; the two intervening links are read from the workflow
files. That is the distinction D035 records as its own residual gap: a guard
rejecting a defect in process is not the same as the gate rejecting a pull
request. Treat the chain as verified at both ends and read in the middle.

The blast radius is also unmeasured. `Governance Root` and `Docs lint` still
run, so a change to a protected path would still be caught; what is not
established is which unprotected changes could reach `main` unchecked.

## Mitigated, not fixed

The narrow mitigation has since been applied: the routing invocation in
`.github/workflows/pr.yml` now runs the classifier isolated, and
`tests/test_classify_pr_paths.py` asserts that it does and that it is the only
invocation. Re-running the three arms inside a real repository confirms it --
with the shadow file still on disk, the isolated interpreter prints the honest
verdict while the bare one prints the forged `docs_only=true`.

That closes this instance. It does not close the finding, which is why this
entry stays open:

- The detector is still sequenced after the step it protects. The non-vacuity
  harness runs from `just check` inside `fork-ci.yml`, a leg the routing step
  can still switch off. Only the routing step's own isolation stops it now, and
  that isolation is enforced by path protection and one test, not by the guard
  that exists for this class.
- The regression test lives downstream of the same routing decision, so it
  cannot catch a forged verdict at the moment it is made. It catches an edit to
  the workflow, which is separately protected anyway.
- Isolation was applied only to the routing invocation. Other workflows still
  invoke helper scripts bare. Those run inside gated jobs, so hardening them
  would not change routing; the reopen trigger below still covers them.

The general fix, unapplied: a control that decides whether checks run should be
verified before it is trusted, rather than by a job it can switch off.

## Reopen trigger

A new bare `python3 scripts/...` invocation in any workflow; a change to the
`docs_only` or `product_impacting` gates in `.github/workflows/ci.yml`; the
non-vacuity harness gaining a second call site or losing the one in
`fork-ci.yml`; or a classifier import whose name no registered gating guard
imports.

## Correction

The mitigation section originally read "Candidate mitigations, none applied".
That was accurate when the entry landed and stopped being accurate when the
isolation flag was applied. It is replaced above rather than deleted, since the
distinction it draws -- instance closed, ordering unchanged -- is the finding.

## Addendum, 2026-08-18: isolation does not stop it

The sentence above -- "Only the routing step's own isolation stops it now" --
is measured false. Isolation closed the shadow instance and nothing else. The
ordering is reachable by an ordinary edit, with `-I` in force and no shadow file
anywhere.

Two arms, the classifier copied to a scratch directory and the repository not
modified. The change set in both arms is one path, `scripts/classify_pr_paths.py`
itself.

| Arm | Classifier | Output |
| --- | --- | --- |
| 1 | unmodified | `docs_only=false`, `product_impacting=true` |
| 2 | `is_docs` relaxed to `return True`, one line | `docs_only=true`, `product_impacting=false` |

Arm 1 is the control and behaves as required. The routing workflow checks out
`head.sha`, so arm 2 is the copy that would run: a pull request weakening the
classifier is routed by the weakened classifier, onto the cheap path, away from
every leg that would judge it.

Nothing exotic is needed. The edit is honest Python, touches no protected path,
and so leaves `Governance Root` green -- that job fires on eight protected paths
and the decision log, and the classifier is on neither list.

## What the registration does, and does not, do

`scripts/classify_pr_paths.py` is now registered in the non-vacuity harness with
two gating claims: that it refuses to exempt its own change set, and that it
fails closed on an unreadable or unrecognised one. The wiring assertion names
the whole invocation, so dropping `-I` from the workflow now reddens the harness
rather than only the classifier's own test.

That is worth having for one reason: the property moves out of
`tests/test_classify_pr_paths.py`, which this repository does not protect and
which can be weakened in the same pull request as the thing it tests. D034 is
that failure mode.

It does not close this entry. The harness runs from `just check` inside
`fork-ci.yml`, which is still gated on `docs_only` -- the value arm 2 forges. A
detector downstream of the decision it checks cannot fire on the case that
matters, and registering a claim in it does not move it upstream. The general
fix recorded above is still unapplied.
