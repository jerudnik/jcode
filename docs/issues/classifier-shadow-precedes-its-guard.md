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
| 2 | one added `scripts/subprocess.py` | `python3` | `docs_only=true`, `product_impacting=false` |
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

## Candidate mitigations, none applied

Adding `-I` to the classifier invocation in `.github/workflows/pr.yml` closes
arm 2 directly, and arm 3 is already evidence that it does. `.github/workflows`
is a protected path, so it needs the recorded ruleset maintenance procedure.

Ordering is the more general fix: any control that decides whether checks run
should be verified before it is trusted, rather than by a job it can switch off.

Neither is applied here. This entry records the finding.

## Reopen trigger

A new bare `python3 scripts/...` invocation in any workflow; a change to the
`docs_only` or `product_impacting` gates in `.github/workflows/ci.yml`; the
non-vacuity harness gaining a second call site or losing the one in
`fork-ci.yml`; or a classifier import whose name no registered gating guard
imports.
