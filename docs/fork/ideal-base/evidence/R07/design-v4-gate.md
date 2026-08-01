# R07 design v4 gate: adversarial re-review

Reviewed `automation/r07-design` at `886f1a383` (local, unpushed) from an isolated worktree,
against:

- R07 in `docs/fork/ideal-base/WORK_GRAPH.json` (`all_nodes` list entry `R07`);
- D031 in `docs/fork/ideal-base/DECISIONS.md`;
- the v1 gate (`automation/r07-design-gate:.../design-gate.md`, FAIL);
- the v2 gate (`automation/r07-design-v2-gate:.../design-v2-gate.md`, FAIL);
- the v3 gate (`automation/r07-design-v3-gate:.../design-v3-gate.md`, FAIL, with an explicit
  five-item "Required to reach PASS" list); and
- the complete `c27b1d8b2..886f1a383` diff.

## Verdict: PASS

v4's diff touches exactly the three files the v3 gate's fix list implicated
(`design.md`, `github-governance.proposed.json`, `workflow-contexts.proposed.patch`), and within
`github-governance.proposed.json` only sequences 3, 4, 5, 6, 8, 14, `abort_policy`, and
`residual_risk` changed — every other sequence (1, 2, 7, 9, 10, 11, 12, 13, 15, 16, 17) is
byte-identical to v3, confirmed by direct object comparison. `STATE.proposed.json`,
`mapping-ledger.proposed.json`, and `archive-manifest.proposed.json` are byte-identical to v3
(and therefore v2 and v1), confirmed by SHA-256 against the exact digests the v3 gate recorded.

I independently reproduced all five of the v3 gate's "Required to reach PASS" items against the
live GitHub API and against real git history and merges on this repository, including
constructing the specific counterexamples the v3 gate used to break v3. All five now hold.

## Item-by-item verification

### 1. Sequence 6: uncapped protected-path identity (was blocking) — HOLDS

v3's defect was the compare API's undocumented 300-file cap on `files`, which silently dropped
`scripts/fork-health.sh` and `scripts/ideal_base_railway.py` from a 875-file/222-commit range.
v4 replaces the compare-API `files` assertion with a `GET_AND_LOCAL_GIT` step: resolve
`current_main_sha` via `GET repos/.../commits/main`, unshallow the operator's clone if needed,
fetch `bootstrap_merge_sha` and `current_main_sha` explicitly by SHA, prove ancestry with
`git merge-base --is-ancestor`, then run `git diff --quiet <old> <new> -- <protected paths>`
locally.

Independent reproduction, this repository:

- **Fresh shallow clone → unshallow.** `git clone --depth 1 https://github.com/jerudnik/jcode.git`
  produces a genuinely shallow clone (`is-shallow-repository` = `true`). Running the design's
  exact prepare command, `git fetch --no-tags --unshallow https://github.com/jerudnik/jcode.git
  refs/heads/main`, succeeds and leaves `is-shallow-repository` = `false`. Verified twice
  (a full unshallow-then-diff run and a from-scratch run against real SHAs
  `8d851ea72..498249777`).
- **Arbitrary-SHA fetch.** `git fetch --no-tags https://github.com/jerudnik/jcode.git <old SHA
  not on any branch tip>` (tested with `04fa8a3922...`, the v3 gate's own truncation-repro
  base commit, and with `8d851ea72...`) succeeds from both a shallow and a full clone. GitHub's
  server does not reject reachable-SHA1-in-want here; no `allowAnySHA1InWant` failure was
  observed for commits that are ancestors of `main`.
- **300-file truncation case, replayed locally.** `git diff --quiet 04fa8a3922...
  498249777c4... -- .github/scripts/ .github/workflows/ scripts/required-checks.json
  scripts/fork-health.sh scripts/ideal_base_railway.py tests/test_ideal_base_railway.py
  docs/fork/ideal-base/evidence/R07/github-governance.proposed.json` exits 1 and
  `git diff --name-status` over the same range and pathspec correctly lists
  `scripts/fork-health.sh` and `scripts/ideal_base_railway.py` alongside the 12
  `.github/workflows/*` changes — exactly the two protected paths the API's cap dropped. Local
  `git diff` has no file-count cap; the finding does not reproduce against it.
