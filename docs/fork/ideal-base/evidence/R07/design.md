# R07 design: reviewed publication and self-checking governance

Design baseline: `main` at `498249777c453c1d551aeb01fc45420d8ca0a585`.
Primary evidence: `automation/r07-recon:docs/fork/ideal-base/evidence/R07/recon.md`.
This document makes no GitHub write, pushes no ref, and does not edit coordinator-owned
`docs/fork/ideal-base/STATE.json` or any workflow. Exact coordinator/out-of-scope proposals
are recorded beside this design.

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
| `docs/fork/ideal-base/evidence/R07/governance-transition.workflow.template.yml` | Candidate-code-free required workflow that authorizes one reviewed PR number, source repository/ref, head SHA, target repository/ref, and base SHA and rejects every other PR during bootstrap or maintenance. |
| `docs/fork/ideal-base/evidence/R07/governance-transition.proposed.template.json` | Exact materialization, immutable-tag ruleset, pin, read-back, restoration, and rollback contract for a governance transition. |
| `docs/fork/ideal-base/evidence/R07/workflow-contexts.proposed.patch` | Exact diff for workflow-owned paths outside R07. Coordinator must fold it into an authorized owner or amend the graph. |
| `docs/fork/ideal-base/STATE.json` and `docs/fork/ideal-base/DECISIONS.md` | Coordinator-owned. R07 must not edit them directly. |

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
  "required_workflows": [
    {
      "path": ".github/workflows/governance-root.yml",
      "repository_id": 1238606714,
      "ref": "refs/heads/main"
    }
  ],
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

The validator must also inspect every `.github/workflows/*.yml` file with a constrained,
fail-closed workflow extractor. It requires exactly one literal job `name:` definition for each
required context, at the declared file/job id, with the exact `needs`, `if`, routing outputs,
and pull-request trigger contract above. Duplicate names, dynamic names, YAML constructs the
extractor cannot classify, route drift, or a workflow-level pull-request path filter fail. This
detects the same-GitHub-Actions-app spoofing gap that `integration_id` alone cannot close.

Detection inside candidate code is not itself a trust root. The server-side `workflows` rule in
§4 therefore requires `.github/workflows/governance-root.yml` from `refs/heads/main` in
repository id `1238606714`. The trusted-main workflow executes no candidate script. It fails any
PR that changes `.github/workflows/**`, the governance manifest/comparator, the railway
validator, or its tests. A candidate cannot rewrite the summary jobs, their validator, and the
trusted workflow in one self-approving PR. This external ruleset rule is mandatory while the PR
approval count remains zero.

The integration ID is not guessed. Recon's PR #38 check runs report app id `15368` and slug
`github-actions` for `Detect changes`, `secret scan`, `fast validation`, and
`DOX Review Advisory`. The integration pin excludes another app; the unique workflow contract
above separately excludes a second repository workflow from emitting the same name.

## 4. Exact server-side target and diff from recon §2

