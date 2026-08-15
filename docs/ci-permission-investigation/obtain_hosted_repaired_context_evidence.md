# obtain_hosted_repaired_context_evidence

Node: `obtain_hosted_repaired_context_evidence`
Date: 2026-08-15 (UTC)
Status: complete

## The question

Does the "repaired governance context" actually work in a hosted GitHub Actions
run? Concretely: does `secrets.RULESET_AUDIT_TOKEN` let the live leg of
`scripts/fork-health.sh --live` read ruleset `bypass_actors` (exit 0), or does
it fail closed (exit 2)? The node's premise was that a green hosted run proves
the token still reads `bypass_actors`. I verified that premise against the
actual run instead of assuming it.

## What I ran

All commands were read-only; the guard's designed issue-creation did not
trigger, and no repository mutation was performed.

1. `gh run view 31891508962 --repo jerudnik/jcode`
2. `gh run view 31891508962 --repo jerudnik/jcode --json status,conclusion,... --jq ...`
3. `gh run view 31891508962 --repo jerudnik/jcode --log`
4. `git fetch --prune --tags github main`
5. `scripts/fork-health.sh --live` (local comparison)
6. `gh api repos/jerudnik/jcode/rulesets`
7. `gh api repos/jerudnik/jcode/rulesets/18509013` and `.../rulesets/18509016`
8. `gh api repos/jerudnik/jcode --jq '{permissions:..., ...}'`
9. `gh secret list --repo jerudnik/jcode` (names only, no values)
10. `gh issue list --state open --search 'in:title "Fork health"' --repo jerudnik/jcode`

Run URL: https://github.com/jerudnik/jcode/actions/runs/31891508962
Job: `verify rail invariants` (id 95028341330)
Trigger: `workflow_dispatch`, head `main` @ `fad729f51359f676b13eb1a98836221050b8f813`,
created `2026-08-15T15:00:42Z`.

## The conclusion

**The hosted run is a false green. The repaired governance context does not
work.** The live governance leg did not succeed: it failed with exit 2
(`You are not logged into any GitHub hosts`). `secrets.RULESET_AUDIT_TOKEN` is
not defined on the repository (only `CACHIX_AUTH_TOKEN` exists), so `GH_TOKEN`
is empty in the hosted check step, `gh auth status` fails, and the comparator
fails closed exactly as designed. The job nevertheless reports `success`
because the workflow line

```yaml
scripts/fork-health.sh --live --repo "$GITHUB_REPOSITORY" --fork-remote origin | tee health.txt
```

runs under `bash -e` without `pipefail`, so the pipeline's exit status is
`tee`'s 0 and the script's exit 2 is discarded.

Answer to the node's question 4: **No.** `RULESET_AUDIT_TOKEN` does not read
`bypass_actors`; it is absent, so the live leg fails (exit 2) rather than
succeeding. The "live leg succeeding proves it" premise does not hold because
the live leg did not succeed.

The local `--live` run exits 0 and matches the manifest, which proves the
comparator, the manifest, and the live rulesets are consistent. The only
hosted failure is the missing secret (compounded by the `| tee` exit-code
masking, which turned a failure into a green run). A side effect: because the
step "succeeded", the `if: failure()` drift-issue step was skipped and the
`if: success()` "Close the drift issue on success" step ran on a false green
(it found no open drift issue to close).

## Evidence

### Hosted run conclusion (overall exit 0)

```json
{"conclusion":"success","createdAt":"2026-08-15T15:00:42Z","event":"workflow_dispatch",
 "headBranch":"main","headSha":"fad729f51359f676b13eb1a98836221050b8f813",
 "jobs":[{"conclusion":"success","name":"verify rail invariants","status":"completed"}],
 "name":"Fork Health","status":"completed","updatedAt":"2026-08-15T15:01:11Z"}
```

### Hosted log, the "Run fork health check" step