- **Rename attack, replayed.** Constructed the v3 gate's own counterexample in a disposable
  clone: renamed `.github/workflows/fork-ci.yml` to `ci-fork.yml.bak` (protected → unprotected)
  and, separately, `README.md` to `.github/workflows/sneaky.yml` (unprotected → protected). Both
  renames make `git diff --quiet <base> <head> -- <protected paths>` exit 1 (diff detected):
  a one-sided pathspec renders a rename crossing the boundary as an add or delete on the covered
  side, which the sequence 6 assertion (and its own text) states explicitly. Verified against a
  real historical rename in this repository (`e59985449`,
  `docs/fork/NEXT_SESSION_KICKSTART.md` → `docs/archive/NEXT_SESSION_KICKSTART.md`) as a sanity
  check on rename semantics.
- **Zero-diff case.** `git diff --quiet <sha> <sha> -- <protected paths>` (current main ==
  bootstrap_merge_sha) exits 0, matching the design's stated pass condition.
- **Protected-path list completeness.** Verified `.github/scripts/` is present in
  `template_variables.protected_paths` and is the first entry; grepped required workflows
  (`fork-ci.yml`, `ci.yml`) for `.github/scripts/run_with_timeout.py` invocations, confirming the
  rationale given in §0b/§3 is accurate.

No counterexample survived. The v3 gate's Finding 1 and Finding 2 are both closed by the same
mechanism change, and I could not construct a new failure mode against it (see "What I did not
check" for the one thing I did not fully stress: `allowReachableSHA1InWant` behavior under a
server configuration this GitHub-hosted repository does not use).

### 2. Maintenance step 7: executable no-intervening-merge proof (was blocking) — HOLDS

v3's defect was that `compare/{base}...refs/heads/main` returns every commit in the range, not
just merge commits, so the "exactly one commit" assertion failed on every real merge (verified
2/4/9/15 commits on four real merges).

v4 replaces this with two conditions: (a) `post_restore_main_sha == merge_sha` (tip equality),
and (b) `git rev-list --first-parent --merges expected_base_sha..post_restore_main_sha` has
exactly one entry equal to `merge_sha`.

Independent reproduction:

- **Real merges pass.** Ran the exact `--first-parent --merges` command against the same four
  real merges the v3 gate used (`498249777`, `8d851ea72`, `78a08e4d4`, `3db42db1f`). All four
  produce exactly one entry, equal to the merge SHA itself, regardless of how many total commits
  (2, 4, 9, 15) were in the branch.
- **Counterexample 1 — second merge lands after restoration.** Built a synthetic history: one
  reviewed one-commit PR merged via `--no-ff` (`merge_sha`), then a second "sneaky" PR merged on
  top before restoration completes. `post_restore_main_sha != merge_sha` (tip-equality check
  correctly fails); `git rev-list --first-parent --merges base..post_restore_main_sha` also shows
  two entries. Both sub-checks independently catch it.
- **Counterexample 2 — direct push (non-merge commit) during the window.** Same reviewed merge,
  followed by a direct non-merge commit on `main`. Tip equality fails (`post_restore_main_sha !=
  merge_sha`); `rev-list --merges` alone would *not* show this (it only lists merge commits), but
  the design correctly pairs the tip check with the merge-count check rather than relying on
  `--merges` alone, so the direct push is still caught.
- **Non-counterexample — PR branch with internal merge commits.** Built a PR branch that itself
  contains an internal merge (of a side branch, not of `main`), then merged that branch into
  `main` with `--no-ff`. `git rev-list --first-parent --merges base..merge_sha` correctly returns
  only `merge_sha` — the internal branch-side merge is excluded by `--first-parent`, confirming
  the design's claim that "branch commits may legitimately remain in the range" without producing
  a false positive. This was one of the three counterexample shapes the task specifically asked
  me to construct, and it does not break the design.

All three constructed scenarios behave exactly as v4 claims: correct runs pass, both intervening-
merge and direct-push shapes fail, and legitimate branch-internal merges do not cause false
positives.

### 3. Hash canonicalization and sequence 5 sanitization — HOLDS

v4 adds an explicit `json_hash_canonicalization` field (`json.dumps(obj, sort_keys=True,
separators=(',',':'))`, UTF-8, no indent, no trailing newline) and adds a recursive `url`/
`contexts_url` strip to sequence 5.

Independent live reproduction (read-only, ambient `gh` auth):

```
GET repos/jerudnik/jcode/rulesets/18509013  → sanitize (drop id/node_id/_links/source/
    created_at/updated_at/current_user_can_bypass) → canon-hash
    = 8440214dee8621d8a12a9456083a1c3afc82442291fd8a67ddcea7852d239124   MATCH, parsed-equal True

GET repos/jerudnik/jcode/rulesets/18509016  → same sanitize → canon-hash
    = 1376e3835feca779dd1dd2387e7cb5e1095f34c6de71ae64483c17e52823f99f   MATCH, parsed-equal True

GET repos/jerudnik/jcode/branches/main/protection → recursive url/contexts_url strip → canon-hash
    = d20823253081aca9537b632d1b8605a72d8838f520fe0d14defa7dc2d76b4704   MATCH, parsed-equal True
```

