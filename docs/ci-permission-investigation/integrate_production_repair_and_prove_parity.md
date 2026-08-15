# integrate_production_repair_and_prove_parity

Node: `integrate_production_repair_and_prove_parity`
Date: 2026-08-15 (UTC). Read-only verification node.

## The question

Is the governance repair (R07 and its maintenance-window integrations) actually
what production runs today? Concretely, for each repair element — the
declarative ruleset manifest, the fork-health guard, the actionlint
compatibility patch, and the four-surface gate topology (Governance Root,
PR Gate, Fork CI / Rust checks, Nix checks) — is it present in the production
tree, and is there a green hosted run on the #147 merge (`c0ee9ba2`), the
#148 merge (`fad729f5`), or fork-health run `31891508962` that evidences it ran?

## What I checked

1. `docs/fork/ideal-base/evidence/R07/integration-adjudication.md` plus the
   maintenance-window records (pr136, pr138 read in full; directory listed) to
   enumerate the repair.
2. Production files: `scripts/required-checks.json`,
   `scripts/fork-health.sh`, `.github/workflows/{governance-root,pr,ci,fork-ci,nix,main,fork-health}.yml`,
   `flake.nix`, `nix/actionlint-dollar-local-workflows.patch`,
   `tests/test_governance_compare.py`.
