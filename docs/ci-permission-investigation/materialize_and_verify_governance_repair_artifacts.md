# Materialize and verify governance repair artifacts

**Node:** `materialize_and_verify_governance_repair_artifacts`
**Date:** 2026-08-15
**Repo:** jerudnik/jcode (hard fork, `main` at `fad729f51`)

## The question

Do the four governance "repair artifacts" — the declarative manifest, the
live-vs-manifest comparator, the workflow permission linter, and the
actionlint compatibility fixture — stand up when the repo's own tools run
over the current tree? Earlier read-only `gh api` GETs confirmed the live
rulesets match the manifest byte-for-byte; this node reruns that claim
through the guard's own tooling and surfaces the actionlint fixture
positives and negatives that `flake.nix:349-470` exercises in CI.

## What I ran (exact commands)

All commands run from the repo root on 2026-08-15. Live `gh api` calls were
read-only `GET` requests, no mutations. The actionlint binary is the
flake-locked `.#actionlint` (pinned upstream 1.7.12 plus the
`nix/actionlint-dollar-local-workflows.patch` and the
`code-quality`/`vulnerability-alerts` permission-table patches), invoked
via `nix run .#actionlint --` so CI and the flake workflow-syntax check
agree on parser behavior.

```bash
# (1) Comparator, fixture mode (the mode tests use)
python3 scripts/generate_governance_fixture.py \
  --output target/fork-health/governance-valid.json
python3 scripts/governance_compare.py \
  --manifest scripts/required-checks.json \
  --snapshot target/fork-health/governance-valid.json

# (2) Comparator, live mode (GETs only)
python3 scripts/governance_compare.py \
  --manifest scripts/required-checks.json \
  --live --workflows-dir .github/workflows \
  --dump-snapshot target/fork-health/governance-snapshot.json

# (3) Whole fork-health guard, fixture mode
bash scripts/fork-health.sh --fixture target/fork-health/governance-valid.json

# (4) Workflow permission linter (the way nix.yml 'validate' runs it)
python3 scripts/check_workflow_permissions.py .
python3 -m unittest tests.test_workflow_permissions
python3 scripts/check_reusable_workflow_calls.py .

# (5) actionlint fixture — positive
nix run .#actionlint -- -version   # 1.7.12 + patch
cp -R tests/fixtures/actionlint-dollar-local /tmp/al-pos
mkdir /tmp/al-pos/.git
( cd /tmp/al-pos && nix run /Users/jrudnik/labs/jcode/.#actionlint -- \
    .github/workflows/caller.yml .github/workflows/called.yaml )

# (6) actionlint negatives — 4 dollar-local + 3 permission
# (each reproduces flake.nix:349-470 assertions under /tmp)
```

## The conclusion

Every artifact the comparator, the permission linter, and the actionlint
fixture are supposed to verify **stands up under the repo's own tools**.
Detector surfaces compared:

| Artifact                                            | Verified | Evidence                                          |
| --------------------------------------------------- | -------- | ------------------------------------------------- |
| `scripts/required-checks.json` manifest             | YES      | Comparator accepts fixture + live snapshot       |
| `scripts/governance_compare.py` comparator          | YES      | Exit 0 on fixture (gen→compare round-trip) and live |
| `scripts/check_workflow_permissions.py`              | YES      | Exit 0 over `.`, `tests.test_workflow_permissions` 30/30 OK |
| `scripts/check_reusable_workflow_calls.py`          | YES      | Exit 0 (companion gate, same evidence surface)   |
| `tests/fixtures/actionlint-dollar-local/` positive  | YES      | `actionlint` exits 0 on caller.yml + called.yaml |
| `flake.nix:349-470` actionlint negatives (4 + 3)    | YES      | All 7 mutated fixtures rejected with the expected diagnostic, exit 1 |

**No mismatches.** The earlier byte-for-byte `gh api` finding is reproduced
through the comparator: 2 rulesets (no-stray-branches, protect-fork-rails),
classic branch protection absent, effective rules on `main` exactly
`['deletion', 'non_fast_forward', 'pull_request', 'required_status_checks']`,
required contexts `Governance Root` and `PR Gate` uniquely defined, and
the audit gate's `protected=( ... )` array matches the manifest's 5-path
adjudicated set.

## Evidence (quoted outputs)

### Comparator (fixture mode)

```
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
```

### Comparator (live mode, read-only GETs)

