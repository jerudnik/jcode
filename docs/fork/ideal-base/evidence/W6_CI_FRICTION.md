# CI friction: why later PRs must rerun the whole pipeline

Raised by the user while W6 was blocked: *"later PRs need to be updated if they
didn't incorporate changes from an earlier PR. Seems kinda wasteful on CI to get
something approved, then need to rerun the whole pipeline just because an
approved change got merged in before."*

The observation is correct. The cause is one setting, and it is **not** the
governance gates.

## Cause

```text
repos/jerudnik/jcode/rulesets/protect-fork-rails
  strict_required_status_checks_policy = true
```

`strict` means "require branches to be up to date before merging". Every merge
to `main` invalidates the green checks on every other open PR, which must then
merge `main` in and rerun. With N ready PRs, landing them costs O(N^2) pipeline
runs. Measured cost of one full pipeline on this repo:

```text
build (x86_64-linux)   5m22s     Quality Guardrails   3m45s
fast validation        1m03s     Governance Contract  35s
secret scan            18s       Governance Root      15s
```

so roughly 11 minutes of wall time and four concurrent runners per rerun, for a
rebase that changed nothing about the PR's own content.

## Why the usual fix is unavailable

The standard answer is a **merge queue**: it batches PRs, tests each against the
projected post-merge state once, and keeps `strict`'s guarantee without the
O(N^2) reruns. Checked, and it does not apply here:

```text
repos/jerudnik/jcode  owner_type = User   private = false
```

GitHub restricts merge queues to public repositories **owned by an
organization**, or private repos on Enterprise Cloud. This repo is user-owned,
so the queue is not an option without transferring ownership to an org.

## What `strict` actually buys, and where it is redundant

`strict` exists to catch *semantic* conflicts: two PRs that merge cleanly at the
text level but break when combined. That is a real risk and worth something. But
the marginal protection is smaller here than it looks:

- `fork-ci.yml` already runs on `push: branches: [main]`, so the full gate set
  runs against the true merged state immediately after every merge. A semantic
  conflict is caught within minutes of landing, not hidden.
- `governance-root.yml` runs on `pull_request:` only, and it is merge-base
  relative, so it is unaffected by staleness in the way `strict` is meant to fix.

So `strict` mostly converts "detected on main within 11 minutes" into "detected
before merge, at the cost of rerunning every other open PR." With 2 open PRs
that is a modest tax; it grows quadratically with parallel agent work, which is
this repo's normal operating mode.

## Options

1. **Set `strict_required_status_checks_policy = false`.** One ruleset field.
   Keeps all four required checks; removes only the up-to-date requirement.
   Post-merge `fork-ci` on `main` remains the safety net. Reversible in one
   call, no code change, no new machinery.
2. **Transfer the repo to an organization and enable a merge queue.** Keeps
   `strict`'s guarantee and removes the reruns. Much larger change, affects
   remotes, permissions, and every recorded workflow reference.
3. **Leave it.** Correct while the number of concurrent PRs stays around two.

Recommendation is (1), and (2) only if PR concurrency grows enough that
pre-merge conflict detection is worth the ownership change.

## The separate half: the governance deadlock

This is a distinct problem that shares a symptom. `EXPECTED_FILE_COUNTS` is a
**measured inventory of the tree** stored inside a **protected policy file**, so
ordinary growth in `crates/jcode-tui/**` becomes a governance-path edit.

`fork-ci.yml` already states the correct principle for every other ratchet:

> ...but not their own baselines, which are deliberately unprotected so that
> routine tightening needs no maintenance window

and the split holds everywhere else:

```text
scripts/check_swallowed_error_budget.py   PROTECTED    (policy)
scripts/check_code_size_budget.py         PROTECTED    (policy)
scripts/check_critical_path_budget.py     PROTECTED    (policy)
scripts/swallowed_error_budget.json       unprotected  (baseline)
scripts/code_size_budget.json             unprotected  (baseline)
scripts/panic_budget.json                 unprotected  (baseline)
scripts/test_size_budget.json             unprotected  (baseline)
```

The critical-path budget is the only gate that keeps its baseline on the
protected side. Moving `EXPECTED_FILE_COUNTS` into a generated, unprotected data
file — with the digest still pinned, so weakening the *policy* still requires a
window — would dissolve the loop and make this gate consistent with its five
siblings.

**Not verified:** whether the protected-path list intends to cover inventory
refreshes, or whether that inclusion is over-broad. Answering it requires the
governance owner, not a measurement. Deliberately left as a proposal rather than
done inside a PR that is already blocked on protected-path edits, since fixing
it inside that PR would be the same category error.
