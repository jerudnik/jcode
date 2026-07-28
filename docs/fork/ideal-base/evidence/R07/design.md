# R07 design v4: reviewed publication and auditable governance

Design baseline: `main` at `498249777c453c1d551aeb01fc45420d8ca0a585`.
Primary evidence: `automation/r07-recon:docs/fork/ideal-base/evidence/R07/recon.md`.
This document makes no GitHub write, pushes no ref, and does not edit coordinator-owned
`docs/fork/ideal-base/STATE.json` or any workflow. Exact coordinator/out-of-scope proposals
are recorded beside this design.

## 0. History: what changed in v1 -> v2, and why (rejected mechanism, kept only as record)

**This subsection is history, not current design.** The `workflows`-rule/trust-root/
immutable-transition mechanism it describes was proposed in v1, rejected by the v1 gate, and is
absent from every artifact in this v4 revision. It is retained here only so a reader can see why
the current mechanism (§0a, §4) looks the way it does; do not treat any noun below as a live
component.

Design v1 was gated adversarially and failed on one blocking finding: the repository-level
`workflows` ruleset rule is organization/enterprise-scoped and is rejected with `422 Validation
Failed` on `jerudnik/jcode`, a personal user-owned repository. v1 had made that rule the sole
external trust root justifying zero required approvals. `DECISIONS.md` D031 resolves the
trade-off: **the owner-admin is the accepted root of trust on this repository**, which was
already true de facto because the owner can rewrite or delete any ruleset. R07's
"self-checking" property is delivered by auditability, not by an anchor the platform will not
sell us.

Concretely, v2:

1. removed the `workflows` rule from `github-governance.proposed.json` and every artifact that
   referenced it;
