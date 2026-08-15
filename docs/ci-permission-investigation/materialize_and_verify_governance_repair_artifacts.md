# Materialize and verify governance repair artifacts

## Question

Do the repo's own governance repair artifacts, when run against the live GitHub
state of `jerudnik/jcode`, confirm the prior claim that the two live rulesets
(`no-stray-branches`, `protect-fork-rails`) match `scripts/required-checks.json`?
In scope: the comparator, the workflow-permission linter, and the actionlint
compatibility fixture negatives. Read-only `gh api` GETs and local analysis only;
nothing pushed, no workflows dispatched, no GitHub state modified.

## What I ran

### 1. Governance comparator (live)

`scripts/fork-health.sh` step 4 invokes the comparator as
`FORK_HEALTH_GH="$gh_bin" python3 "$comparator" --manifest "$manifest" --live
--workflows-dir "$repo_root/.github/workflows"` (lines 179-180), with
`FORK_HEALTH_GH` defaulting to `gh`. I ran the equivalent directly:

```sh
cd /Users/jrudnik/labs/jcode
python3 scripts/governance_compare.py --manifest scripts/required-checks.json \
  --live --workflows-dir .github/workflows
```

Live mode (`acquire_live`, `scripts/governance_compare.py:1013`) performs only
read-only GETs: `repos/{repo}`, `repos/{repo}/rulesets`,
`repos/{repo}/rulesets/{id}` (per ruleset), `repos/{repo}/rules/branches/main`,
`repos/{repo}/branches/main/protection` (404 allowed), and
`repos/{repo}/branches?per_page=100`. `gh auth status` was logged in as
`jerudnik` with scopes `gist`, `read:org`, `repo`, `workflow` (`repo` +
`workflow` are what let the read see ruleset `bypass_actors`).

### 2. Workflow-permission linter

`.github/workflows/nix.yml` (line 64) invokes it as:

```sh
cd /Users/jrudnik/labs/jcode
python3 scripts/check_workflow_permissions.py .
```

### 3. Actionlint fixture negatives

Reproduced the `workflow-syntax` check's negatives (`flake.nix:349-470`) by
running the flake-locked actionlint binary directly instead of the full
derivation:

```sh
FLAKE=/Users/jrudnik/labs/jcode
tmp=$(mktemp -d)
# positive: dollar-local reusable workflow fixture
cp -R tests/fixtures/actionlint-dollar-local "$tmp/valid"
mkdir "$tmp/valid/.git"
( cd "$tmp/valid" && nix run "$FLAKE#actionlint" -- \
  .github/workflows/caller.yml .github/workflows/called.yaml )   # expect exit 0

# positive: patched permission scopes accepted
cat > "$tmp/supported.yml" <<'EOF'
name: Supported permissions
on: push
permissions:
  code-quality: write
  vulnerability-alerts: read
  models: read
  repository-projects: write
jobs:
  valid:
    runs-on: ubuntu-latest
    steps:
      - run: 'true'
EOF
nix run "$FLAKE#actionlint" -- "$tmp/supported.yml"                    # expect exit 0

# negatives, generated exactly as flake.nix does
sed '/    with:/,+1d'    "$tmp/valid/.github/workflows/caller.yml" > "$tmp/missing-input.yml"
sed '/    secrets:/,+1d' "$tmp/valid/.github/workflows/caller.yml" > "$tmp/missing-secret.yml"
sed 's/needs.call.outputs.result/needs.call.outputs.unknown/' \
                         "$tmp/valid/.github/workflows/caller.yml" > "$tmp/unknown-output.yml"
sed 's|called.yaml|called.yaml@main|' \
                         "$tmp/valid/.github/workflows/caller.yml" > "$tmp/local-ref.yml"
sed 's/code-quality: write/code-quality: admin/' "$tmp/supported.yml" > "$tmp/invalid-access.yml"
sed 's/vulnerability-alerts: read/vulnerability-alerts: write/' \
                         "$tmp/supported.yml" > "$tmp/invalid-read-only-access.yml"
sed 's/code-quality: write/future-scope: read/' "$tmp/supported.yml" > "$tmp/unknown-scope.yml"
```

Each negative was run from its fixture root (with a `.git` sentinel present, as
the derivation does) and asserted to exit non-zero with the expected diagnostic.

## Conclusion

Verified. All three artifact families behave as the manifest claims:

- Comparator: exit 0, `=== Governance: snapshot matches the manifest ===`.
  Zero `FAIL:` lines, which covers both manifest rulesets (the ruleset
  comparison emits only failures, so silence = match) plus repository identity,
  merge methods, effective main rules, classic-protection absence, branch set,
  and both workflow contracts.
- Permission linter: exit 0, no findings.
- Actionlint: both positives accepted; all 7 negatives rejected with the exact
  diagnostics the derivation greps for.

## Evidence

Comparator output (verbatim):

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
EXIT=0
```

Permission linter: empty stdout/stderr, `EXIT=0`.

Actionlint negatives (all non-zero exit, exact expected text present):

```
.github/workflows/missing-input.yml:10:11: input "required-input" is required by "$/.github/workflows/called.yaml" reusable workflow [workflow-call]
.github/workflows/missing-secret.yml:10:11: secret "required-secret" is required by "$/.github/workflows/called.yaml" reusable workflow [workflow-call]
.github/workflows/unknown-output.yml:20:24: property "unknown" is not defined in object type {result: string} [expression]
.github/workflows/local-ref.yml:10:11: reusable workflow call "$/.github/workflows/called.yaml@main" at "uses" must not specify a ref for a same-repository $/ workflow [workflow-call]
invalid-access.yml:4:17: "admin" is invalid as permission of scope "code-quality". available values are "read", "write", "none" [permissions]
invalid-read-only-access.yml:5:25: "write" is invalid as permission of scope "vulnerability-alerts". available values are "read", "none" [permissions]
unknown-scope.yml:4:3: unknown permission scope "future-scope". all available permission scopes are "actions", "artifact-metadata", ... [permissions]
```

## Remaining unknowns

- The comparator compares sanitized desired state, not raw bytes:
  `sanitize()` (`governance_compare.py:334`) strips volatile server keys
  (`id`, `node_id`, `url`, `_links`, `created_at`, `updated_at`, etc.) before
  comparison. So "matches the manifest" here means desired-state equality after
  stripping server-generated fields. The earlier worker's "byte-for-byte"
  phrasing is strictly stronger than what the repo's own tooling asserts; I did
  not re-run a raw byte diff of the two ruleset bodies (out of scope, and the
  comparator is the authoritative repo gate).
- I ran the comparator directly, not the full `scripts/fork-health.sh --live`
  wrapper: its local invariants 1-3 need the `github/main` remote ref and the
  `fork-point` tag fetched, which are unrelated to the governance-artifact
  claim. The wrapper's governance step is the identical command I ran.
- I did not run the full `workflow-syntax` nix derivation (which additionally
  runs `check_reusable_workflow_calls.py`, its unit test, and
  `python3 -m unittest tests.test_workflow_permissions`), nor `nix flake check`,
  per the "run actionlint directly" instruction.