Same OK lines as fixture mode. The dumped `target/fork-health/governance-snapshot.json`
contains 2 rulesets (`no-stray-branches`, `protect-fork-rails`), `classic_branch_protection: None`,
branches `['automation/ideal-base-final-signoff', 'automation/pr138-window-close',
'automation/s03-atomic-checkpoint', 'main']`, and `effective_main_rules` is the four-rule
list shown above (ruleset_id 18509013, source `jerudnik/jcode`).

### Whole fork-health guard

```
OK:   fork-point (631935dd1d3b) is an ancestor of main
OK:   docs/BRANCHING.md documents every workflow
OK:   no Windows CI jobs (issue #19)
OK:   governance snapshot matches scripts/required-checks.json
INFO: main payload: 1143 commit(s) over fork-point
=== Fork health: all invariants hold ===
```

### Workflow permission linter

```
$ python3 scripts/check_workflow_permissions.py . ; echo EXIT=$?
EXIT=0
$ python3 -m unittest tests.test_workflow_permissions
..............................
----------------------------------------------------------------------
Ran 30 tests in 0.070s
OK
$ python3 scripts/check_reusable_workflow_calls.py . ; echo EXIT=$?
EXIT=0
```

### actionlint fixture positive

```
$ nix run .#actionlint -- -version
1.7.12
installed by building from source
built with go1.26.3 compiler for darwin/arm64

( cd /tmp/al-pos && nix run .#actionlint -- \
    .github/workflows/caller.yml .github/workflows/called.yaml )
(no output, exit 0)
```

### actionlint negatives — 4 dollar-local (flake.nix:386-454)

| Mutated fixture         | Expected diagnostic (flake)                                                       | Actual exit |
| ----------------------- | --------------------------------------------------------------------------------- | ----------- |
| `missing-input.yml`     | `input "required-input" is required by "$/.github/workflows/called.yaml" reusable workflow` | 1 |
| `missing-secret.yml`    | `secret "required-secret" is required by "$/.github/workflows/called.yaml" reusable workflow` | 1 |
| `unknown-output.yml`    | `property "unknown" is not defined in object type`                                | 1 |
| `local-ref.yml`         | `reusable workflow call "$/.github/workflows/called.yaml@main" at "uses" must not specify a ref for a same-repository $/ workflow` | 1 |

### actionlint negatives — 3 permission (flake.nix:456-469)

| Mutated fixture                  | Expected diagnostic (flake)                                                    | Actual exit |
| -------------------------------- | ------------------------------------------------------------------------------ | ----------- |
| `invalid-access.yml`             | `"admin" is invalid as permission of scope "code-quality"`                     | 1 |
| `invalid-read-only-access.yml`   | `"write" is invalid as permission of scope "vulnerability-alerts"`             | 1 |
| `unknown-scope.yml`              | `unknown permission scope "future-scope"`                                      | 1 |

All seven negative fixtures reproduced the matching `actionlint` exit code
(1) and emitted the matching diagnostic text. The patched `rule_workflow_call.go`
adds the same-repository `$/` rejection that the flake expects, and the
permission-table patches add `code-quality` and `vulnerability-alerts` to
the accepted scope set so valid uses pass and out-of-band uses fail.

## Remaining unknowns

1. **`RULESET_AUDIT_TOKEN` credential scope and expiry.** The comparator
   fails closed when `bypass_actors` is absent from the ruleset body;
   `GITHUB_TOKEN` does not return `bypass_actors`. Live mode here
   succeeded only because the active `gh` session has a `repo` token
   (gho_********) whose scope is broader than `GITHUB_TOKEN`. When CI
   runs the guard with `secrets.RULESET_AUDIT_TOKEN`, the comparator will
   only pass if that PAT also retains the access. Needs a live CI run to
   confirm.
2. **Tag protection.** The two manifest rulesets target `refs/heads/main`
   and `refs/heads/automation/**` (creation block on `~ALL` excluding
   `main`/`automation/**`). No ruleset targets `refs/tags/*`, so the
   `fork-point` tag is not protected by an active ruleset — the
   `fork-health.yml` GUID guard is detection-only, with up to ~24 h lag.
   This is the gap called out in `reconcile_tag_guard_with_main_permission_ceiling.md`
   and is unchanged by this verification run.
3. **`default_workflow_permissions` ceiling.** Live `default_workflow_permissions`
   remains `write`; the comparator does not query it because no
   ruleset-level `workflows` rule is in the manifest, and the org-level
   `workflows` rule type is unavailable on user-owned repos. Out of scope
   for this artifact verification.
4. **Mutability of `secrets.*`/`vars.*` names.** Not a governance defect
   (actionlint cannot validate secret names by design), but worth noting
   that the verifier does not catch `vars.JCODE_PREVIOUS_RELEASE_REF`-style
   typos.