2. deleted `governance-transition.proposed.template.json` and
   `governance-transition.workflow.template.yml`. Those two files existed only to bootstrap and
   maintain the required-workflow pin (materialize a candidate-free workflow, protect an
   immutable tag, pin the ruleset's workflow `ref`/`sha` to it, restore afterwards). With no
   `workflows` rule there was nothing to pin, so the entire immutable-transition protocol was
   dead code;
3. replaced the assertion that `repository_id: 1238606714` and `integration_id: 15368` are
   recon-derived with live preflight verification at apply time (§3, §4). The gate confirmed the
   repository id is correct but does not appear in recon at all, and no recon line names 15368
   either; both are now proved by read-back, not by citation;
4. recorded the residual risk this accepts (§4, §13) instead of a stop-condition that can never
   be satisfied.

The v2 STATE schema split and validator semantics, the equivalence ladder and mapping ledger, the
archive plan, the workflow-contexts patch, fail-closed sequencing, and the remaining stop
conditions were independently verified by the v1 gate and are unchanged in substance through v4.

## 0a. History: what changed in v2 -> v3, and why v3 still failed

**This subsection is history, not current design.** The v2 gate found a bootstrap-to-apply race,
an unbound maintenance window, inexact preflight comparisons, and overclaimed residual language.
v3 attempted to close those with an immediately-pre-write current-`main` comparison, a
transaction-bound maintenance procedure, embedded exact bodies/hashes, and narrower auditability
claims. The v3 gate reproduced three defects against the live API: the compare endpoint's `files`
array silently stopped at 300 entries, rename records placed the old path only in
`previous_filename`, and a legitimate one-commit PR produced two commits in the compare range.
It also recovered an unstated hash encoder, found sequence 5 omitted live-response sanitization,
found `.github/scripts/` missing from the protected set, and identified a sequence-6-to-7 race.

## 0b. What changed in v3 -> v4, and why

v4 repairs the v3 gate's complete five-item PASS list without changing the steady-state ruleset
shape or the byte-identical STATE, mapping-ledger, and archive-manifest proposals.

1. **Uncapped protected-path identity.** Sequence 6 now resolves `main` with a live GET,
   unshallows the apply clone read-only when needed, fetches the exact returned commit plus
   `bootstrap_merge_sha`, proves ancestry, and runs path-limited `git diff --quiet`. This compares
   endpoint tree/blob identity without a response array or file-count cap. A rename out of the
   protected set appears as deletion of its protected old path; a rename into it appears as addition.
2. **Executable maintenance-window proof.** Maintenance step 7 re-reads `main` and requires its
   tip still equal `merge_sha`, then requires `git rev-list --first-parent --merges
   expected_base_sha..merge_sha` to contain exactly one SHA, `merge_sha`. Step 5's ordered
   two-parent proof remains the binding to `expected_base_sha` and `head_sha`. Branch commits may
   legitimately remain in the range; an intervening merge or any later direct commit fails.
3. **Pinned hash and healthy classic-protection comparison.** The apply document pins
   `json.dumps(obj, sort_keys=True, separators=(',',':'))`, UTF-8, with no trailing newline.
   Sequence 5 recursively removes `url` and `contexts_url` before equality and hashing.
4. **Complete protected helper coverage.** `.github/scripts/` is protected both in the apply
   template and in `governance-root.yml`, because required workflows execute
   `.github/scripts/run_with_timeout.py`.
5. **Narrowed activation race with explicit residual.** Sequence 8 immediately repeats the full
   sequence 6 assertion after the first ruleset read-back. A protected-path change triggers
   restoration of the pre-write ruleset body and a stop while only that surface has changed.
   GitHub exposes no expected-main-SHA precondition on a ruleset PUT, so a smaller residual race
   remains after the repeat; v4 states it rather than claiming serialization it cannot provide.

## 1. Decisions

R07 has three independent durability boundaries. All three must close before R07 can be
accepted.

1. **Governance is manifest-driven and fail-closed.** `scripts/required-checks.json` becomes
   the single machine-readable desired state for required contexts, complete ruleset shape,
   absence of classic branch protection, and repository merge methods. Fixture mode proves
   the comparison without GitHub. Live mode compares the complete remote surfaces and exits
   non-zero when `gh`, authentication, authorization, a required field, or an endpoint is
   unavailable. It never warns and skips.
2. **Publication state records two identities.** Schema v2 replaces ambiguous `commit` with
   `reviewed_commit` and `published_commit`. A completed record is valid only when the
   reviewed object exists and the published commit is an ancestor of an explicit, fully
   fetched main ref. The mapping ledger records how the two identities were proved
   equivalent.
3. **Archive durability is ref-based, not object-database-based.** Every one of the 33
   non-main-ancestral reviewed identities gets a direct private archive head, and each of the
   six `archive/stash-*` tags is pushed under its existing tag name. `git ls-remote` proves
   ref identity; a fresh fetch plus `git cat-file`/`git fsck` proves object reachability.

Boundary 1 is an **audit** boundary, not an enforcement boundary against the owner. Per D031 the
owner-admin can change any ruleset; what R07 buys is bounded, not unconditional, visibility into
that. Precisely: the fixture matrix proves the comparator rejects each way the rule shape can be
wrong without touching GitHub; a live `--live` run at any moment proves the actual server state
equals or differs from the manifest at that moment; and the scheduled daily `fork-health.yml
--live` run re-proves this once every 24 hours and fails closed on drift or on insufficient
authorization it can observe. This is detection at sample points, not a continuous audit log: a
change made and reverted between two `--live` samples, or made during the §4 maintenance window
before its restoration step completes, is not guaranteed to be caught by this boundary alone (see
§4's window proof, which is the part of the mechanism that does close the reverted-during-window
case). Against everything other than the owner-admin — a contributor, a compromised workflow, a
fork PR — the ruleset remains a genuine server-side enforcement boundary, not merely an audit one.

The current accepted set does **not** need reopening under the equivalence ladder selected in
§7. If any listed proof fails when independently re-run, the affected node must be reopened
with an injected repair node. The ladder must not be weakened to keep a node accepted.

## 2. Ownership and proposal artifacts

| Artifact | Owner/action |
|---|---|
| `scripts/required-checks.json` | R07 implementation creates it. |
| `scripts/fork-health.sh` | R07 implementation adds explicit fixture/live modes and full comparison. |
| `scripts/ideal_base_railway.py` | R07 implementation adds schema v2, governance comparison, and ancestry semantics. |
| `tests/test_ideal_base_railway.py` | R07 implementation adds state and governance planted-failure tests. |
| `docs/fork/ideal-base/evidence/R07/STATE.proposed.json` | **Coordinator proposal only.** Full migrated schema-v2 content. Never copy partially. |
| `docs/fork/ideal-base/evidence/R07/mapping-ledger.proposed.json` | Full 35-node reviewed-to-published ledger and equivalence methods. |
| `docs/fork/ideal-base/evidence/R07/archive-manifest.proposed.json` | Exact 39-ref private archive write set. |
| `docs/fork/ideal-base/evidence/R07/github-governance.proposed.json` | Exact external GitHub configuration write set. Requires confirmation immediately before use. |
| `docs/fork/ideal-base/evidence/R07/workflow-contexts.proposed.patch` | Exact diff for workflow-owned paths outside R07. Coordinator must fold it into an authorized owner or amend the graph. |
| `docs/fork/ideal-base/STATE.json` and `docs/fork/ideal-base/DECISIONS.md` | Coordinator-owned. R07 must not edit them directly. |

**History.** v1 proposed two `governance-transition.*` artifacts specifying materialization,
immutable-tag protection, ruleset pinning, and restoration for a required-workflow rule. That
mechanism was rejected (§0) and those two files are deleted, not superseded or present anywhere
in v3's artifact set.

The workflow proposal is required, not optional. Without it, the four intended contexts are
not emitted on every pull request and ancestry runs in a depth-1 checkout. The coordinator
must explicitly resolve this ownership handoff before implementation integration.

## 3. Canonical governance manifest

`scripts/required-checks.json` should contain the whole comparison contract, not only a list
that a human must mentally join to other policy. Exact schema:

```json
{
  "schema_version": 1,
  "repository": "jerudnik/jcode",
  "repository_id": 1238606714,
  "target_branch": "main",
  "github_actions_integration_id": 15368,
  "required_checks": [
    { "context": "Governance Root", "integration_id": 15368 },
    { "context": "Fork CI Gate", "integration_id": 15368 },
    { "context": "Security Gate", "integration_id": 15368 },
    { "context": "Nix Gate", "integration_id": 15368 }
  ],
  "workflow_contracts": [
    {
      "context": "Fork CI Gate",
      "file": ".github/workflows/fork-ci.yml",
      "job_id": "fork-ci-gate",
      "if": "always() && github.event_name == 'pull_request'",
      "needs": ["changes", "governance-contract", "quality", "macos", "linux-tests"],
      "routing": {
        "quality": "rust || scripts",
        "macos": "rust",
        "linux-tests": "rust"
      }
    },
    {
      "context": "Security Gate",
      "file": ".github/workflows/security.yml",
      "job_id": "security-gate",
      "if": "always() && github.event_name == 'pull_request'",
      "needs": ["detect-dependency-changes", "secret-scan", "dependency-audit"],
      "routing": { "dependency-audit": "deps" }
    },
    {
      "context": "Nix Gate",
      "file": ".github/workflows/nix.yml",
      "job_id": "nix-gate",
      "if": "always() && github.event_name == 'pull_request'",
      "needs": ["validate", "matrix", "build"],
      "pull_request_paths_filter": "forbidden"
    }
  ],
  "classic_branch_protection": "absent",
  "repository_merge_methods": {
    "allow_merge_commit": true,
    "allow_squash_merge": false,
    "allow_rebase_merge": false
  },
  "rulesets": {
    "protect-fork-rails": "exact body in §4",
    "no-stray-branches": "exact body in §4"
  }
}
```

The implementation should store the two rule bodies as JSON objects rather than the strings
shown above. Array comparison is order-insensitive where GitHub does not promise response
order, but set equality is exact. Unknown active branch rulesets, unknown required contexts,
unknown bypass actors, missing keys, and extra merge methods fail. Response-only keys such as
`id`, `_links`, timestamps, `node_id`, `source`, and `current_user_can_bypass` are sanitized
before comparison and never become desired-state inputs.

`repository_id` and `github_actions_integration_id` are desired-state inputs like any other:
live mode reads `GET /repos/jerudnik/jcode` and compares `id`, and compares the `integration_id`
on every required status check. Their provenance is §4's preflight, not this document — see the
provenance note below.

The validator must also inspect every `.github/workflows/*.yml` file with a constrained,
fail-closed workflow extractor. It requires exactly one literal job `name:` definition for each
required context, at the declared file/job id, with the exact `needs`, `if`, routing outputs,
and pull-request trigger contract above. Duplicate names, dynamic names, YAML constructs the
extractor cannot classify, route drift, or a workflow-level pull-request path filter fail. This
detects the same-GitHub-Actions-app spoofing gap that `integration_id` alone cannot close.

**What this detection is and is not.** Every check above runs from the pull request head, so a
single PR could in principle change the summary jobs, the comparator, and `governance-root.yml`
together. (History: v1 answered that with a server-side `workflows` rule pinned to
`refs/heads/main`; that rule is unavailable on this account and per D031 no substitute is
claimed — see §0.) `Governance Root` is therefore an **audit gate**: it makes a governance-path
change loud on the PR that makes it (a red required context, and a named diff in the log) rather
than impossible to make. What this concretely proves, and only this: (a) at the moment a pull
request's head is evaluated, `Governance Root` truthfully reports whether that PR's diff touches
a protected path; (b) `github-governance.proposed.json` sequence 6 proves, immediately before the
initial apply's first write and again immediately after the first ruleset read-back, that current
`main`'s protected-path bytes equal what was reviewed at the bootstrap merge (§4); and (c) the
§4 maintenance procedure's tip and merge-commit proof shows, after the fact, whether only the
reviewed merge landed during a deliberate drop of the `Governance Root` requirement.
None of these is a continuous audit log, and none proves that every historical governance change
was reviewed — only that the specific transitions this design controls (initial apply, and each
maintenance window) are bounded and checked. Against non-owner actors the required contexts plus
the ruleset remain a real server-side boundary, because a fork PR cannot merge itself.

**Identifier provenance.** `repository_id: 1238606714` and `integration_id: 15368` are *not*
established by recon. The design gate confirmed the repository id is correct but appears nowhere
in `recon.md`; likewise no recon line names 15368, only the app slug `github-actions` on PR #38
check runs. Both values are therefore treated as unverified inputs until proved live:
`github-governance.proposed.json` sequences 1 and 2 read `GET /repos/jerudnik/jcode` and
`GET /repos/jerudnik/jcode/commits/{sha}/check-runs` and stop the apply before any write if
either differs. The integration pin excludes another app; the unique workflow contract above
separately excludes a second repository workflow from emitting the same name.

## 4. Exact server-side target and diff from recon §2

The authoritative apply document is `github-governance.proposed.json`. It opens with six
read-only preflight asserts (repository identity and id; required-context integration id and
`Governance Root`'s own failure conclusion on the bootstrap head; both rulesets' full bodies,
exactly, bound to their expected names; the classic-protection exact baseline; and, last,
immediately before the first write, current `main`'s protected-path bytes proved identical to
the reviewed bootstrap-merge state) so that no identifier or surface in this design is trusted on
the strength of citation alone. The desired ruleset bodies are:

Every embedded JSON SHA-256 uses one pinned encoder after the assertion's sanitization:
`json.dumps(obj, sort_keys=True, separators=(',',':'))`, encoded as UTF-8 with no indentation and
no trailing newline. Sequence 5 recursively removes every `url` and `contexts_url` member from the
live classic-protection response before parsed-object equality and hashing; this is the same class
of volatile-link sanitization sequences 3-4 state for rulesets.

```json
{
  "name": "protect-fork-rails",
  "target": "branch",
  "enforcement": "active",
  "bypass_actors": [],
  "conditions": {
    "ref_name": {
      "include": ["refs/heads/main"],
      "exclude": []
    }
  },
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    {
      "type": "pull_request",
      "parameters": {
        "allowed_merge_methods": ["merge"],
        "dismiss_stale_reviews_on_push": false,
        "require_code_owner_review": false,
        "require_last_push_approval": false,
        "required_approving_review_count": 0,
        "required_review_thread_resolution": true
      }
    },
    {
      "type": "required_status_checks",
      "parameters": {
        "do_not_enforce_on_create": false,
        "required_status_checks": [
          { "context": "Governance Root", "integration_id": 15368 },
          { "context": "Fork CI Gate", "integration_id": 15368 },
          { "context": "Security Gate", "integration_id": 15368 },
          { "context": "Nix Gate", "integration_id": 15368 }
        ],
        "strict_required_status_checks_policy": true
      }
    }
  ]
}
```

```json
{
  "name": "no-stray-branches",
  "target": "branch",
  "enforcement": "active",
  "bypass_actors": [],
  "conditions": {
    "ref_name": {
      "include": ["~ALL"],
      "exclude": ["refs/heads/main", "refs/heads/automation/**"]
    }
  },
  "rules": [
    { "type": "creation" }
  ]
}
```

This is exactly the rule set the R07 contract in `WORK_GRAPH.json` enumerates: deletion and
non-fast-forward protection, changes through pull requests, zero required approvals,
review-thread resolution, merge commits only, strict required checks, and no silent
administrator bypass. The contract never asked for an external trust root; that was a
design-layer addition in v1 (D031).

Diff from the live state recorded by recon:

| Surface | Current | Target |
|---|---|---|
| `protect-fork-rails.rules` | `deletion` only | Add `non_fast_forward`, complete `pull_request`, and strict `required_status_checks`. |
| `protect-fork-rails.bypass_actors` | `[]` | Remains exactly `[]`. |
| `no-stray-branches.bypass_actors` | repository role 5, `always` | `[]`; stale-rail creation has no administrator escape hatch. |
| Classic protection | `strict:false`, `enforce_admins:false`, weak `Detect changes`, no PR rule | Delete only after the active ruleset is read back and verified. A contradictory second layer must not remain. |
| Repository merge methods | merge, squash, rebase | merge only. |

The recon baseline in the middle column is exact-compared, sanitized parsed body and canonically
encoded hash, by preflight sequences 3-5 before any write; if live governance has drifted from
what recon captured, the apply stops
rather than overwriting an unreviewed state. Sequence 6, separately, proves current `main`'s
protected-path bytes are unchanged since the bootstrap merge (below); it does not compare against
recon, because recon predates the bootstrap PR that first wrote those paths.

The write order is executable in schema-v4 `github-governance.proposed.json`: six preflight reads
first, then every write followed by a named `read_assert`, and an explicit checkpoint forbids
the classic-protection DELETE until the new ruleset, effective main rules, no-stray ruleset,
and repository merge methods all match. If any read-back differs, stop before the next write.
These are external writes and require explicit confirmation immediately before execution.

### Ordering, and the current-main preflight assertion that replaces a bootstrap pin

(History: v1 serialized the bootstrap merge with an immutable-tag-pinned required workflow,
because the rejected `workflows` rule made a candidate-free trust root available at bootstrap
time — see §0. That mechanism does not exist in v4.) The ordering constraint that actually holds
is narrower, and v2 gate finding 1 showed that sequencing text alone is not sufficient to enforce
it — an executable preflight assertion is required. v4's mechanism:

1. The bootstrap pull request lands the four context definitions **before** any of them is
   required. It merges under the pre-R07 regime, where the only required context is the weak
   `Detect changes`. Requiring a context whose defining workflow is not yet on `main` would
   deadlock the repository, so this ordering is mandatory, not stylistic.
   The bootstrap pull request must originate from a branch in `jerudnik/jcode` itself, not a
   fork, so that the added workflow files are instantiated with the repository's own
   permissions and every context is actually emitted. Preflight sequence 2 verifies exactly
   that by requiring all four check runs on the reviewed head SHA, and additionally requires
   `Governance Root` to conclude `failure` there (v2 gate finding 3).
2. `Governance Root` is expected to conclude **failure** on that bootstrap PR, because the
   bootstrap PR is by construction a governance-path change. Nothing is required yet, so it does
   not block; record the failure and the three green contexts in evidence. Any other conclusion
   (for instance `Governance Root` passing on a PR that changes `.github/workflows/**`) means
   the gate is not wired correctly and is a stop — sequence 2 makes this an executable assertion,
   not only a prose expectation.
3. The bootstrap merge is verified as main's new tip with the reviewed base/head parents
   (§11 barrier 4), and its resulting merge commit SHA is recorded as `bootstrap_merge_sha`.
4. Immediately before the ruleset apply's first write, `github-governance.proposed.json`
   sequence 6 resolves current `main`'s exact SHA with `GET repos/jerudnik/jcode/commits/main`,
   unshallows the apply clone read-only when needed, fetches that commit and
   `bootstrap_merge_sha`, proves ancestry, and runs
   `git diff --quiet bootstrap_merge_sha current_main_sha -- <protected
   paths>`. The protected set is `.github/scripts/`, `.github/workflows/`, the manifest,
   `fork-health.sh`, the railway validator and its tests, and this apply document itself — the
   same list `governance-root.yml` checks. This path-limited tree comparison has no response cap;
   a rename away from a protected prefix appears as deletion of the old protected path, and a
   rename into one appears as addition. Only after this passes is the ruleset applied requiring
   all four contexts.

**What this proves about an intervening merge, precisely.** If another pull request merges
between the bootstrap merge and the ruleset apply and does not touch a protected path, sequence 6
observes an empty local protected-path tree diff and passes: the four context definitions sequence
2 proved emitting are still exactly what is on `main`, so requiring them is safe. If an intervening pull
request does touch a protected path — whether through an unauthorized bypass of `Governance Root`
or through a legitimate, correctly-executed §4 maintenance window — sequence 6 fails closed: the
apply stops, and the operator must re-review current `main`'s actual state (and, if the change was
legitimate maintenance, re-capture a current baseline) before any write proceeds. There is no
claim that an intervening merge is safe merely because it happened; the claim is narrower and now
executable: an intervening merge that left the protected paths untouched is safe, and this is
verified live at apply time rather than inferred from the historical bootstrap head.