All three hashes reproduce exactly against the live repository today, using only the
canonicalization the document now states, and sequence 5's live response is parsed-equal to the
embedded object after the stated `url`/`contexts_url` strip. This closes both halves of v3
gate Finding 4 (unstated encoder, and sequence 5's missing sanitization clause).

### 4. `.github/scripts/` protected in both lists, and broader coverage check — HOLDS with one residual gap noted (non-blocking)

`template_variables.protected_paths` (governance JSON) and the `protected=(...)` array in
`governance-root.yml` (workflow patch) both list `.github/scripts` consistently.

Broader check (the task specifically asked me to look for what v3 admitted it hadn't fully
enumerated): I grepped every required workflow (`ci.yml`, `fork-ci.yml`, `security.yml`,
`nix.yml`, `fork-health.yml`) for local script/action references. Beyond `.github/scripts/`,
the required-context-gating jobs also execute roughly twenty `scripts/check_*.{sh,py}` files
(warning budget, code-size budget, panic budget, dependency boundaries, TUI render-lock,
env-lease drop order, swallowed-error budget, agent-instructions contract, real-home isolation,
ambient-roots, security preflight, etc.) that are **not** in `protected_paths` or
`governance-root.yml`'s protected array. Modifying one of these silently changes what `Fork CI
Gate` or `Security Gate` actually enforces without turning `Governance Root` red or being caught
by sequence 6/8's diff.

I judge this **non-blocking** for the same reason the v3 gate rated the analogous
`.github/scripts/` gap material rather than blocking before it was fixed: it degrades detection
(these checks can drift silently) rather than creating a lockout, D031 accepts the owner-admin as
root of trust for exactly this class of residual, and the design's own §3 already disclaims
completeness ("a single PR could in principle change the summary jobs, the comparator, and
`governance-root.yml` together... `Governance Root` is therefore an audit gate"). The R07
contract's acceptance gates require ruleset hardening, required-context presence, fixture
coverage, mapping-ledger correctness, and archive ratification — they do not require an
exhaustively enumerated protected-path list, and no acceptance gate is violated by this residual.
It is worth fixing cheaply (see Recommendations) but does not block PASS.

### 5. TOCTOU mitigation (sequence 8) and rollback correctness — HOLDS

v4 has sequence 8 repeat sequence 6 in full immediately after the ruleset read-back, with a
`rollback_body` restoring the sequence-3 pre-write ruleset on mismatch.

Independent live write-probe (ambient admin token via `rbw get jcode-temp-admin-key`, inline
only, never echoed/stored, disabled enforcement, nonexistent-branch target throughout):

- Created a scratch ruleset (`enforcement: disabled`, target
  `refs/heads/__gate-v4-scratch-nonexistent__`, name `gate-v4-scratch-test`).
- `PUT` a modified body (different name, extra rule) onto it — succeeds, read-back confirms the
  change.
- `PUT` the `rollback_body` shape v4 embeds (name/target/enforcement/bypass_actors/conditions/
  rules, i.e. `source_type` omitted) — succeeds and the read-back is byte-for-byte the original
  pre-modification state. The rollback body's schema is valid for a ruleset PUT and actually
  restores the prior ruleset.
- Deleted the scratch ruleset; confirmed 404 on re-GET. No scratch rulesets remain
  (`gh api repos/jerudnik/jcode/rulesets --jq '.[] | select(.name|startswith("gate-v4-scratch"))'`
  returns nothing).
- Tested the residual claim directly: `PUT` with an `If-Match` conditional header against a real
  ruleset returns HTTP 400 `"Conditional request headers are not allowed in unsafe requests
  unless supported by the endpoint"` — the ruleset PUT endpoint does not honor ETag/If-Match
  preconditions at all. This substantiates, rather than merely repeats, the design's residual-risk
  claim that "GitHub's ruleset PUT has no expected-main-SHA precondition."

The rollback mechanism is real and the residual framing is accurate, not an unacknowledged
overclaim: §3, §4, and `residual_risk` all state the narrowed-but-nonzero window explicitly rather
than claiming serialization.

## Regression checks: all clean

- `STATE.proposed.json`, `mapping-ledger.proposed.json`, `archive-manifest.proposed.json`:
  SHA-256 identical to `c27b1d8b2` (`e1c4e8bb...`, `88a1fdfd...`, `20f1a0dc...`), confirmed by
  direct hash comparison of both commits' blobs.
- `github-governance.proposed.json` sequences 1, 2, 7, 9, 10, 11, 12, 13, 15, 16, 17: object-equal
  to `c27b1d8b2` (confirmed by parsed-JSON comparison); only 3, 4, 5, 6, 8, 14 plus
  `abort_policy`/`residual_risk` changed, matching exactly the v3 gate's five-item list plus the
  checkpoint text it implies.
- Step sequencing: contiguous 1-17, first write at 7, checkpoint at 14 gating DELETE at 15,
  unchanged.
- Steady-state ruleset shape (sequence 7 write body), confirmed unchanged and contract-correct:
  `deletion`, `non_fast_forward`, `pull_request` (0 approvals, thread resolution required,
  `allowed_merge_methods: ["merge"]`), `required_status_checks` (strict, 4 contexts — Governance
  Root, Fork CI Gate, Security Gate, Nix Gate — all `integration_id: 15368`), `bypass_actors: []`.
- Sequence 15 (classic-protection DELETE) remains last write; sequence 12 (repository PATCH)
  disables squash/rebase (`allow_merge_commit: true`, `allow_squash_merge: false`,
  `allow_rebase_merge: false`), unchanged.
- Repository identity: live `GET repos/jerudnik/jcode` returns `id: 1238606714`,
  `full_name: jerudnik/jcode`, `owner.login: jerudnik` — matches `expected_repository_id` and
  sequence 1's assertion.
- `workflow-contexts.proposed.patch` applies cleanly to current `main` in a disposable worktree
  (`git apply --check` and a real apply), producing all ten workflows (`governance-root.yml` new,
  plus `fork-ci.yml`, `security.yml`, `nix.yml`, `fork-health.yml` modified with `.github/scripts`
  added to the protected array and each summary-gate job's `needs`/routing logic).
- `actionlint` (fetched via `nix shell nixpkgs#actionlint`, available on this machine) runs clean
  (exit 0, no findings) over all ten patched workflow files, including `governance-root.yml`,
  `fork-ci.yml`, `security.yml`, and `nix.yml` individually and as a full-directory pass. This was
  a gap in the v3 gate ("actionlint is not installed... did not independently re-confirm it") that
  I was able to close for v4.
- `c27b1d8b2..886f1a383` diff by name-status touches exactly three files
  (`design.md`, `github-governance.proposed.json`, `workflow-contexts.proposed.patch`) — no state,
  mapping, or archive weakening rode along.

## Edge cases considered

- Does the local-git mechanism actually work against a genuinely fresh, network-obtained shallow
  clone, not just a worktree of an already-full local repo? Yes — I ran the entire prepare/
  assertions sequence (unshallow, dual-SHA fetch, ancestry check, protected-path diff) end to end
  in a clone created with `git clone --depth 1 https://github.com/jerudnik/jcode.git`, using real
  historical SHAs, and it produced the correct result at every step.
- Does `git rev-list --first-parent --merges` avoid false positives from a PR branch that itself
  contains merge commits? Yes, constructed and verified (see item 2 above).
- Is the rollback body schema (`source_type` omitted, relative to the read GET response) actually
  accepted and does it restore state? Yes, verified with a real write/read/restore/delete cycle
  against a disposable, disabled-enforcement, nonexistent-branch-targeted scratch ruleset.
- Does GitHub's ruleset PUT support any conditional-write precondition that would let the design
  claim more than it does? No — a real `If-Match` probe was rejected outright by the endpoint
  (HTTP 400, "not allowed in unsafe requests unless supported by the endpoint"), confirming the
  residual-risk statement is accurate rather than a missed opportunity.
- Blast radius if the identified non-blocking gap (item 4) were exploited: a `scripts/check_*`
  modification changes what `Fork CI Gate`/`Security Gate` enforce without turning `Governance
  Root` red. This degrades detection under the owner-admin's own tooling; it does not create a
  lockout and does not let a non-owner bypass PR review, since the owner-admin remains the actor
  who would make such a change per D031.

## Validation performed

- Diffed `c27b1d8b2..886f1a383` by name-status and full unified diff.
- SHA-256 compared STATE/mapping-ledger/archive-manifest blobs between `c27b1d8b2` and
  `886f1a383`.
- Parsed-JSON compared every `github-governance.proposed.json` step object between `c27b1d8b2`
  and `886f1a383`.
- Live GET of both rulesets and classic branch protection; applied the stated sanitization;
  confirmed parsed-equality and canonical-hash equality for sequences 3, 4, 5.
- Fresh `git clone --depth 1` of the real repository; ran the design's exact unshallow, dual-SHA
  fetch, ancestry, and protected-path-diff commands end to end with real historical SHAs
  (`8d851ea72` → `498249777`).
- Reproduced the v3 gate's original 300-file-truncation range (`04fa8a3922...
  498249777c4...`) with local `git diff`, confirming no cap and correct detection of the two
  paths the API had dropped.
- Constructed and tested, in disposable clones, both rename-attack directions (protected→
  unprotected, unprotected→protected) from the v3 gate's exact scenario, plus a sanity check
  against a real historical rename commit (`e59985449`).
- Ran `git rev-list --first-parent --merges` against four real merges on `main`
  (`498249777`, `8d851ea72`, `78a08e4d4`, `3db42db1f`), matching the v3 gate's own sample.
- Constructed and tested, in disposable clones, three maintenance-window shapes: intervening
  second merge, direct push during the window, and a PR branch containing its own internal
  merge commit (verifying no false positive from `--first-parent`).
- Live write probe: created, modified, rolled back (via the literal `rollback_body` shape), and
  deleted a disabled-enforcement, nonexistent-branch-targeted scratch ruleset using
  `GH_TOKEN=$(rbw get jcode-temp-admin-key)` inline, never echoed or stored; confirmed no scratch
  artifacts remain.
- Live write probe: sent a `PUT` with an intentionally wrong `If-Match` header against a real
  ruleset to confirm GitHub rejects conditional headers on this endpoint outright.
- Applied `workflow-contexts.proposed.patch` to a disposable worktree of current `main`;
  confirmed clean apply and the expected ten-workflow result.
- Ran `actionlint` (via `nix shell nixpkgs#actionlint`) over all ten patched workflow files;
  confirmed clean exit.
- Grepped all required workflows for local script/action references to check the completeness of
  the protected-path lists beyond what v3's gate had flagged.
- Live GET of repository identity, confirming it matches `expected_repository_id`.

## What I did not check

- I did not open a real bootstrap or maintenance pull request, so live four-context emission and
  a live `Governance Root` failure-then-pass cycle for the patched workflows remain unobserved
  (same caveat as v1/v2/v3).
- I did not stress `allowReachableSHA1InWant`/`allowAnySHA1InWant` failure modes on a server that
  actually has them configured differently; GitHub-hosted `jerudnik/jcode` accepted every
  arbitrary reachable-SHA fetch I attempted, so I could not construct a case where the design's
  fetch step fails on this specific host. If the operator ever runs this apply against a
  differently-configured Git host mirror, that assumption would need re-verification.
- I did not re-verify the mapping-ledger's per-node tree/patch-equivalence proofs or the private
  recovery-archive `ls-remote`; those artifacts are byte-identical to v1/v2/v3, whose gates
  checked them, and v4 does not touch them.
- I did not fully enumerate every `scripts/*` file transitively imported or sourced by the
  ~20 `scripts/check_*` entries found in item 4; I stopped at direct workflow references, which is
  the same depth the design's own protected-path list operates at.
- I did not test the design's behavior under a genuinely rewritten `main` (force-push) live,
  since that is destructive; the `git merge-base --is-ancestor` mechanism is well-understood git
  semantics and I did not find a reason to doubt it, but it was not exercised against a real
  force-push on this repository.
- I did not evaluate GitHub Actions' own environment for the local-git steps (i.e., whether a
  hosted runner's default checkout is shallow, whether `actions/checkout` credentials permit the
  explicit-URL fetches used in `local_git.prepare`). All local-git testing was done from an
  ordinary developer machine with unauthenticated `https://github.com/...` fetches, which worked
  because the repository is public; this is very likely also how a human operator would run this
  apply document (it's an operator-run apply, not itself a CI job), but that operating context
  was not independently confirmed against the design's stated audience.

## Recommendation (non-blocking)

Add the ~20 `scripts/check_*.{sh,py}` files (and any others referenced by `run:` steps inside
required-context-gating jobs) to `protected_paths` and `governance-root.yml`'s protected array,
the same way `.github/scripts/` was added in this revision. This is a mechanical, same-shape
extension of the exact fix already applied for finding 4 across the two prior revisions in this
gate chain — it does not require a design change.

## Confidence: high

Every finding in the v3 gate's five-item list was independently reproduced as fixed against live
GitHub API state and against real, constructed git scenarios on this exact repository — not
merely re-read from the design document. The one residual gap I found (item 4's broader
`scripts/check_*` coverage) is demonstrated by direct grep and is rated non-blocking for reasons
tied to the R07 contract's acceptance gates and D031's explicit scope, not merely asserted.

## Final verdict: PASS
