# A parked PR fails when a workflow on main outgrows the script on the branch

## What happened

PR #219 sat open while five other PRs merged. It went from green to a
classifier failure without anyone touching it. Nothing was wrong with the
branch.

`pr.yml` on main had begun invoking
`scripts/check_docs_references.py --moved-from … --head …`. Workflow files run
from the merge base, so #219 ran main's newer workflow against its own older
copy of the script, which did not accept those flags. `gh pr update-branch 219`
fixed it with no code change.

## Why it matters

The failure names the classifier, not the skew, so it reads as a defect in the
parked branch. Diagnosing it means noticing that a file the branch never
touched changed underneath it.

The cost scales with how long PRs sit. `Governance Root` is a required check
that cannot run on a merge queue, so PRs serialize: roughly twelve minutes each
regardless of whether they are independent. During one burndown, six PRs
merged in sequence and the sixth had been open long enough to go stale.

Any workflow that calls a versioned script with arguments can do this. The
pairs are not enumerated anywhere, so there is no way to tell which parked PR
is at risk.

## What would fix it

A check that fails loudly and names the skew, rather than surfacing as a
confusing failure in an unrelated job. Options, roughly in order of cost:

- Have the workflow assert the script supports the flags it passes, and fail
  with a message that says the branch is stale and needs `update-branch`.
- Have workflows call scripts through a stable entry point, so argument changes
  do not cross the workflow-script boundary.
- Detect the general case: a workflow changed on main since the merge base and
  invokes a script the branch also carries.

## Evidence

- #219 head `cd273ca98`, blocked with a classifier failure; `73e8289e6` after
  `update-branch`, green with no content change.
- `.github/workflows/pr.yml` on main invokes `check_docs_references.py` with
  `--moved-from` and `--head`.
- Six PRs merged in sequence during the burndown: #220, #224, #222, #223, #221,
  #219.

## Resolution criteria

- A PR whose branch predates a workflow's new script arguments fails with a
  message naming the staleness, or does not fail at all.
- The check is exercised by a test that would catch its removal.
- The runbook note about this hazard is replaced by the check.