**Activation TOCTOU mitigation and residual.** Sequence 8 first reads back the ruleset written by
sequence 7, then immediately obtains a fresh live `main` SHA and repeats sequence 6 in full. If the
boundary was rewritten or any protected path changed, sequence 8 restores the exact pre-write
ruleset body captured by sequence 3, verifies that restoration, and stops while no other surface
has been changed. This catches a merge between the initial sequence 6 read and the first ruleset
read-back. GitHub's ruleset PUT has no expected-main-SHA precondition, so the assertion and write
cannot be made atomic: a smaller residual race remains after the repeat and before the apply
finishes. The operator minimizes that interval and permits no concurrent merges, but v4 does not
claim the platform serializes it.

### Later governance maintenance (transaction-bound)

A subsequent legitimate change to `.github/scripts/**`, `.github/workflows/**`, the manifest, the
comparator, the railway validator, or `github-governance.proposed.json` will make `Governance Root` red, and a
red required context blocks the merge for that PR alone — the other three required contexts stay
live and unaffected. v2 gate finding 2 found that dropping `Governance Root` from
`required_status_checks` repository-wide for an unbound interval, with no PR/head/base capture and
no proof only the reviewed PR merged in that interval, left this window exploitable by a wrong or
sloppy PR merge, not only by the owner-admin's accepted authority. v4 binds the transaction the
same way the (history-only, rejected) v1 immutable-transition protocol bound the bootstrap step:

