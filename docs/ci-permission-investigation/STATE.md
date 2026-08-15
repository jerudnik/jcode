# CI permission investigation — state

Snapshot of the deep-swarm investigation into jcode's CI workflow permission
governance, as of 2026-08-15. The task graph (114 nodes, coordinator `dog`) is
parked; the three analysis-only ready items are now drained. The remaining
frontier requires hosted GitHub Actions runs and is gated on human
authorization (see below).

## Completed conclusions (this round)

1. **`actionlint-gap-dollar-actionlint-compat`** — production workflows hit no
   actionlint grammar gap. Upstream actionlint 1.7.12 has three real gaps on
   this surface (`$/path` false rejection, `$/path@ref` silent acceptance,
   `code-quality`/`vulnerability-alerts` false rejection) and the repo's flake
   pin already patches all three with fail-closed fixture tests. None is
   triggered by today's workflows.
   → `actionlint-gap-dollar-actionlint-compat.md`

2. **`actionlint-gap-workflow-parser-valid-yaml`** — no live parser gap. The
   flake-pinned actionlint parses every YAML construct the repo uses and exits
   0 on all 14 workflow files. Only merge keys `<<:` (rejected) and custom
   tags `!tag` (silently dropped) mishandle, and neither appears in the repo.
   → `actionlint-gap-workflow-parser-valid-yaml.md`

3. **`reconcile_tag_guard_with_main_permission_ceiling`** — no permission gap
   between the tag guard (`fork-health.yml` check 1) and the main-branch
   rulesets: rulesets constrain branch ref operations, not `GITHUB_TOKEN`
   (live `default_workflow_permissions` is `write`; the org-only `workflows`
   ruleset rule is unavailable here). The real gaps are coverage and PAT
   dependency: **no tag-protection ruleset exists** and the guard does not
   fire on tag push, so `fork-point` mutation is detection-only with up to
   ~24 h lag; the governance leg needs `secrets.RULESET_AUDIT_TOKEN` because
   `GITHUB_TOKEN`'s ruleset read omits `bypass_actors` (comparator fails
   closed on that omission).
   → `reconcile_tag_guard_with_main_permission_ceiling.md`

## Low-confidence completed node (updated assessment)

- `verify_production_workflow_transplant_parity` — previously flagged
  low-confidence by the swarm. This round did not re-verify it; the flag
  stands until the hosted-evidence items below run.

## Parked frontier — requires human authorization (hosted CI runs)

These three items gather before/after evidence from real GitHub Actions runs
and cannot proceed without explicit per-run approval:

1. `integrate_production_repair_and_prove_parity`
2. `materialize_and_verify_governance_repair_artifacts`
3. `obtain_hosted_repaired_context_evidence`

Also still blocked behind `depends_on` edges in the graph: `actionlint`,
`failing_head_permission_matrix`, `live_required_context_reconciliation`,
`plan2`, `review`, `search_code`, `search_web` and their `::gate` nodes.

## Hosted-CI round (2026-08-15, authorized) — all three completed

4. **`obtain_hosted_repaired_context_evidence`** — CRITICAL FINDING. Dispatched
   fork-health run 31891508962 is a **false green**: the live governance leg
   exited 2 ("You are not logged into any GitHub hosts") because
   `secrets.RULESET_AUDIT_TOKEN` is **undefined** in this repo, and
   `fork-health.yml:41` pipes the guard through `| tee health.txt` with no
   `set -o pipefail`, so `tee`'s exit 0 masks the failure. All ~30 recent
   scheduled runs share this signature — the hosted governance-comparison leg
   has never once succeeded. A local `fork-health.sh --live` exits 0 and
   matches the manifest, so only the hosted leg is dead.
   → `obtain_hosted_repaired_context_evidence.md`

5. **`materialize_and_verify_governance_repair_artifacts`** — all three repair
   artifacts verified against live state: comparator exits 0 ("snapshot
   matches the manifest", zero FAIL lines), permission linter exits 0, 7/7
   actionlint negatives rejected and 2/2 positives accepted via the flake-locked
   binary. The manifest IS the live ruleset state.
   → `materialize_and_verify_governance_repair_artifacts.md`

6. **`integrate_production_repair_and_prove_parity`** — parity proven for 3 of
   4 repair elements (manifest-vs-live, actionlint patch green on both PR
   heads, all four gates green and non-vacuous on both merge SHAs). Two gaps:
   the fork-health false green (above), and production is a **deliberately
   shrunken successor** of the R07 repair contract (5 protected paths, 2
   required contexts, comparator tests removed from CI via modernization PR
   #142) rather than the 27-path/4-context contract the R07 adjudication
   describes — reopening the comparator-bypass class R07's independent review
   had closed.
   → `integrate_production_repair_and_prove_parity.md`

## Actionable findings for the maintainer

- **FIX: fork-health false green** (highest priority). Two one-line-class fixes:
  (a) add `set -o pipefail` to the `Run fork health check` step in
  `.github/workflows/fork-health.yml` (line 41) so the guard's exit 2 fails the
  job instead of being masked by `tee`; (b) either define
  `secrets.RULESET_AUDIT_TOKEN` (needs ruleset read incl. `bypass_actors`) or
  formally accept the governance leg as locally-verified-only and delete it
  from the hosted workflow. Until then the hosted guard reports success while
  doing nothing.
- **DECIDE: shrunken contract.** Production protects 5 paths with 2 required
  contexts; the R07 adjudication describes 27 paths / 4 contexts, and PR #142
  removed the comparator tests from CI. If the shrink was intentional scope
  reduction, record it in the adjudication docs; if not, the comparator-bypass
  class R07's independent review closed is open again.
- **Tag protection**: `fork-point` has no ruleset targeting `refs/tags/*` and
  the guard only runs on schedule/dispatch. If prevention (not just detection)
  is wanted, options are a tag-push-triggered workflow or accepting the ~24 h
  detection lag as documented residual risk. R07 history shows the
  `workflows`-rule mechanism was explicitly rejected as unavailable on
  user-owned repos.
- **secrets/vars name validation**: actionlint can never validate
  `secrets.*`/`vars.*` names (e.g. `vars.JCODE_PREVIOUS_RELEASE_REF` typos
  pass lint). Accepted by design; noted for completeness.
