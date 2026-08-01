# R07 design gate — adversarial review verdict

Reviewed: `automation/r07-design` at `26eedab27` (design.md + 7 proposal artifacts),
against ground truth in `recon.md` (`automation/r07-recon`, read via `git show`, never
merged into the reviewed tree). Live checks ran against `jerudnik/jcode` via `gh api`
with a token whose scope is limited to this repository; no write survived any check
(every created test ruleset was deleted immediately after observation).

## Verdict: FAIL

The design's central trust-root mechanism — the repository-level `workflows` ruleset
rule that is supposed to give zero-approval governance an external, candidate-proof
anchor — is rejected outright by the live GitHub API for this repository. The design
was written and internally validated as if this rule is available; it is not, for the
plan as currently scoped (personal/user-owned public repository, not an organization).
This is not a cosmetic gap: every other control in the design (zero required
approvals, `required_status_checks` pinned by `integration_id`, the immutable-transition
bootstrap) is explicitly justified in design.md as depending on this rule for its
"independent trust root." Without it, the design's own stop-condition fires
("The required-workflow rule is unavailable... Stop; zero-approval governance is not
self-protecting"), so this is a confirmed FAIL against the design's own criteria, not
merely an outside objection.

## Findings

### Finding 1 (blocking): `workflows` ruleset rule type is unavailable on this repository

- **Claim under review:** design.md §4 requires GitHub to accept a `workflows` rule
  (`.github/workflows/governance-root.yml` from `refs/heads/main`, `repository_id:
  1238606714`) inside the `protect-fork-rails` ruleset, and treats this as the sole
  external anchor that makes zero-required-approvals safe (§2, §3, §4, §13 all state
  this explicitly).
- **What I did:** Created and immediately deleted disabled, narrowly-scoped test
  rulesets (`target: refs/heads/__nonexistent-test-branch__`, `enforcement: disabled`,
  so nothing could ever be enforced) directly against `repos/jerudnik/jcode/rulesets`
  via `gh api`, using a `workflows` rule with the exact `repository_id` design.md cites,
  varying `ref` form (`main`, `refs/heads/main`, `heads/main`), `sha` instead of `ref`,
  and workflow paths that exist on `main` today (`ci.yml`) as well as the not-yet-created
  `governance-root.yml`.
- **Result:** Every variant returned `422 Validation Failed`:
  `"Invalid rule 'workflows': Invalid parameter workflows: Workflow error at index 0: "`.
  A control ruleset using only `deletion` + `non_fast_forward` on the same repository
  succeeded and was cleanly deleted, confirming the API and credentials work and the
  failure is specific to the `workflows` rule type.
- **Root cause, confirmed against GitHub's own docs:** GitHub's "Available rules for
  rulesets" page states plainly: *"Ruleset workflows can be configured at the
  organization or enterprise level to require workflows to pass before merging pull
  requests."* This is a **repository-level** ruleset write (`repos/{owner}/{repo}/rulesets`,
  matching every other artifact in this design), against a **personal, user-owned**
  repository (`gh api repos/jerudnik/jcode` shows `owner.type: User`). The `workflows`
  rule is documented and, per direct testing, actually enforced as organization/enterprise
  scoped only — it is not available to attach to a repository-level ruleset on a
  personal account regardless of plan.
- **Evidence:** live `gh api` transcripts above (repeated 422s); GitHub docs
  `available-rules-for-rulesets` ("Require workflows to pass before merging" section,
  both the GHEC and default doc trees, word-for-word consistent); GitHub community
  discussion #69595 confirming (from GitHub staff, `lrotschy`, Oct 2023) *"we do not
  plan to release this to non-enterprise orgs. Organization Rulesets is an
  enterprise-only feature"* for the org-level required-workflows predecessor.
- **Why this is not covered by design.md's existing stop-condition:** The stop-condition
  exists ("The required-workflow rule is unavailable... Stop"), so the design
  acknowledges the *possibility* abstractly. But the design's own feasibility claims
  (§3: "This external ruleset rule is mandatory while the PR approval count remains
  zero"; the diff table in §4 presenting the `workflows` rule as a plain "Add" alongside
  three rules that did succeed in testing) read as if the write is expected to succeed,
  and nothing in design.md, recon.md, or the artifact set flags that this specific rule
  type is scoped to organizations/enterprises and structurally cannot attach to a
  repository-level ruleset on `jerudnik/jcode` as currently owned. The design was never
  tested against the live API before being written up as executable, and the recon this
  design cites does not contain a feasibility probe of this rule either (recon.md's
  `grep -n -i workflow` hits are all about `.github/workflows/*.yml` CI files, not the
  ruleset `workflows` rule type).