1. **Capture identity before any write.** Record the exact PR number, source repository/ref, the
   reviewed head SHA (`head_sha`), and current `main`'s tip as the expected base SHA
   (`expected_base_sha`). Independently review the diff at exactly `head_sha` against this design
   and the manifest. Confirm `Governance Root` fails on `head_sha` and names exactly the protected
   paths this PR changes; capture that check-run output as evidence.
2. **Record the pre-change ruleset, exactly.** Read the complete current `protect-fork-rails`
   body and its SHA-256 using the apply document's pinned canonical encoder. This is the literal
   object restored in step 6, not a freshly authored equivalent — restoration is defined as
   parsed-body equality plus equality to this captured hash, not as re-applying
   `github-governance.proposed.json`'s target body (which differs from live state only by the
   temporary absence of `Governance Root`).
3. **Drop the one context, read back, start the window clock.** `PUT` the ruleset with
   `Governance Root` removed from `required_status_checks` and every other sanitized field
   parsed-equal to step 2's body. Read the ruleset back, record the sanitized post-change body and
   its canonical hash, and record the wall-clock window start.
4. **Merge conditioned on the exact reviewed head.** `PUT
   /repos/jerudnik/jcode/pulls/{pr_number}/merge` with `sha: head_sha` and `merge_method: merge`.
   If the API rejects the head-SHA precondition (the PR's head moved since step 1's review), stop;
   do not retry unconditioned. This is the binding that prevents merging a substituted or
   force-pushed PR under the same number.
5. **Verify the merge landed exactly as reviewed.** Confirm the response has `merged: true`;
   confirm `GET repos/jerudnik/jcode/commits/main` resolves to the response's merge SHA
   (`merge_sha`); confirm `GET repos/jerudnik/jcode/commits/{merge_sha}` reports exactly two
   parents equal to `expected_base_sha` and `head_sha`.
6. **Restore the exact pre-change body, read back, close the window clock.** `PUT` the ruleset
   back to the literal body captured in step 2 (not a freshly authored one); read it back and
   confirm its hash equals step 2's hash exactly. Record the wall-clock window end.
7. **Executable proof no other merge or commit landed in the window.** Re-read
   `GET repos/jerudnik/jcode/commits/main`, set `post_restore_main_sha` to its full `sha`, and assert
   `post_restore_main_sha == merge_sha` from step 5. Unshallow the local clone read-only if needed,
   fetch `expected_base_sha` and `post_restore_main_sha`, and run
   `git rev-list --first-parent --merges expected_base_sha..post_restore_main_sha`; assert its
   complete output has exactly one entry and that entry equals `merge_sha`. `--first-parent`
   measures merges onto `main`, not merge commits that may exist inside the reviewed PR branch.
   Do **not** assert the range has one total
   commit: the reviewed branch's commits legitimately appear in the range. Step 5's ordered
   two-parent proof (`parents == [expected_base_sha, head_sha]`) binds that sole merge to the exact
   reviewed base and head; tip equality catches a later direct/non-merge commit as well. This is
   checked after restoration, so it catches anything that landed between steps 5 and 6. On any
   mismatch, stop and treat it as a governance incident requiring out-of-band investigation, not
   as routine maintenance to be closed by re-running step 6.
