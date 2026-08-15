# actionlint gap: `$`-prefixed contexts and the `.#actionlint` compatibility layer

## The question

Does GitHub Actions expression syntax (`${{ }}` and `$`-prefixed contexts) used in
this repo's `.github/workflows/*.yml` hit any actionlint grammar gap, i.e. a
construct actionlint misparses, rejects incorrectly, or silently skips?

## What I checked

1. Inventoried all 14 workflows under `.github/workflows/` (13 linted by CI, plus
   the exempt `freebsd-smoke.yml`) for expression and `$`-prefixed context usage.
2. Ran actionlint exactly the way CI does: `nix run .#actionlint --` over the 12
   workflows listed in `.github/workflows/nix.yml:48-60`.
3. Re-ran the same files through upstream, unpatched `nixpkgs#actionlint` 1.7.12
   and through the flake-locked `.#actionlint` 1.7.12 to isolate parser differences.
4. Read the compatibility patch, its fixture, the flake `workflow-syntax` check, and
   the two Python checkers that CI runs immediately after actionlint.

Environment: Determinate Nix 3.20.0 on darwin/arm64. `.#actionlint` reports
"1.7.12, built with go1.26.3 compiler for darwin/arm64". actionlint was not on
`PATH`; both runs were done through Nix.

## Expression inventory (production workflows)

Contexts actually used, with representative locations:

- `github.*` — `github.ref` (main.yml:8, pr.yml:8, release.yml:24, scheduled.yml:9),
  `github.event.*` (pr.yml:20/25/31/32, docs-impact.yml:9/28/29), `github.token`
  (security.yml:50, scheduled.yml:55, nix-update.yml:59, release.yml:144,
  fork-health.yml:52/70), `github.run_id`/`github.server_url`/`github.repository`
  (security.yml:68, fork-health.yml:55/74), `github.workflow` (docs-impact.yml:9),
  `github.event_name` (nix.yml:100).
- `secrets.*` — `secrets.CACHIX_AUTH_TOKEN` (nix.yml:116),
  `secrets.RULESET_AUDIT_TOKEN` (fork-health.yml:38).
- `needs.*` — `needs.classify.result`, `needs.checks.result`,
  `needs.classify.outputs.docs_only` (pr.yml:51/60/61).
- `inputs.*` — `inputs.docs_only` (ci.yml:17/31/39/48/56), `inputs.broad`
  (fork-ci.yml:57), `inputs.strict`/`inputs.weekly` (security.yml:36/40/47),
  `inputs.release`/`inputs.rollback_ref` (release.yml:53/54), `inputs.publish`
  (nix.yml:100).
- `matrix.*` — `matrix.system` (nix.yml:77/92/93).
- `vars.*` — `vars.JCODE_PREVIOUS_RELEASE_REF` (release.yml:55).
- `steps.*.outputs.*` — `steps.route.outputs.docs_only` (pr.yml:20),
  `steps.release.outputs.*` (release.yml, many).
- Functions/operators — `!inputs.strict` (security.yml:40), `!inputs.docs_only`
  (ci.yml), `inputs.publish || github.event_name == 'push'` (nix.yml:100),
  `needs.classify.outputs.docs_only == 'true'` (pr.yml:51).

No `fromJSON`, `toJSON`, or `env.*` context appears anywhere. All same-repository
reusable-workflow calls use the `./` prefix (`uses: ./.github/workflows/...`);
**no production workflow uses the `$/` prefix.**

## The conclusion

The current production workflows hit **no** actionlint grammar gap: the flake-locked
`.#actionlint` exits 0 with zero findings over all 12 linted workflows.

However, upstream actionlint 1.7.12 (unpatched `nixpkgs#actionlint`) has three real
grammar/table gaps on exactly this "dollar + compat" surface, and this repo already
patches all three in its flake-locked `.#actionlint`:

1. **`$/path` same-repository reusable call — false positive.** Upstream rejects the
   valid GitHub syntax; the patch makes it parse like `./`.
2. **`$/path@ref` — silent false negative.** Upstream accepts a ref on a same-repo
   call (it reads `$` as an owner and `.github` as a repo); the patch rejects it.
3. **`code-quality` and `vulnerability-alerts` permission scopes — false positive.**
   Upstream calls both "unknown permission scope"; the patch adds them to the
   permission table.

None of the three is triggered by today's workflows (no `$/` call, no
`code-quality`/`vulnerability-alerts` in production). They are pre-emptive:
`flake.nix` pins a patched actionlint and CI asserts fail-closed behavior with a
fixture and negative tests, so a future workflow that uses these constructs is
validated correctly instead of misparsed.

## Evidence

