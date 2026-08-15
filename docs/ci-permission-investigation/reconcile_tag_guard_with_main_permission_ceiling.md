# Tag guard vs. the main-branch permission ceiling

## The question

Does the tag-guard workflow's declared `permissions:` and `on:` triggers reconcile
cleanly against the "main-branch permission ceiling" (constrained `GITHUB_TOKEN`
on main, direct pushes to main blocked, `automation/*` prefix required for
pushes)? For each event root (push of a tag, push to main, `workflow_dispatch`,
`schedule`), does the guard's required permission exceed what the ceiling allows,
or does enforcement silently depend on a PAT rather than `GITHUB_TOKEN`?

## What I checked

1. Identified the tag-guard workflow. There is no workflow literally named
   "tag guard": the fork-point immutability invariant is enforced by
   `fork-health.yml` running `scripts/fork-health.sh` check 1.
2. Read all workflows that reference tags (`fork-health.yml`, `release.yml`,
   `nix.yml`, `governance-root.yml`), plus `scripts/fork-health.sh`,
   `scripts/governance_compare.py`, `scripts/check_workflow_permissions.py`,
   and `scripts/required-checks.json` (the canonical governance manifest).
3. Read the local ruleset evidence: `docs/BRANCHING.md` ruleset table,
   `docs/agent-workflows.md` CI section, and the R07 gate records
   (`design-gate.md`, `design-v2-gate.md`, `design.md`).
4. Read-only `gh api` GETs (no mutation): repo metadata, Actions workflow
   permission default, the ruleset index, and both ruleset bodies.

Live values observed this session: `default_workflow_permissions: "write"`,
`can_approve_pull_request_reviews: true`; exactly two rulesets, both
`target: branch`, `enforcement: active`, `bypass_actors: []`, repository-scoped.

## The conclusion

**No permission gap exists between the tag guard and the main-branch rulesets,
because the two operate on different layers and the guard asks for only
read + issue-write. The substantive gap is coverage, not permissions: no
tag-protection ruleset exists and the guard does not fire on tag push, so the
"immutable, must never move" `fork-point` tag is detection-only and lags by up
to one day.**

The "permission ceiling" is a ruleset-defined ceiling over *branch ref
operations*, not over `GITHUB_TOKEN`. The `workflows` ruleset rule type — the
only ruleset mechanism that could constrain what runs on main — is unavailable
on this user-owned repository (org/enterprise-scoped). The live Actions default
is `write`, not a read-only cap. So:

- **push of a tag** — the guard does not subscribe. No ruleset targets
  `refs/tags/*` at all. A `v*` tag fires `release.yml` (`contents: write`,
  `github.token` for a metadata-only GitHub release) and `nix.yml`
  (`contents: read`, `CACHIX_AUTH_TOKEN` for Cachix); `fork-point` matches no
  trigger, so moving or deleting it is unprevented and detected only at the next
  scheduled/dispatch run (up to ~24 h).
- **push to main** — the guard does not subscribe (`main.yml` runs instead,
  `contents: read`). This is fine for the ancestry invariant: direct/force push
  and deletion of main are already blocked by `protect-fork-rails`
  (`deletion` + `non_fast_forward` + `pull_request`), so main cannot be rewritten
  to shed `fork-point`.
- **`workflow_dispatch` and `schedule`** — the guard runs with
  `contents: read` (checkout + `git fetch --prune --tags origin main`) and
  `issues: write` (the drift issue). Both are `GITHUB_TOKEN` scopes and neither
  mutates refs, so nothing collides with the rulesets. The one scope that would
  break under a read-only default is `issues: write`; the live default is
  `write`, so no gap today.

The real external dependency is a PAT: `secrets.RULESET_AUDIT_TOKEN` feeds the
governance leg, because `GITHUB_TOKEN`'s ruleset read omits `bypass_actors` and
the comparator fails closed on that omission. The guard's fork-point check
itself (check 1) needs no token at all.

## Evidence

### The tag guard's triggers and permissions

`fork-health.yml:9-16`:

```yaml
on:
  schedule:
    - cron: "37 9 * * *"
  workflow_dispatch:

permissions:
  contents: read
  issues: write
```

- `fork-health.yml:33` `git fetch --prune --tags origin main` (read only).
- `fork-health.yml:38` `GH_TOKEN: ${{ secrets.RULESET_AUDIT_TOKEN }}` for the
  live governance read; `fork-health.yml:52,70` `github.token` for issue ops.
- Header comment `fork-health.yml:6-7`: the token is required "because GitHub
  omits `bypass_actors` from callers without ruleset write access."

The guard writes nothing but issues. `fork-health.sh:184-190`: live-acquisition
failure returns exit 2, which fails the job and trips the
`if: failure()` drift-issue step — fail closed, but the issue title still says
"rail invariant violation," conflating token failure with real drift.

### The fork-point check it guards

`scripts/fork-health.sh:108-124`:

```bash
if ! git rev-parse --verify --quiet "${fork_point_ref}^{commit}" >/dev/null; then
  usage_error "missing $fork_point_ref tag (fetch tags: git fetch --tags $fork_remote)"
fi
fork_point="$(git rev-parse "${fork_point_ref}^{commit}")"
...
if git merge-base --is-ancestor "$fork_point" "$fork_main"; then
  ok "$fork_point_ref (${fork_point:0:12}) is an ancestor of $main_branch"
else
  fail "$fork_point_ref ... is NOT an ancestor of $main_branch; the fork-touched gates are measuring against the wrong base"
fi
```

`docs/BRANCHING.md:15-25` states the tag "is immutable and must never be moved"
and that the check exists to make a silent measurement change "loud."

### No tag-protection ruleset exists

`scripts/required-checks.json` defines exactly two rulesets, both `target:
branch`:

- `protect-fork-rails` (`required-checks.json:69-121`): conditions
  `ref_name.include: ["refs/heads/main"]`; rules `deletion`,
  `non_fast_forward`, `pull_request` (merge-only, zero approvals, thread
  resolution), `required_status_checks` (`Governance Root`, `PR Gate`,
  `integration_id 15368`).
- `no-stray-branches` (`required-checks.json:122-143`): conditions `~ALL`
  excluding `refs/heads/main` and `refs/heads/automation/**`; rule `creation`.
  This is the `automation/*` prefix requirement.

`classic_branch_protection` is `"absent"` (`required-checks.json:38`). Nothing in
the manifest, and no live ruleset, targets `refs/tags/*`. Live `gh api
repos/jerudnik/jcode/rulesets` returned exactly these two, both
`target: branch`; their bodies match the manifest byte-for-byte (bypass_actors
`[]`, same conditions and rules).

### The "permission ceiling" is not a GITHUB_TOKEN ceiling

- Live: `default_workflow_permissions` is `"write"`. A read-only repo default
  would be a hard cap that workflow YAML cannot exceed; that cap is not in
  effect.
- `docs/fork/ideal-base/evidence/R07/design-gate.md:26-53` (Finding 1,
  blocking): the `workflows` ruleset rule type is org/enterprise-scoped and
  returns `422 Validation Failed` on this personal, user-owned repository, so no
  ruleset here can require/constrain a workflow.
- `docs/fork/ideal-base/evidence/R07/design-v2-gate.md:99`: "repository Actions
  default workflow permissions are `write`, with review approval enabled."

Each workflow constrains its own token via `permissions:`. The main.yml entry
(`main.yml:11-12`) and every helper declare `contents: read`.

### Other tag-triggered workflows (for the push-of-tag event root)

- `release.yml:3-6,27-28`: `push: tags: ['v*']`, `permissions: contents: write`;
  verifies the tag is an ancestor of authoritative main (`release.yml:73-78`
  `git merge-base --is-ancestor "${GITHUB_SHA}" FETCH_HEAD`) and publishes a
  metadata-only release with `github.token` (`release.yml:143-163`).
- `nix.yml:4-5,29-30,116`: `push: tags: ["v*"]`, `contents: read`, publishes to
  Cachix via `secrets.CACHIX_AUTH_TOKEN` (not `GITHUB_TOKEN`).
- Neither fires for `fork-point` (not a `v*` tag).

### Why the governance leg needs a PAT

`scripts/governance_compare.py:420-428`:

```python
# A credential without ruleset write access gets a body with no
# bypass_actors at all. Treating that as "no bypass actors" would turn
# an unauthorized read into a green result ...
if "bypass_actors" not in entry:
    raise SchemaError(... "the credential cannot see bypass actors, so this read is unauthorized, not empty")
```

Live acquisition is a set of bare `gh api` GETs (`governance_compare.py:1013-1060`:
`repos/{repo}`, `repos/{repo}/rulesets`, per-id ruleset, `rules/branches/main`,
`branches/main/protection`, `branches`), any of which failing (other than the
permitted 404) is fatal (`gh_api`, `governance_compare.py:986-1005`).

## Remaining unknowns

- **`RULESET_AUDIT_TOKEN` scope/expiry** is not inspectable from the clone or via
  read-only `gh api` (secrets are write-side). Whether the token can still read
  `bypass_actors` can only be confirmed by a live `fork-health.sh --live` run,
  which this investigation did not execute (it would have been a legitimate
  read, but a live run was out of scope and could not be verified against a
  clean, unauthenticated baseline).
- **Whether the owner intends tags to be protected at all.** The R07 design
  history shows "immutable tag, pin the ruleset's workflow ref/sha to it" was
  explicitly rejected with the `workflows`-rule mechanism
  (`docs/fork/ideal-base/evidence/R07/design.md:26-41`). No replacement
  tag-protection mechanism is recorded, so the fork-point tag remains
  detection-only by design. Whether that is an accepted residual risk (like the
  owner-as-trust-root decision, `design.md:20-24`) or an unexamined gap is not
  stated in any file I read.
- **The exact daily-lag ceiling** is the cron `37 9 * * *` (`fork-health.yml:11`),
  so worst-case detection latency is just under 24 h plus job runtime, but I did
  not verify the GitHub scheduler's actual execution history.
- I did **not** run `scripts/fork-health.sh --live` or open/trigger any workflow,
  and I made no GitHub writes of any kind.