8. **Post-merge comparison from two independent comparator implementations.** Run
   `scripts/fork-health.sh --live` twice: once checked out at the *pre-window* `expected_base_sha`
   (the reviewed comparator as it existed before this PR could have altered its own judgment), and
   once at `merge_sha` (the new, post-merge comparator). Both runs must independently confirm the
   restored `protect-fork-rails` body matches the exact steady-state target. Record both
   transcripts; a disagreement between them is a stop, since it means the PR being merged changed
   what the comparator itself considers compliant.

**What this does and does not close.** Steps 1-8 bind PR identity, condition the merge on the
exact reviewed head SHA, and executably prove after the fact that `main` still ends at the reviewed
`merge_sha` and the range contains exactly that one merge commit (step 7 subsumes "no wrong PR
merged" and "no later direct push happened" while allowing the reviewed branch's own commits).
What this cannot do, and does not claim to do: prevent a *second, ordinary* pull request from
merging during the window through the normal PR flow, since `Fork CI Gate`, `Security Gate`, and
`Nix Gate` remain required and unaffected by dropping `Governance Root` alone — full binding
would require pausing all merges, which this platform's ruleset API does not offer short of
making the branch fully unmergeable (a stronger and separately costly step outside this design's
scope). Step 7 detects this case (the tip differs and the range contains a second merge commit) but
does not prevent it. This residual is scoped, not overclaimed, per D031: the owner-admin who runs
this procedure is the accepted root of trust and controls when a second PR is allowed to merge in
practice; the mechanism above exists to make it *provable after the fact* whether that discipline
held, which is what an independent reviewer needs to check.

**Residual risk, accepted per D031.** The owner-admin can change or delete any of these rules at
any time, with or without this procedure, and nothing in R07 prevents it. R07 does not enforce
against the root of trust; it bounds and makes provable what happens inside the one deliberate
bypass window the procedure opens, and it makes deviation from that procedure detectable. Beyond
this specific window, detection still comes from three independent places: the governance
fixtures prove the comparator rejects each wrong rule shape offline; a live `--live` run at any
moment proves whether the actual server state equals the manifest at that instant; and the
scheduled fork-health run repeats that instantaneous check daily and exits non-zero on drift or on
a credential that cannot see `bypass_actors`. None of these three constitutes a continuous audit
log of every historical change; §1 states this precisely. A silent, undetected weakening of
governance made *outside* the §4 procedure would require the owner-admin to also alter the
manifest, the comparator, and the scheduled workflow consistently, all of which are themselves
protected paths under `Governance Root` and tracked in Git history — that remains true and is not
weakened by the above.

## 5. Required context emission and CI depth

Four contexts are required. Each is created on every pull request:

| Context | Summary semantics |
|---|---|
| `Governance Root` | Audit gate for governance-path changes. Fails any pull request that touches `.github/scripts/**`, `.github/workflows/**`, the manifest, the comparator, the railway validator, its tests, or the apply document, and names the offending paths. It runs from the PR head like every other job, so it detects rather than prevents; see §3 and §4. |
| `Fork CI Gate` | Requires `Detect changes` and `Governance Contract Gate` success. Quality must succeed iff `rust || scripts`; macOS/Linux must succeed iff `rust`. An applicable skip or inapplicable run fails. |
| `Security Gate` | Requires dependency detection and secret scan success. Dependency audit must succeed iff `deps`; an unexplained skip fails. |
| `Nix Gate` | Requires fast validation, matrix selection, and the real x86_64-linux package build/smoke success. |

`Detect changes` is deliberately not required because it is routing, uses `continue-on-error`,
and defaults outputs to true. Conditional jobs are not required directly. Third-party Cursor
contexts are not required because the repository does not control their names or emission.

`workflow-contexts.proposed.patch` makes five exact out-of-scope changes:

1. Adds `governance-root.yml`, which fails any pull request that changes a workflow file or an
   R07 governance file and prints the offending paths. It is a required audit context, not a
   trust root (§3): no repository-level rule pins it against modification, by design decision
   D031 on this personal-repository account (§0 records why that path was rejected in v1).
2. Adds a `Governance Contract Gate` job to `fork-ci.yml` with `fetch-depth: 0`, an explicit fetch of
   `main` to `refs/remotes/origin/main`, a non-shallow assertion, railway tests/check, and the
   valid fork-health fixture.
3. Adds the always-running `Fork CI Gate` and `Security Gate` summary jobs. Their shell checks
   consume routing outputs and require `success` for applicable jobs and `skipped` only for
   inapplicable jobs.
4. Removes the Nix workflow-level pull-request `paths:` filter and adds `Nix Gate`. This is the
   narrowest lockout-proof change. It intentionally builds/smokes the package on every PR;
   later optimization may move relevance inside jobs, but may never restore a workflow-level
   filter while `Nix Gate` is required.
5. Gives scheduled `fork-health.yml` live mode a dedicated `RULESET_AUDIT_TOKEN`. The default
   `GITHUB_TOKEN` cannot be trusted to return `bypass_actors`, which GitHub documents as visible
   only to a caller with ruleset write access.

The required pull-request governance job proves the repository contract and fixture comparator;
it does **not** claim that an untrusted PR can read privileged live governance. Actual server
state is proved by the trusted scheduled/manual live workflow and the post-enforcement live
read-back in §11. Those are separately required R07 evidence.

The railway may only make an authoritative ancestry statement when
`git rev-parse --is-shallow-repository` returns `false`. A shallow result is an environmental
error, not evidence that a commit is unrelated. `refs/remotes/origin/main` must resolve before
validation. No fallback to `HEAD` is allowed in CI.

## 6. Fork-health modes and full comparison

The script gets two mutually exclusive governance sources:

- `--fixture PATH`: no GitHub access. Load a sanitized aggregate snapshot and compare it to
  `scripts/required-checks.json`.
- `--live`: require `gh`, a working authenticated API call, access sufficient to return
  `bypass_actors`, and successful reads of branch list, both full rulesets, effective rules for
  `main`, classic protection, and repository merge settings.

Omitting both is usage error 2. Supplying both is usage error 2. Live acquisition errors are
exit 2 with the failing endpoint named. Governance mismatches are accumulated and exit 1. A
missing `bypass_actors` key is authorization failure, not an empty list. There is no `WARN` or
silent skip for a remote invariant.

Because live comparison is now the primary drift detector for governance (§4, D031), its
coverage is load-bearing rather than confirmatory. The comparison must cover:

- exact ruleset names and active enforcement;
- exact include/exclude ref conditions;
- exact rule types and all relevant parameters;
- exact repository id, so the comparison cannot silently succeed against a different repository;
- absence of any rule type not in the manifest, so an added rule is a mismatch rather than an
  ignored extra;
- exact bypass actors, including unexpected actors on non-main rulesets;
- effective `main` rule types;
- classic protection absence;
- repository merge methods;
- required context names and GitHub Actions integration IDs;
- exactly one workflow definition for each required context, with exact summary dependencies,
  routing logic, and trigger/path-filter contract;
- maintained `main`, allowed `automation/**` topics, and no stale rail names.

Local checks that do not need GitHub remain local, but the script must not print "all invariants
hold" unless the selected governance source was successfully validated.

## 7. Governance fixture matrix

Use one on-disk valid aggregate fixture under
`docs/fork/ideal-base/evidence/R07/fixtures/governance-valid.json`. Tests deep-copy it, mutate
one property, run `fork-health.sh --fixture`, and assert the diagnostic. This matches the
existing planted-failure idiom without adding an unowned `tests/fixtures/**` path.

| Case | Mutation | Expected result |
|---|---|---|
| valid | Exact target from §§3-4 | 0 |
| missing PR enforcement | Remove `pull_request` | 1, names missing rule |
| wrong approvals | `required_approving_review_count: 1` | 1 |
| wrong merge methods | Add `squash` or `rebase`, or remove `merge` | 1 |
| force-push permission | Remove `non_fast_forward` | 1 |
| deletion permission | Remove `deletion` | 1 |
| unresolved threads allowed | Set resolution false | 1 |
| checks not strict | Set strict policy false | 1 |
| missing required context | Remove each summary context in parameterized subtests | 1 |
| wrong repository | Change the repository id in the live/fixture snapshot | 1 |
| unexpected extra rule | Add any rule type absent from the manifest | 1 |
| governance-root context missing | Remove `Governance Root` from required checks | 1 |
| stale/extra context | Add `Detect changes` or a historical name | 1 |
| spoofable context | Null or change integration id | 1 |
| duplicate context definition | Add the same required job name to another workflow | 1 |
| summary dependency drift | Remove or add a `needs` entry | 1 |
| unexplained applicable skip | Mark Rust/dependency routing true with the routed job skipped | summary fails |
| wrong inapplicable run | Mark routing false with the routed job successful | summary fails |
| workflow trigger lockout | Add a pull-request `paths:` filter to a required-context workflow | 1 |
| stale rail | Add `vendor/upstream` or `distro/nix` to conditions | 1 |
| wrong automation carve-out | Remove or widen `automation/**` unexpectedly | 1 |
| unexpected bypass actor | Add any actor to either ruleset | 1 |
| disabled/evaluate ruleset | Change `enforcement` | 1 |
| classic protection remains | Supply the recon §2.2 object instead of 404/absent | 1 |
| squash/rebase enabled | Toggle either repository setting true | 1 |
| malformed/missing field | Remove `rules`, `parameters`, or `bypass_actors` | 2 for acquisition/schema failure, never pass |
| no `gh` in live mode | PATH without `gh` | 2 |
| unauthenticated live mode | fake `gh` returns auth/API failure | 2 |
| insufficient live authorization | omit `bypass_actors` from an otherwise valid response | 2 |
| endpoint failure | fail each live endpoint in parameterized subtests | 2 and endpoint named |
| fake-gh valid live mode | shim returns the valid aggregate surfaces | 0 |

At least the six acceptance-gate mutations named in WORK_GRAPH are mandatory: missing PR,
wrong approvals, non-merge method, missing non-fast-forward, stale rail, and unexpected bypass
actor. The implementation is not trusted until each planted mutation has been observed red.

## 8. STATE schema v2 and validator semantics

The full coordinator proposal is `STATE.proposed.json`; it preserves all 57 records and all
summaries/evidence/timestamps. Its only content migration is:

```json
{
  "schema_version": 2,
  "last_checkpoint": {
    "node": "...",
    "state": "...",
    "reviewed_commit": null,
    "published_commit": null,
    "updated_at": "...",
    "summary": "..."
  },
  "nodes": {
    "NODE": {
      "state": "accepted",
      "reviewed_commit": "40-hex reviewed SHA",
      "published_commit": "40-hex main-ancestral SHA",
      "evidence": ["..."],
      "summary": "...",
      "updated_at": "..."
    }
  }
}
```

Every record has both keys. Pending/in-progress records use null for both. Dependency-complete
records require both non-null full SHAs. The validator performs:

1. `git cat-file -e <reviewed_commit>^{commit}` for reviewed object existence;
2. `git cat-file -e <published_commit>^{commit}`;
3. explicit published ref resolution;
4. non-shallow repository assertion;
5. `git merge-base --is-ancestor <published_commit> <published-ref>`.

Object existence is never called reachability. The implementation should rename the current
helper accordingly and introduce a separately named ancestry helper.

`check` receives `--published-ref`. CI passes `refs/remotes/origin/main`. Library tests use a
synthetic non-shallow repository or an explicit full ref. The production validator must not
have an `allow_shallow` escape hatch.

Completed checkpoint syntax becomes explicit:

```text
checkpoint NODE --state accepted \
  --reviewed-commit REVIEWED_SHA \
  --published-commit MAIN_ANCESTRAL_SHA \
  --evidence PATH ...
```

The ambiguous `--commit` option is removed rather than guessed. This establishes a two-phase
publication protocol: review the implementation, merge it, then checkpoint it in a subsequent
coordinator change using the reviewed identity and the merge/main identity. A node cannot be
accepted while its only durable identity is a topic commit.

The coordinator must apply `STATE.proposed.json` in the same integration change as the strict
schema-v2 validator. Landing either half alone is invalid. Checkpoint prospective-state
validation retains the existing delta rule so a repair can reduce an old violation without
being blocked by unrelated pre-existing state.

## 9. Equivalence ladder decision

`mapping-ledger.proposed.json` is authoritative. The first successful rung wins; no rung may be
skipped:

1. **Identity:** reviewed commit already is main-ancestral.
2. **Unique stable patch-id:** exactly one non-merge commit in published main history matches.
3. **Complete merge payload:** decompose the reviewed merge's second-parent payload; every
   payload commit maps by identity or unique patch-id. `published_commit` is the earliest main
   commit containing all mapped payload commits.
4. **Per-file tree equality at a named published commit:** derive the complete changed-path set
   from the reviewed commit or payload with `git diff-tree`; every derived path must appear in
   the ledger and be byte-identical at the named historical commit. The reproduction rejects
   either an omitted derived path or an extra hand-selected path. Equality is intentionally
   historical, not equality with current HEAD.
5. **Reopen:** inject a repair node. Similar subject text, approximate diff, object existence,
   or current-tree resemblance is not sufficient.

Distribution for the 35 accepted records:

| Method | Count | Nodes |
|---|---:|---|
| identity | 2 | F29, W3 |
| unique patch-id | 26 | F01-F17, R01, R03, R04, W0, W0.1, W0.2, W0.3, W1, W2 |
| complete merge payload | 3 | F19, F20a, F20b |
| merge payload with one file-tree split | 1 | F18 |
| file-tree at named publication commit | 3 | F20c, F21, F28 |
| reopen | 0 | none, contingent on independent proof reproduction |

The weak-rung decisions are explicit:

- **F18 is accepted.** `git diff-tree` derives four paths from reviewed payload
  `01fcf0bba502...`: `.github/workflows/nix.yml`, `README.md`, the local Darwin build log, and
  the local Darwin version file under `evidence/F18`. All four are byte-identical at published
  `163e6e0d7...`. Its separate `ci-pr-run.md` payload maps by unique patch-id to
  `767afae9c...`; the former is an ancestor of the latter, which is the publication boundary.
- **F20c and F21 are accepted** by exact evidence-file equality at
  `e1d17541eb1207db4d1bc5ccbe05422a2adef68c`.
- **F28 is accepted** by exact `crates/jcode-app-core/src/tool/tests.rs` equality at that same
  publication commit. Later `ac8777d2d` legitimately superseded the file, so comparison to
  current HEAD would be wrong.

An independent reviewer should derive each rung-4 path set, compare it to the ledger, and then
reproduce every byte comparison from Git objects rather than accepting ledger booleans.

## 10. Private archive plan and reachability proof

`archive-manifest.proposed.json` contains 39 new refs:

- 33 `refs/heads/archive/reviewed/<node>` refs, one for every accepted reviewed identity that is
  not main-ancestral, even when an older archive branch already contains it transitively;
- six existing lightweight tags, `refs/tags/archive/stash-0` through `stash-5`, retaining their
  exact names and target objects.

The recovery archive currently reports 45 ordinary `ls-remote` lines: HEAD plus 44 heads and
zero tags. `git ls-remote --refs` excludes HEAD, so its baseline is 44 and, absent unrelated
concurrent additions, its post-write count would be 83. These global counts are informational
only. The gate is exact equality inside `refs/heads/archive/reviewed/*` and
`refs/tags/archive/stash-*`. Existing refs are not moved or deleted.

The preferred write is one atomic push built from the manifest:

```text
git push --atomic "$verified_archive_push_url" \
  <reviewed-sha>:refs/heads/archive/reviewed/<node> ... \
  refs/tags/archive/stash-0:refs/tags/archive/stash-0 ... \
  refs/tags/archive/stash-5:refs/tags/archive/stash-5
```

Before the push, enumerate both fetch URLs and every push URL with `git remote get-url --all`
and `git remote get-url --push --all`. Reject unexpected `pushurl`, URL rewriting, non-GitHub
hosts, or any canonical owner/repository other than
`github.com/jerudnik/jcode-recovery-archive`. Verify that exact repository is private with a
credential that can see it. Use the validated explicit URL variable in the push command rather
than the remote nickname. Recon could not prove privacy because its REST credential received
404. If URL identity or privacy cannot be proved, stop before writing.

After the push:

1. Capture sanitized `git ls-remote --refs "$verified_archive_push_url"` output from the same
   canonical URL used for the write.
2. Compare every manifest ref and full SHA exactly; reject missing, moved, or extra refs in the
   managed namespaces.
3. Fetch the managed refs from that canonical URL into a fresh temporary bare repository.
4. Run `git cat-file -e <sha>^{commit}` for every manifest object.
5. Run `git fsck --full --no-dangling` in that fresh repository.
6. Prove all six stash refs retain their multi-parent commits.
7. Prove no `refs/tags/archive/stash-*` exists on the public fork.

`archive/f20c-retire-distribution` is deliberately not pushed by this plan. R07 names
`archive/stash-*`, not every `archive/*` tag, and F21/F28 are directly preserved by reviewed
refs. Expanding that external-write scope requires coordinator authorization. The tag must not
be moved or deleted locally.

## 11. Parallel implementation streams and integration sequence

Three streams may prepare in parallel, but the barriers are ordered.

### Stream G: governance

Own `required-checks.json`, `fork-health.sh`, governance comparison code, tests, valid fixture,
and evidence transcripts. Hand `workflow-contexts.proposed.patch` to the coordinator/workflow
owner. Do not apply GitHub configuration.

After activation, a pull request that modifies `.github/scripts/**`, `.github/workflows/**`, or an R07 governance file
turns `Governance Root` red, and a red required context blocks the merge. A future legitimate
governance change uses the transaction-bound maintenance procedure in §4: identity capture, one
ruleset PUT that temporarily drops `Governance Root` from required checks (recording the exact
pre-change body and hash), a SHA-conditioned merge, main-tip and two-parent verification, one PUT
restoring the exact pre-change body, a post-restoration tip-equality check plus an executable
exactly-one-merge-commit proof over the whole window, and a post-merge comparison from both a
pre-window and a post-window comparator.
All writes and read-backs are evidence. Per D031 this window is under the owner-admin, who is the
accepted root of trust; it is designed to be auditable and its boundaries provable, not
unavailable.

### Stream S: state

Own schema-v2 validator/checkpoint changes and state tests. Reproduce the ledger. Hand
`STATE.proposed.json` to the coordinator. Do not edit live `STATE.json` directly.

### Stream A: archive

Re-verify every local source object and the private remote identity, prepare atomic refspecs,
and capture pre-write manifest. This stream is time-sensitive because six reviewed commits are
reflog-only.

### Ordered barriers

0. **Authorization barrier:** coordinator resolves workflow ownership, confirms external GitHub
   writes, and proves the archive repository is private.
1. **Archive barrier first:** push and fresh-fetch-verify all 39 refs. Stop if any source object
   or remote proof is missing.
2. **Bootstrap integration PR:** combine R07-owned implementation, the authorized workflow diff,
   and the coordinator-applied schema-v2 state proposal. The validator and state migration land
   together. Record and independently review the exact PR number, source repository/ref/head,
   and target repository/ref/base identity.
3. **Context-emission proof:** on that exact bootstrap PR, confirm all four contexts are emitted
   by integration id 15368, that `Fork CI Gate`, `Security Gate`, and `Nix Gate` are green, and
   that `Governance Root` is red naming the governance paths this PR changes (§4). A green
   `Governance Root` here would mean the audit gate is misconfigured: stop. Nothing is required
   yet, so the red context does not block the merge.
4. **Bootstrap merge:** merge with the API using the expected head SHA and `merge_method=merge`;
   prove the response merge SHA is main's new tip with the reviewed base/head parents. All four
   context definitions now exist on `refs/heads/main`, which is the precondition for requiring
   them. Only now run the `github-governance.proposed.json` sequence, whose own preflight
   re-verifies repository id, integration id, both ruleset ids, and the classic-protection
   baseline before the first write.
5. **Under-enforcement proof PR:** open a harmless PR that changes no governance-root path.
   Confirm `Governance Root`, `Fork CI Gate`, `Security Gate`, and `Nix Gate` appear and pass;
   stale base forces strict reruns; squash/rebase are unavailable; unresolved threads block; and
   merge commit is the only normal merge path. Plant a separate attempted workflow change and
   observe `Governance Root` fail before closing it unmerged.
6. **Final evidence:** merge the harmless proof PR normally, rerun live fork health, record
   sanitized ruleset/PR/archive/state transcripts, and obtain the independent adversarial review.
7. **R07 checkpoint:** in a subsequent coordinator change, checkpoint R07 with the reviewed R07
   identity and the merge/main-ancestral published identity, then rerun schema-v2 validation.
   R07 remains pending until this checkpoint is itself published.

Never require a context before its definition is on `refs/heads/main` and has been observed
emitting; that ordering is what prevents a lockout, and it is the only ordering constraint the
removed transition protocol was actually load-bearing for.
Never land schema v2 without the full state proposal. Never delete classic protection before
ruleset read-back. Never push archive refs to the public fork.

## 12. Acceptance evidence matrix

| R07 gate | Required evidence |
|---|---|
| Intended server enforcement | Sanitized preflight snapshots and hashes for repository (with `id`), both rulesets by id and name, and classic protection; exact PR/source/head/base identity; SHA-conditional merge response with main-tip and two-parent proof; full `protect-fork-rails` and `no-stray-branches` JSON after apply, with empty `bypass_actors`; effective-main rules exactly `deletion`, `non_fast_forward`, `pull_request`, `required_status_checks`; classic-protection 404; and repository merge settings. |
| Every required context present/green | Bootstrap PR check-run list showing all four contexts emitted by integration id 15368, three green and `Governance Root` red on its own governance change; post-enforcement PR check-run list showing all four green and no absent required context. |
| Fixture and live governance | Valid transcript plus every planted failure in §7; live transcript showing complete rules/bypass/enforcement/repository-id comparison; a scheduled `fork-health.yml --live` run observed exiting 0, and one deliberately mutated live-shim run observed exiting non-zero, so the drift detector is proved non-vacuous in both directions. |
| Completed records published or reopened | Schema-v2 validator transcript, full ledger, ancestry output for all 35 published commits, and any injected repair nodes. |
| Reviewed identities and stash tags archived | Pre/post manifest, redacted `ls-remote`, fresh-fetch `cat-file` and `fsck`, and public-fork negative tag proof. |
| Independent review | Reviewer explicitly checks bypass, context lockout, shallow-history behavior, file-tree rung, archive reachability, external-write ordering, and whether the auditability controls detect a governance change made outside the §4 maintenance procedure. |

The accepted residual risk is recorded rather than evidenced: no artifact can demonstrate that
the owner-admin will not rewrite a ruleset, because per D031 the owner-admin is the root of
trust. The evidence above shows the specific, bounded set of things R07 actually proves: the
server state matches the manifest at each live/scheduled comparison instant; the bootstrap
transition and each §4 maintenance transaction are individually provable as exact-head-conditioned,
tip-bound, exactly-one-merge-commit, restored-to-hash windows; and a governance-path change that bypasses that
procedure is flagged the next time a comparison runs. It does not claim, and §1/§3 say so
explicitly, that every historical change is captured in a continuous log or that detection is
instantaneous rather than sample-point.

## 13. Stop conditions and open questions

A result that would disprove this design is named in advance:

- A proposed summary context is absent on any pull request shape. Stop; do not require it.
- `Governance Root` does not fail a planted governance-path change, or fails to name the changed
  paths. Stop; the audit gate is the only thing standing between a governance change and silence,
  so a vacuous one is worse than none.
- Live fork-health comparison passes against a deliberately mutated live surface. Stop; the drift
  detector must be observed red before it is trusted (D029's standing rule).
- The merge API rejects the expected head SHA, the response is not `merged:true`, main does not
  equal the returned merge SHA, or its two parents differ from reviewed base/head. Stop before
  applying the ruleset; the required contexts are not yet on `main`.
- Sequence 6 finds current `main`'s tip is not a descendant of `bootstrap_merge_sha`, or finds a
  non-empty protected-path diff between them. Stop before the ruleset write; re-review current
  `main`'s actual state (§4).
- Sequence 8's immediate repeat of sequence 6 sees a different protected-path state or rewritten
  boundary. Restore and verify the sequence-3 ruleset body, then stop; do not continue activation.
- During a §4 maintenance window, the post-restoration main tip differs from `merge_sha`, the
  complete `git rev-list --first-parent --merges expected_base_sha..post_restore_main_sha` output is not exactly
  `merge_sha`, or the pre-window and post-window `fork-health.sh --live` transcripts (§4 step 8)
  disagree. Stop and treat it as a governance incident, not routine maintenance.
- Any preflight identifier read-back differs from the value this design embeds (repository id,
  integration id, either ruleset id/name binding, or the classic-protection baseline). Stop; the
  apply document is describing a repository state that no longer exists.
- GitHub rejects either exact ruleset body. Stop; reconcile against the current official API
  rather than deleting classic protection or weakening fields.
- Live credentials omit `bypass_actors`. Stop; the live comparison is unauthorized, not green.
- The repository is shallow where ancestry is asserted. Stop; fetch complete history.
- Any published mapping is not ancestor of the chosen main ref, has a non-unique patch-id, has
  incomplete merge payload, or fails byte equality. Stop and reopen that node.
- Any reviewed/archive source object is missing locally. Stop before push and recover it first.
- The archive cannot be proved private. Stop before push.
- Any manifest ref fails exact `ls-remote` or fresh-fetch object verification. Stop and repair
  archive durability before R07 proceeds.

**Not a stop condition, by decision.** v1 stopped if the repository-level `workflows` rule was
unavailable, on the theory that zero-approval governance is then not self-protecting. That rule
*is* unavailable (§0) and D031 resolves the question in the other direction: the owner-admin is
the accepted root of trust, and R07 delivers a self-*checking* rather than self-*protecting*
property. The residual risk is explicit: any ruleset change made by the owner-admin, inside or
outside the §4 maintenance procedure, is outside enforcement and is caught only by audit — live
comparison and the scheduled fork-health run. R07 must not be reported as preventing it. If the
repository ever moves into an organization or enterprise plan, or GitHub makes repository-level
required-workflow rules generally available, D031's reopen triggers apply and this trade-off is
revisited as a new decision.

Open questions for the coordinator are intentionally limited to ownership/authorization:

1. Which authorized owner will land `workflow-contexts.proposed.patch`?
2. Who will apply `STATE.proposed.json` atomically with validator schema v2?
3. Which credential/owner can prove the recovery archive is private and perform the confirmed
   atomic push?
4. Which administrator will confirm and apply the GitHub configuration write set after context
   emission is proved?

No technical acceptance decision is deferred to those questions.