```text
shell: /usr/bin/bash -e {0}
env:
  GH_TOKEN:                     <- empty: secrets.RULESET_AUDIT_TOKEN is undefined
...
=== Fork health: jerudnik/jcode (governance source: live) ===
OK:   fork-point (631935dd1d3b) is an ancestor of main
OK:   docs/BRANCHING.md documents every workflow
OK:   no Windows CI jobs (issue #19)
ERROR: live governance acquisition failed at endpoint: gh auth status
      You are not logged into any GitHub hosts. To log in, run: gh auth login
error: governance comparison could not be completed (exit 2)
```

Contrast: the neighboring step that uses `github.token` shows a masked,
non-empty value:

```text
Close the drift issue on success
env:
  GH_TOKEN: ***
```

### Secret inventory (names only)

```text
CACHIX_AUTH_TOKEN	2026-05-29T15:51:33Z
```

No `RULESET_AUDIT_TOKEN` exists.

### Local run (exit 0)

```text
=== Fork health: jerudnik/jcode (governance source: live) ===
OK:   fork-point (631935dd1d3b) is an ancestor of main
OK:   docs/BRANCHING.md documents every workflow
OK:   no Windows CI jobs (issue #19)
OK:   repository id 1238606714 matches the manifest
OK:   repository merge methods are merge-commit only
OK:   effective rules on 'main' are exactly ['deletion', 'non_fast_forward', 'pull_request', 'required_status_checks']
OK:   classic branch protection is absent
OK:   maintained rail 'main' is present
OK:   'Governance Root' at .github/workflows/governance-root.yml enforces exactly the 5 protected path(s) the manifest declares
OK:   required context 'Governance Root' is uniquely defined at .github/workflows/governance-root.yml:governance-root
OK:   required context 'PR Gate' is uniquely defined at .github/workflows/pr.yml:pr-gate
NOTE: protected-path additions are adjudicated; enforcing 5 paths
=== Governance: snapshot matches the manifest ===
OK:   governance snapshot matches scripts/required-checks.json
INFO: main payload: 1141 commit(s) over fork-point

=== Fork health: all invariants hold ===
```

Local auth (this credential CAN read `bypass_actors`):

```text
github.com
  ✓ Logged in to github.com account jerudnik (keyring)
  - Token scopes: 'gist', 'read:org', 'repo', 'workflow'
```

### Live ruleset state

Index (`gh api repos/jerudnik/jcode/rulesets`):

```text
[{"id":18509016,"name":"no-stray-branches","target":"branch","source_type":"Repository",
  "source":"jerudnik/jcode","enforcement":"active"},
 {"id":18509013,"name":"protect-fork-rails","target":"branch","source_type":"Repository",
  "source":"jerudnik/jcode","enforcement":"active"}]
```

`protect-fork-rails` (18509013): `enforcement: active`, `bypass_actors: []`,
`current_user_can_bypass: "never"`, conditions include `refs/heads/main`, rules
`deletion`, `non_fast_forward`, `pull_request` (merge only), and
`required_status_checks` pinned to `Governance Root` and `PR Gate`
(integration_id 15368).

`no-stray-branches` (18509016): `enforcement: active`, `bypass_actors: []`,
`current_user_can_bypass: "never"`, conditions include `~ALL` excluding
`refs/heads/main` and `refs/heads/automation/**`, single rule `creation`.

Repo metadata:

```text
{"allow_merge_commit":true,"allow_rebase_merge":false,"allow_squash_merge":false,
 "default_branch":"main",
 "permissions":{"admin":true,"maintain":true,"pull":true,"push":true,"triage":true},
 "private":false,"visibility":"public"}
```

Open fork-health drift issues: `[]` (none).

## Remaining unknowns

1. Where `RULESET_AUDIT_TOKEN` was supposed to live. It is not a repo secret;
   it may exist at the org level or may have been deleted. Repair requires
   defining a repo (or org) secret with a token whose ruleset permissions cause
   GitHub to return `bypass_actors`; the exact scope/expiry is a maintainer
   decision.
2. The `| tee` exit-code masking is a second, independent bug. Even after the
   secret is restored, an acquisition failure (exit 2) will still report green
   unless the pipeline is made fail-safe (`set -o pipefail`, or explicitly
   capturing `${PIPESTATUS[0]}`).
3. Whether earlier scheduled runs silently reported green with the same
   failure. Only the one dispatched run was inspected.