The authoritative apply document is `github-governance.proposed.json`. The desired ruleset
bodies are:

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
    },
    {
      "type": "workflows",
      "parameters": {
        "do_not_enforce_on_create": false,
        "workflows": [
          {
            "path": ".github/workflows/governance-root.yml",
            "repository_id": 1238606714,
            "ref": "refs/heads/main"
          }
        ]
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

Diff from the live state recorded by recon:

| Surface | Current | Target |
|---|---|---|
| `protect-fork-rails.rules` | `deletion` only | Add `non_fast_forward`, complete `pull_request`, strict `required_status_checks`, and the trusted-main `workflows` rule. |
| `protect-fork-rails.bypass_actors` | `[]` | Remains exactly `[]`. |
| `no-stray-branches.bypass_actors` | repository role 5, `always` | `[]`; stale-rail creation has no administrator escape hatch. |
| Classic protection | `strict:false`, `enforce_admins:false`, weak `Detect changes`, no PR rule | Delete only after the active ruleset is read back and verified. A contradictory second layer must not remain. |
| Repository merge methods | merge, squash, rebase | merge only. |

The write order is executable in schema-v2 `github-governance.proposed.json`: every write is
followed by a named `read_assert`, and an explicit checkpoint forbids the classic-protection
DELETE until the new ruleset, effective main rules, no-stray ruleset, and repository merge
methods all match. If any read-back differs, stop before the next write. These are external
writes and require explicit confirmation immediately before execution. If GitHub rejects the
repository-level `workflows` rule or cannot prove that it selects the workflow from main rather
than the candidate, R07 is blocked. Required contexts plus zero approvals have no independent
trust root without it.

### Immutable transition protocol

`github-governance.proposed.json` is the steady state. Bootstrap and every later governance
change must first materialize `governance-transition.proposed.template.json` from an externally
reviewed PR identity: PR number, head repository/ref/SHA, and base repository/ref/SHA. The paired
workflow contains no checkout and runs no candidate code. It succeeds only when every event field
equals its reviewed literal, so once it is the required workflow every other PR is server-blocked.
This is the concurrency freeze; it does not depend on an actor bypass.

The exact transition is:

1. Record the PR number, both repository ids and refs, and `reviewed_head_sha` plus
   `reviewed_base_sha`. Complete independent review of that exact identity, substitute all seven
   reviewed fields into `governance-transition.workflow.template.yml`, reject every remaining
   `__[A-Z0-9_]+__` token, and run `actionlint`.
2. Create an otherwise empty Git commit whose sole tree entry is the materialized file at
   `.github/workflows/governance-root.yml`. Name its lightweight tag
   `governance-root-transition-<reviewed_head_sha[0:12]>-<random-16-hex>`. Record the transition
   commit SHA, tag, and materialized workflow SHA-256 in the evidence transcript.

   The materialization/commit CLI is deterministic with respect to reviewed inputs and never
   changes the working tree:

   ```bash
   template=docs/fork/ideal-base/evidence/R07/governance-transition.workflow.template.yml
   materialized=$(mktemp)
   transition_index=$(mktemp)
   rm -f "$transition_index"
   trap 'rm -f "$materialized" "$transition_index"' EXIT

   python3 - "$template" "$materialized" \
     "$pull_request_number" \
     "$reviewed_head_sha" "$reviewed_head_repository_id" "$reviewed_head_ref" \
     "$reviewed_base_sha" "$reviewed_base_repository_id" "$reviewed_base_ref" <<'PY'
   import pathlib, re, sys
   source, target, pr, head, head_repo, head_ref, base, base_repo, base_ref = sys.argv[1:]
   assert re.fullmatch(r"[1-9][0-9]*", pr)
   assert re.fullmatch(r"[0-9a-f]{40}", head)
   assert re.fullmatch(r"[1-9][0-9]*", head_repo)
   assert re.fullmatch(r"[A-Za-z0-9._/-]+", head_ref)
   assert re.fullmatch(r"[0-9a-f]{40}", base)
   assert base_repo == "1238606714"
   assert base_ref == "main"
   text = pathlib.Path(source).read_text()
   replacements = {
       "__REVIEWED_PR_NUMBER__": pr,
       "__REVIEWED_HEAD_SHA__": head,
       "__REVIEWED_HEAD_REPOSITORY_ID__": head_repo,
       "__REVIEWED_HEAD_REF__": head_ref,
       "__REVIEWED_BASE_SHA__": base,
       "__REVIEWED_BASE_REPOSITORY_ID__": base_repo,
       "__REVIEWED_BASE_REF__": base_ref,
   }
   for token, value in replacements.items():
       text = text.replace(token, value)
   assert not re.search(r"__[A-Z0-9_]+__", text)
   pathlib.Path(target).write_text(text)
   PY
   actionlint "$materialized"
   workflow_sha256=$(shasum -a 256 "$materialized" | awk '{print $1}')
   workflow_blob=$(git hash-object -w "$materialized")
   GIT_INDEX_FILE="$transition_index" git read-tree --empty
   GIT_INDEX_FILE="$transition_index" git update-index --add \
     --cacheinfo "100644,$workflow_blob,.github/workflows/governance-root.yml"
   transition_tree=$(GIT_INDEX_FILE="$transition_index" git write-tree)
   transition_commit_sha=$(printf 'Governance transition for PR %s, %s on %s\n' \
     "$pull_request_number" "$reviewed_head_sha" "$reviewed_base_sha" | env \
     GIT_AUTHOR_NAME='R07 Governance Transition' \
     GIT_AUTHOR_EMAIL='noreply@invalid' \
     GIT_AUTHOR_DATE='2000-01-01T00:00:00Z' \
     GIT_COMMITTER_NAME='R07 Governance Transition' \
     GIT_COMMITTER_EMAIL='noreply@invalid' \
     GIT_COMMITTER_DATE='2000-01-01T00:00:00Z' \
     git commit-tree "$transition_tree")
   transition_nonce=$(python3 -c 'import secrets; print(secrets.token_hex(8))')
   transition_tag="governance-root-transition-${reviewed_head_sha:0:12}-${transition_nonce}"
   git tag "$transition_tag" "$transition_commit_sha"
   ```
3. On bootstrap, first GET and normalize classic protection, both existing rulesets, and merge
   settings. They must equal the recon baseline summarized above; persist the sanitized
   snapshot/hash and stop for redesign if recon drifted. Then, with explicit external-write
   confirmation, POST the exact
   tag ruleset body. Read it back active with target `tag`, exact ref include, no bypass actors,
   and `deletion` plus parameterized `update` rules. The ruleset deliberately permits initial
   creation but forbids changing or deleting the tag afterward.
4. Only after that read-back, push the tag to the canonical verified `jerudnik/jcode` URL. Verify
   with `git ls-remote --refs "$verified_repository_url" "refs/tags/$transition_tag"` that it
   names exactly the transition commit. Fetch it into a fresh repository and compare the
   workflow bytes and SHA-256 to the reviewed materialization. If the exact remote tag was
   pre-created or races the push, never reference or mutate it; retain the inert protection rule,
   generate a new nonce, and restart review/materialization with a new tag.
5. Materialize the main-ruleset write set by replacing only the required-workflow ref
   `refs/heads/main` with `refs/tags/$transition_tag` and adding the exact transition workflow
   `sha`. GitHub's current ruleset schema explicitly permits a workflow `ref` from a branch or
   tag and a workflow commit `sha`. For bootstrap, apply the full checkpointed write set. For
   later maintenance, update only `protect-fork-rails`. Read back the complete effective rule and
   prove repository id `1238606714`, exact workflow path, exact immutable tag ref, exact workflow
   SHA, and empty bypass actors. At this point only the fully bound reviewed PR can pass
   `Governance Root`; all other PR merges are frozen.
6. Re-read the complete PR identity and all four check runs immediately before merge. Any mismatch
   stops and requires a new review and immutable tag. Merge only with
   `PUT /repos/jerudnik/jcode/pulls/$pull_request_number/merge` and body
   `{"sha":"$reviewed_head_sha","merge_method":"merge"}`; GitHub rejects a raced head SHA.
   Require response `merged:true`, capture `merge_commit_sha`, then read back the PR as merged and
   the main ref exactly at that SHA. In a fresh fetch prove merge parent 1 equals
   `reviewed_base_sha`, parent 2 equals `reviewed_head_sha`, and main's governance workflow bytes
   equal the reviewed head's workflow bytes.
7. Only after every merge postcondition passes, restore the required-workflow ref to
   `refs/heads/main`, remove the transition-only `sha` field, and read the entire steady-state
   workflow object back exactly, including proof that `sha` is absent. Retain the transition tag
   and its active protection ruleset as evidence; do not create a mutable cleanup window.

Recovery is fail-closed. Before a maintenance merge, restore the old `refs/heads/main` workflow
ref and read it back. If bootstrap is abandoned after the transition rule is active, do **not**
restore weak pre-R07 protection. Leave the full ruleset pinned so every PR remains blocked, then
independently review a replacement bootstrap PR, create/protect a new transition tag, and replace
only the old required-workflow ref/SHA with the new exact ref/SHA in one ruleset PUT and read-back.
If the reviewed PR has merged but restoration to main fails, do not relax or remove the transition rule: its
already-consumed PR identity blocks every new PR while the administrator retries restoration. If
GitHub rejects an immutable tag ref, the tag `update` rule, the SHA-conditional merge, or any exact
read-back, stop. There is no interval in which a candidate-selected workflow can authorize an
unreviewed merge. The materialized evidence record binds PR identity, workflow hash, tag/commit,
ruleset ids/read-back hashes, merge response/main SHA, and the restored object's absent `sha`.

## 5. Required context emission and CI depth

Four contexts are required. Each is created on every pull request:

| Context | Summary semantics |
|---|---|
| `Governance Root` | Trusted-main required workflow. Fails any change to workflow or R07 governance-root paths; candidate code cannot redefine it. |
| `Fork CI Gate` | Requires `Detect changes` and `Governance Contract Gate` success. Quality must succeed iff `rust || scripts`; macOS/Linux must succeed iff `rust`. An applicable skip or inapplicable run fails. |
| `Security Gate` | Requires dependency detection and secret scan success. Dependency audit must succeed iff `deps`; an unexplained skip fails. |
| `Nix Gate` | Requires fast validation, matrix selection, and the real x86_64-linux package build/smoke success. |

`Detect changes` is deliberately not required because it is routing, uses `continue-on-error`,
and defaults outputs to true. Conditional jobs are not required directly. Third-party Cursor
contexts are not required because the repository does not control their names or emission.

`workflow-contexts.proposed.patch` makes five exact out-of-scope changes:

1. Adds `governance-root.yml`, whose main-branch version is selected by the server-side required
   workflow rule and refuses candidate changes to every workflow and R07 governance-root file.
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

The comparison must cover:

- exact ruleset names and active enforcement;
- exact include/exclude ref conditions;
- exact rule types and all relevant parameters;
- exact required-workflow repository id, path, and trusted main ref;
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
| missing trusted workflow | Remove the `workflows` rule | 1 |
| candidate-controlled workflow | Change required workflow ref away from `refs/heads/main` | 1 |
| wrong workflow repository/path | Change repository id or path | 1 |
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

After activation, normal PRs may not modify `.github/workflows/**` or the R07 governance-root
files. A future legitimate governance change must use the immutable transition protocol in §4.
The tag-pinned workflow authorizes exactly one externally reviewed PR/source/head/base identity, blocks all
other PRs during the transition, and fails closed if restoration is interrupted. There is no
actor bypass, candidate-selected context, or silent in-PR escape hatch.

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
3. **Immutable bootstrap root:** materialize the transition workflow, protect its not-yet-created
   tag against update/deletion, create and fresh-fetch-verify the tag, then apply the checkpointed
   GitHub write set with that immutable tag as the required-workflow ref. Prove all other PRs are
   frozen and the reviewed PR alone emits all four green contexts.
4. **Bootstrap merge and restoration:** re-read the complete PR identity and four check runs, use
   the merge API with the expected head SHA and `merge_method=merge`, prove the response merge SHA
   is main's new tip with reviewed base/head parents, then switch the required-workflow ref to
   `refs/heads/main` and remove the transition SHA. Read back the exact workflow object with no
   `sha`, empty bypass actors, effective rules, and absent classic protection. If restoration
   fails, leave the immutable transition rule active and all new PRs blocked while retrying;
   never disable it as a workaround.
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

Never enable candidate-owned required contexts before their definitions have emitted
successfully; the separately trusted transition workflow is installed first to serialize the
bootstrap merge.
Never land schema v2 without the full state proposal. Never delete classic protection before
ruleset read-back. Never push archive refs to the public fork.

## 12. Acceptance evidence matrix

| R07 gate | Required evidence |
|---|---|
| Intended server enforcement | Materialized transition evidence record; sanitized transition-tag ruleset; immutable tag fetch/hash proof; exact PR/source/head/base identity; SHA-conditional merge response; main-tip and two-parent proof; full main ruleset JSON before/after restoration with transition `sha` absent; effective-main rules; classic-protection 404; and repository merge settings. |
| Every required context present/green | Bootstrap and post-enforcement PR check-run lists showing all four GitHub Actions contexts and no absent required context. |
| Fixture and live governance | Valid transcript plus every planted failure in §7; live transcript showing complete rules/bypass/enforcement comparison. |
| Completed records published or reopened | Schema-v2 validator transcript, full ledger, ancestry output for all 35 published commits, and any injected repair nodes. |
| Reviewed identities and stash tags archived | Pre/post manifest, redacted `ls-remote`, fresh-fetch `cat-file` and `fsck`, and public-fork negative tag proof. |
| Independent review | Reviewer explicitly checks bypass, context lockout, shallow-history behavior, file-tree rung, archive reachability, and external-write ordering. |

## 13. Stop conditions and open questions

A result that would disprove this design is named in advance:

- A proposed summary context is absent on any pull request shape. Stop; do not require it.
- The required-workflow rule is unavailable, candidate-controlled, or does not fail a planted
  workflow change. Stop; zero-approval governance is not self-protecting.
- The transition tag can be updated/deleted, its fetched workflow differs, another PR passes
  during the pin, or restoration does not read back main exactly. Stop and remain fail-closed.
- The merge API rejects the expected head SHA, the response is not `merged:true`, main does not
  equal the returned merge SHA, or its two parents differ from reviewed base/head. Stop without
  restoring the main workflow ref; the immutable transition remains the merge lock.
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

Open questions for the coordinator are intentionally limited to ownership/authorization:

1. Which authorized owner will land `workflow-contexts.proposed.patch`?
2. Who will apply `STATE.proposed.json` atomically with validator schema v2?
3. Which credential/owner can prove the recovery archive is private and perform the confirmed
   atomic push?
4. Which administrator will confirm and apply the GitHub configuration write set after context
   emission is proved?

No technical acceptance decision is deferred to those questions.