- **Consequence:** If this design is executed as written, sequence step in
  `github-governance.proposed.json` that adds the `workflows` rule will 422, the
  named checkpoint (sequence 8, "stop here on any mismatch") will correctly prevent
  the classic-protection DELETE from running — so the design's own fail-closed
  sequencing does contain the blast radius — but the entire zero-approval governance
  model this design set out to build cannot be completed as specified. The design
  needs a real resolution (e.g., accept required non-zero approvals as the trust
  root instead of the `workflows` rule, move the repository into an organization,
  or find another mechanism), not just a stop-condition that correctly detects
  the dead end.

### Finding 2 (minor, previously noted): unsupported citation of `repository_id` and `integration_id`

- design.md §4 and §3 cite `repository_id: 1238606714` and `integration_id: 15368` as
  values embedded in the "authoritative apply document," framed throughout the design
  as recon-derived facts (design.md explicitly says the integration ID "is not guessed"
  and cites "Recon's PR #38 check runs"). `integration_id: 15368` is indeed traceable to
  recon (recon documents the `github-actions` app on PR #38 check runs). However,
  `repository_id: 1238606714` **does not appear anywhere in recon.md** — confirmed by
  direct grep across the full recon text.
- I independently verified `repository_id: 1238606714` is in fact the correct value for
  `jerudnik/jcode` via `gh api repos/jerudnik/jcode`. The number is right; the
  evidentiary chain is not. The design should either add a repository_id lookup step to
  recon (or a dedicated verification step in the design itself) rather than presenting
  the number as if it flows from the cited recon evidence.
- Severity: minor. Does not affect correctness of the design's intended behavior, only
  the traceability/reproducibility of one input value.

## Edge cases considered

- Whether GitHub's blanket 422 might be caused by a malformed request rather than a
  genuine plan/ownership restriction: ruled out by varying `ref`/`sha` forms and by the
  fact that a structurally similar ruleset request (different rule type, same endpoint,
  same auth) succeeded and was cleanly deleted.
- Whether the repository might need to be transferred into an organization for this
  design to work at all, which is a legitimate fix but a significant, undocumented
  scope change absent from every artifact reviewed.
- Whether `evaluate` enforcement mode (mentioned in GitHub's own docs as a way to test
  required-workflow rules before activating them) changes anything: it does not apply
  here — the docs are explicit that the *feature itself* is org/enterprise-scoped, not
  merely gated by enforcement status; evaluate mode is also GHEC-only per other doc
  text encountered during this review.
- Whether this could be a transient GitHub API bug: considered unlikely given (a)
  GitHub's own documentation independently and consistently states this scope
  restriction across multiple doc versions (GHEC, GHES 3.13/3.15/3.18, default), and
  (b) a GitHub staff member confirms the same restriction in a public community thread.
- Did not attempt to create the ruleset as `enforcement: active` against the real
  `refs/heads/main` target (that would be a genuine, non-reversible production change
  to the reviewed repository and is out of scope for a read-only design review); the
  `disabled` + nonexistent-branch-target probes were chosen specifically to get an
  authoritative accept/reject signal from the API without any risk of enforcement.

## Validation performed (this session and prior, cumulative)

- JSON syntax: all 6 JSON artifacts valid. YAML syntax: `governance-transition.workflow.template.yml` valid (yamllint).
- `github-governance.proposed.json`'s `workflows` rule reproduced verbatim (same
  `repository_id`, same rule shape) against the live API: rejected (422), see Finding 1.
- `STATE.proposed.json`: 57 nodes, exact match to live `STATE.json` node set; all
  "accepted" nodes have non-null reviewed/published commits; all others null. Clean.
- Ancestry spot-check (F01, F18, F20c, F29, W3 published commits): all confirmed
  main-ancestral via `git merge-base --is-ancestor`.
- F28 content divergence and F18 content equality both confirmed via direct `git diff`.
- Patch-id uniqueness for F01 confirmed: reviewed/published commits share one patch-id,
  which appears exactly once across 597 distinct patch-ids in main history, matching
  recon's "597 distinct over 601 commits."