3. Hosted state via GETs only: check runs on both PR head SHAs
   (`e8800545f` for #147, `36d24bf44` for #148) and both merge commits, the
   Main push runs on both merges, fork-health runs (all 30 recent), job step
   detail for the gates, live rulesets, and repo secrets (names only).
4. Git history for how the production shape diverged from the R07 record
   (commits `621f4d44d`, `4b088097f`, `8907e568d`, `9a580f1e9`; PR #142) and
   `docs/modernization/TASK_GRAPH.json` nodes G10A/V10/H10/H30.

## Conclusion: parity holds for three of four elements; one hard gap

**Parity is proven for the manifest contract, the actionlint patch, and the
gate topology — but the production design is a deliberately shrunken successor
of the R07 repair, not the R07 repair itself, and the fork-health guard's live
governance leg has never once run green hosted. Every "success" conclusion on
fork-health is a false green.**

### Element 1 — manifest (`scripts/required-checks.json`): PRESENT, contract at parity, scope superseded

- Present at `scripts/required-checks.json:1-145`; `additions_adjudicated: true`
  (line 58), 5 protected paths (lines 59-65), 2 required contexts
  (lines 104-119).
- Independently verified live parity via `gh api repos/jerudnik/jcode/rulesets`:
  `protect-fork-rails` (id 18509013) is `active`, `bypass_actors: []`, rules
  `[deletion, non_fast_forward, pull_request, required_status_checks]`, required
  contexts exactly `["Governance Root", "PR Gate"]`; `no-stray-branches`
  (18509016) `active` with the creation rule. The live state matches the
  manifest byte-for-byte on every field the comparator checks (a local
  `--live` run by the sibling node exits 0; `STATE.md` records it).
- **Scope drift, not a defect:** the R07 adjudication enforced 27-32 protected
  paths and four required contexts (`Governance Root`, `Fork CI Gate`,
  `Security Gate`, `Nix Gate`). Production enforces 5 paths and 2 contexts.
  This reduction was an explicit, planned decision of the modernization
  program — `docs/modernization/TASK_GRAPH.json` node G10A ("limit protected
  paths to long-lived rules"), commits `621f4d44d` (2026-08-08) and
  `8907e568d` (2026-08-10), merged as PR #142 (2026-08-11). It is a successor
  design, not drift. But no R07 §4-format maintenance-window record covers the
  PR #142 merge itself; the modernization used its own H10/H30 permission
  nodes instead, and no `Modernization-Node: H10` trailer exists in git log.

### Element 2 — fork-health guard: PRESENT in tree, hosted evidence is a FALSE GREEN

- Present at `scripts/fork-health.sh:1-203` (fork-point anchoring, CI-table
  currency, Windows-CI exclusion, mandatory governance comparison with
  exit-2-on-acquisition-failure) and wired to `.github/workflows/fork-health.yml:35-47`.
- Run `31891508962` (workflow_dispatch, head `fad729f5`, 2026-08-15T15:00:42Z)
  reports `conclusion: success`. The log shows why that is false:

  ```text
  GH_TOKEN:                      <- empty: secrets.RULESET_AUDIT_TOKEN is undefined
  ERROR: live governance acquisition failed at endpoint: gh auth status
        You are not logged into any GitHub hosts.
  error: governance comparison could not be completed (exit 2)
  ```

  The job still reports success because `fork-health.yml:40-41` pipes the
  script through `tee` under `bash -e` without `pipefail`, so exit 2 is
  discarded. `gh secret list` shows only `CACHIX_AUTH_TOKEN`; no
  `RULESET_AUDIT_TOKEN` exists.
- This is not a one-off. I sampled all 30 recent runs: every run since
  2026-07-29 shows the same masked exit 2; runs 2026-07-18..07-28 (older script)
  ended in `2 invariant violation(s)`, also masked green. **No hosted
  fork-health run in history contains the string `governance snapshot matches
  scripts/required-checks.json`.** The live leg has never succeeded hosted.
- Parity of the *underlying contract* is proven locally (local `--live` exit 0
  plus my independent ruleset GET), but there is zero hosted green evidence
  for the guard's governance leg.

### Element 3 — actionlint compatibility patch: PRESENT and green hosted

- Present at `flake.nix:220-237` (`actionlint.overrideAttrs` applying
  `nix/actionlint-dollar-local-workflows.patch` plus `rule_permissions.go`
  substitutions for `code-quality`/`vulnerability-alerts`) and the fail-closed
  fixture battery in the flake `workflow-syntax` check (`flake.nix:350-478`).
- Hosted evidence: `nix.yml:43-65` "Lint fork-owned workflows" runs
  `nix run .#actionlint` over the 11 fork-owned workflows. Step verified green
  on both PR heads: job `95012413887` (#147) and job `95012434973` (#148), and
  the same job green on both merge-commit runs (`31885795898`, `31887033303`).
- Residual: the flake `workflow-syntax` check's fixture regression tests run
  only under `nix flake check`, which no hosted job executes; the hosted step
  runs actionlint itself but not the negative fixtures.

### Element 4 — gate topology: PRESENT and green hosted

All four surfaces exist in production and ran green on both PR heads
(all check runs emitted by GitHub Actions app id 15368):

| Surface | Production location | #147 head `e8800545f` | #148 head `36d24bf44` |
|---|---|---|---|
| Governance Root | `.github/workflows/governance-root.yml:15` | success | success |
| PR Gate | `.github/workflows/pr.yml:53` | success | success |
| Fork CI / Rust checks | `.github/workflows/fork-ci.yml:16` | success | success |
| Nix / Validate Nix and workflow policy | `.github/workflows/nix.yml:33` | success | success |

Non-vacuity confirmed at the step level: Governance Root job `95012381781`
ran "Detect governance-path changes" to success; the Rust job `95012434967`
ran check/test/full-test recipes to success; Nix validate ran actionlint and
the distribution policy to success. On the merge commits themselves the Main
push runs were green except `Publish / Require Cachix publication` on
`c0ee9ba2` (`31885795898`), which is `cancelled` — expected, superseded by
#148's push under `main.yml`'s `cancel-in-progress` concurrency, and it is not
a required context.

## The parity gap list

1. **Fork-health live governance leg (blocking).** No hosted run has ever
   compared live rulesets to the manifest. `RULESET_AUDIT_TOKEN` does not
   exist (repo has only `CACHIX_AUTH_TOKEN`), and the `| tee` pipeline in
   `fork-health.yml:40` masks the guard's designed fail-closed exit 2 as
   success. Every daily "green" since 2026-07-29 is false.
2. **Comparator tests no longer run hosted.** R07 remediation G1 added a
   `governance-contract` job to `fork-ci.yml` running
   `tests.test_governance_compare`; modernization commit `4b088097f` removed
   it. The 1009-line test file still exists but is invoked by no workflow,
   justfile recipe, or flake check (grep across `.github/`, `flake.nix`,
   `justfile` finds zero call sites).
3. **Protected set no longer covers its own test.** `621f4d44d` removed
   `tests/test_governance_compare.py` (and 20+ gate scripts) from the
   protected list, so a tampered comparator or its tests merge without
   turning Governance Root red — the exact G1 bypass class R07's independent
   review closed.
4. **No R07-format window record for the PR #142 topology change** (gap 1's
   upstream cause). The reduction is planned and reviewed inside
   `docs/modernization/`, but the R07 §4 record trail this investigation
   treats as the repair contract was not extended to it.
5. **Minor:** the flake actionlint fixture regression battery runs hosted
   nowhere; and the R07 fixture `governance-valid.json` was deleted by
   `9a580f1e9`, leaving `generate_governance_fixture.py` protected but with
   its fixture consumer gone.

## Evidence

- Files: `scripts/required-checks.json`, `scripts/fork-health.sh`,
  `.github/workflows/governance-root.yml`, `.github/workflows/pr.yml`,
  `.github/workflows/ci.yml`, `.github/workflows/fork-ci.yml`,
  `.github/workflows/nix.yml`, `.github/workflows/main.yml`,
  `.github/workflows/fork-health.yml`, `flake.nix`,
  `nix/actionlint-dollar-local-workflows.patch`, `tests/test_governance_compare.py`,
  `docs/modernization/TASK_GRAPH.json` (G10A/V10/H10/H30).
- Hosted runs (all GETs):
  - PR #147 head checks: https://github.com/jerudnik/jcode/commit/e8800545ff5daa9d19a59c08e5494625c4a1fbfd/checks
    (Governance Root, PR Gate, Rust checks, Nix validate all success, app 15368)
  - PR #148 head checks: https://github.com/jerudnik/jcode/commit/36d24bf4420b8941c735a33aa549559ebdf38d2a/checks
    (same set, all success)
  - Governance Root job detail: https://github.com/jerudnik/jcode/actions/runs/31884781630/job/95012381781
  - Rust checks job detail: https://github.com/jerudnik/jcode/actions/runs/31884792317/job/95012434967
  - Nix validate job detail: https://github.com/jerudnik/jcode/actions/runs/31884792317/job/95012434973
  - Main push on `c0ee9ba2`: https://github.com/jerudnik/jcode/actions/runs/31885795898
    (Publish cancelled by concurrency; rest green)
  - Main push on `fad729f5`: https://github.com/jerudnik/jcode/actions/runs/31887033303 (all green)
  - Fork health: https://github.com/jerudnik/jcode/actions/runs/31891508962
    (success = false green; log excerpt above)
  - Live rulesets: `gh api repos/jerudnik/jcode/rulesets` and `.../rulesets/18509013`
  - Secrets: `gh secret list` → `CACHIX_AUTH_TOKEN` only.
- Key quoted outputs are inline above; the false-green log excerpt and the
  empty `GH_TOKEN:` line come from the run-31891508962 job log.

## Remaining unknowns

1. Whether `RULESET_AUDIT_TOKEN` ever existed and was deleted, or was never
   created after the modernization rewrote `fork-health.yml` (its commit
   history was not probed commit-by-commit).
2. Whether the modernization's H10/H30 permission nodes were approved out of
   band (chat) without leaving a git trailer or doc record; nothing in
   `docs/modernization/` or git log records their execution.
3. Whether `tests/test_governance_compare.py` still passes today — no hosted
   or local run of it was performed in this node (read-only posture), so gap 2
   describes absence of evidence in CI, not a known test failure.
4. Whether pre-2026-07-18 fork-health runs (log retention window) ever had a
   working live leg; the earliest retrievable run is 2026-07-18.