### Gap 1 — upstream rejects valid `$/` (false positive)

Fixture `tests/fixtures/actionlint-dollar-local/.github/workflows/caller.yml:10`:

```yaml
    uses: $/.github/workflows/called.yaml
```

Upstream `nixpkgs#actionlint` 1.7.12 output (exit 1):

```text
.github/workflows/caller.yml:10:11: reusable workflow call "$/.github/workflows/called.yaml" at "uses" is not following the format "owner/repo/path/to/workflow.yml@ref" nor "./path/to/workflow.yml". see https://docs.github.com/en/actions/learn-github-actions/reusing-workflows for more details [workflow-call]
```

The same file passes under the flake `.#actionlint` (exit 0), and the required
input/secret/output of the `$/-called` workflow are validated.

### Gap 2 — upstream accepts `$/...@ref` (silent false negative)

`sed 's|called.yaml|called.yaml@main|'` on the fixture, then upstream actionlint:
exit 0, no diagnostic. The patched actionlint rejects it (exit 1):

```text
reusable workflow call "$/.github/workflows/called.yaml@main" at "uses" must not specify a ref for a same-repository $/ workflow [workflow-call]
```

The fix is in `nix/actionlint-dollar-local-workflows.patch`:

- `rule_workflow_call.go`: adds an early `$/` + `@ref` rejection, and changes the
  `./` prefix checks to also accept `$/`.
- `reusable_workflow.go`: resolves `$/path` like `./path` so inputs, secrets, and
  outputs of the called local workflow are validated instead of skipped.

`flake.nix:225-238` applies this patch and, in the same override, adds the two
permission scopes to `rule_permissions.go` via `--replace-fail`.

### Gap 3 — upstream rejects `code-quality` / `vulnerability-alerts` (false positive)

Fixture equivalent to `flake.nix:391-396` (`supported.yml` with `code-quality:
write`, `vulnerability-alerts: read`, plus `models: read`, `repository-projects:
write`). Upstream actionlint (exit 1):

```text
unknown permission scope "code-quality". all available permission scopes are ...
unknown permission scope "vulnerability-alerts". all available permission scopes are ...
```

The flake `.#actionlint` accepts both (exit 0). `scripts/check_workflow_permissions.py:23-26`
documents this: actionlint 1.7.12 and current main omit both scopes even though
GitHub accepts them.

### How CI enforces the compatibility layer

`.github/workflows/nix.yml:44-66` runs `nix run .#actionlint -- <12 workflows>` then
the two Python checkers. `flake.nix:349-470` (`workflow-syntax`) runs the same list
and then asserts fail-closed negatives from the fixture:

- missing required input (`flake.nix:420-427`),
- missing required secret (`flake.nix:429-436`),
- unknown `needs.*.outputs.*` property (`flake.nix:438-445`),
- `$/...@ref` local-ref rejection (`flake.nix:447-454`),
- `code-quality: admin` and `vulnerability-alerts: write` access-level rejection
  (`flake.nix:456-464`),
- unknown scope `future-scope` rejection (`flake.nix:466-469`).

`tests/test_nix_distribution_policy.py:405-425` additionally pins that CI uses
`nix run .#actionlint --` (not `nixpkgs#actionlint`) and that the patch stays narrow
and fail-closed. `scripts/check_reusable_workflow_calls.py:3-6` independently accepts
the `$/` prefix (`LOCAL_CALL_RE` at line 20: `^(?:\./|\$/)\.github/workflows/...`).

### Actionlint run result

```text
nix run .#actionlint -- \
  .github/workflows/ci.yml ... .github/workflows/governance-root.yml
# EXIT=0, no findings
```

## Remaining unknowns

- **`secrets.*` and `vars.*` names are never validated.** actionlint cannot know the
  configured secret/variable set at lint time, so a typo like `vars.MISSING` is
  silently accepted. This is a by-design silent skip, not a grammar misparse, and the
  compatibility patch does not (and cannot) address it. Example: `release.yml:55`
  `vars.JCODE_PREVIOUS_RELEASE_REF` is accepted without verifying the variable exists.
- **Verified against 1.7.12 only**, the flake pin. The repo's comments
  (`scripts/check_workflow_permissions.py:23-25`) claim the permission gaps persist
  "through current main (as of 2026-08-10)"; I did not build actionlint from upstream
  main to confirm the current-tip behavior.
- **Did not run the full `workflow-syntax` flake check** (heavy Nix derivation);
  I reproduced its actionlint invocations and negative cases directly instead.
- actionlint does not evaluate expression semantics (e.g. whether `needs.*`/`matrix.*`
  is actually available in a given step position); that is out of scope for a grammar
  gap review and was not tested.