- `mapping-ledger.proposed.json`: 35 entries, method distribution matches recon exactly;
  zero mismatches against `STATE.proposed.json`.
- `archive-manifest.proposed.json`: 33 heads + 6 tags, all cross-checked against
  `STATE.proposed.json` reviewed_commit values; zero mismatches; F20c retire-tag
  exclusion and F29/W3 already-ancestral exclusions both correctly reasoned.
- `workflow-contexts.proposed.patch`: applies cleanly (`git apply --check` and a real
  apply in a disposable scratch worktree of `main`); resulting workflow files pass
  `actionlint` with zero errors across all 5 touched/added workflow files; `fetch-depth:
  0` present; `nix.yml`'s `pull_request` path filter correctly removed while `push`
  filter remains, addressing "every required context fires on every PR."
- F19 confirmed as a genuine 2-parent merge commit.
- Stop-conditions (§13) verified fail-closed and named in advance; the four open
  questions verified to be pure ownership/authorization, not smuggled technical
  decisions, cross-checked against the 6-row acceptance-mutation matrix.
- Archive-privacy-before-push ordering verified explicit in design.md (§ "Ordered
  barriers," barrier 0/1): coordinator must prove the archive repository is private
  before any push; recon's own 404-on-privacy-check gap is explicitly named and
  produces a stop, not a silent proceed.
- Classic-protection deletion ordering verified explicit and fail-closed in
  `github-governance.proposed.json`: DELETE of classic protection is sequence 9,
  gated by a checkpoint at sequence 8 that requires sequences 2/3/5/7 (all
  read-backs of the new ruleset, effective rules, no-stray ruleset, and repository
  merge-method settings) to have passed first; `abort_policy` makes this explicit.
  This ordering is sound in isolation, but it cannot rescue Finding 1: the write it
  is protecting will never reach a passing state as specified, because the
  `workflows` rule inside that same ruleset write is rejected before checkpoint 8 is
  ever reached.

## What I did not check

- Whether every other rule type used in `github-governance.proposed.json`
  (`pull_request` with `required_approving_review_count: 0`, `required_status_checks`
  with `strict_required_status_checks_policy: true`, `non_fast_forward`, `deletion`,
  `creation`) matches the live API schema field-by-field beyond what a passing 201
  create/delete round-trip on a disabled test ruleset with those exact rule bodies
  would show; I did not perform that full round-trip for every remaining rule (only
  for `deletion`+`non_fast_forward` as a control, and for `workflows` as the target of
  this investigation). Given Finding 1 is already blocking, I judged further schema
  micro-validation of the surviving rules to have low marginal value for this gate,
  but it is not exhaustively confirmed.
- Whether an organization-owned fork of this repository, or a GitHub Enterprise
  Cloud/Team plan upgrade, would be an acceptable remediation path — that is a product/
  ownership decision outside this review's scope, not a technical fact I can settle.
- Full field-by-field diff of `required_status_checks`/`pull_request` parameters
  against the GitHub REST API docs schema beyond the spot checks above (was
  in-progress at the start of this session; superseded in priority by the blocking
  Finding 1, which I judged needed to be nailed down and reported before spending
  further budget on non-blocking schema polish).
- Did not test the org-level ruleset endpoint (`orgs/{org}/rulesets`) since this
  repository is not inside an organization; that path is unavailable to test as this
  repository is currently owned.
- Did not exhaustively enumerate every possible `workflows` rule payload shape (e.g.
  omitting `do_not_enforce_on_create`, varying array wrapping); the four variants
  tried all failed identically and match the documented scope restriction, which I
  judge sufficient to attribute the failure to plan/ownership scope rather than
  payload malformation, but a residual small chance of an unexplored payload shape
  succeeding cannot be fully ruled out.

## Confidence: high

The 422 is reproducible, consistent across multiple payload variants, matches
GitHub's own documented scope restriction word-for-word across several doc versions,
and is corroborated by a GitHub staff comment on the public predecessor feature. The
control ruleset creation succeeding on the same repository/credentials rules out an
auth or malformed-request explanation. This finding is blocking because it falsifies
the specific, named claim in design.md §3 ("This external ruleset rule is mandatory...")
under the design's own stated criteria for what would disprove it (§13).
