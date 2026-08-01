# R07 recon: governance, state schema, and archive inventory

Read-only reconnaissance for R07 (`docs/fork/ideal-base/WORK_GRAPH.json`, node
`R07`, parent `W4R`). No repository state was mutated, no GitHub write was
issued, nothing was pushed. Every GitHub read was a `GET` with the admin token
supplied inline per command and never persisted; all captured output below is
sanitized (no tokens, no `_links` beyond public URLs, no actor identities beyond
role IDs).

Baseline: `main` at `498249777c453c1d551aeb01fc45420d8ca0a585`, working tree
clean, this report authored on `automation/r07-recon`.

**One structural warning that shapes every measurement below**: this working
clone is **shallow**. `.git/shallow` pins a single grafted root
`631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`, and `git rev-list --count HEAD` is
601. Ancestry answers here are valid for the 601-commit window; they would be
identical in a full clone for the commits examined (all reviewed commits fall
inside or after the graft), but any R07 implementation that runs ancestry checks
in CI must set `fetch-depth: 0` or the answer degrades silently. See §5.

---

## 1. Local governance surface

### 1.1 `scripts/required-checks.json` — **does not exist**

```
$ ls -la scripts/required-checks.json
ls: cannot access 'scripts/required-checks.json': No such file or directory
```

R07 clause (a) requires this file. It must be created from scratch. There is no
prior art in the repository for a machine-readable required-check manifest;
nothing reads such a file today.

### 1.2 `scripts/fork-health.sh` — exists, 181 lines, five checks

Executable, `set -euo pipefail`. Options: `--repo` (default `jerudnik/jcode`),
`--fork-remote` (default `$FORK_REMOTE` or `github`), `--help`. Reads
`FORK_POINT_REF` (default `fork-point`). Fails hard (exit 2) if
`refs/remotes/$fork_remote/main` or the `fork-point` tag is absent. Accumulates
`failures` and exits 1 at the end if non-zero.

| # | Check | Lines | Mechanism | Degradation |
|---|---|---|---|---|
| 1 | `fork-point` is an ancestor of the rail | 67-76 | `git merge-base --is-ancestor` | hard fail |
| 2 | Rail exists on GitHub; topic branches reported | 78-111 | `gh api repos/$repo/branches --paginate`, filters out `automation/` | **skips with WARN** if `gh` missing/unauthenticated |
| 3 | `docs/BRANCHING.md` names every workflow | 113-128 | `grep -qF` per `.github/workflows/*.yml` | hard fail |
| 4 | Rulesets reference only current rails | 130-152 | `gh api repos/$repo/rulesets` then per-id `.conditions.ref_name` include/exclude, allow-list `main` and `automation/**` | **silently skipped** if `gh` unavailable (no `else` branch); WARNs, does not fail, when *no* rulesets exist |
| 5 | No Windows CI | 154-169 | `grep -rlEi` over `.github/workflows/` | hard fail |

Two properties matter for R07 design:

- **Check 4 only inspects `conditions.ref_name`.** It never looks at
  `rules[]`, `bypass_actors[]`, or `enforcement`. A ruleset that names `main`
  and enforces nothing passes check 4 today. That is exactly the current state
  (§2). This is the "false durability" gap the acceptance gates name.
- **Both GitHub-dependent checks degrade to silence, not failure.** Fine for a
  local convenience script; unacceptable for a check R07 wants to be a required
  context. Any live-mode ruleset comparison R07 adds needs an explicit
  "GitHub unreachable" failure path distinct from "rules are wrong", plus the
  offline fixture mode the gates demand.

Consumed by `.github/workflows/fork-health.yml` (`schedule: 37 9 * * *` +
`workflow_dispatch` only — **not** on `pull_request`), which checks out with
`fetch-depth: 0`, runs `git fetch --prune --tags origin main`, then invokes the
script with `--fork-remote origin`. On failure it opens/updates a drift issue
rather than blocking anything. Nothing about fork-health is a PR gate today.

### 1.3 `tests/test_ideal_base_railway.py` — 191 lines, 9 tests, no fixture directory

Loads `scripts/ideal_base_railway.py` by `importlib.util.spec_from_file_location`
as module `railway`. There is **no fixture directory and no fixture files**:
every test either (a) calls `railway.validate_repository()` against the live
repository, or (b) constructs small in-memory `dict` node graphs inline.

| Test | Style | Planted failure? |
|---|---|---|
| `test_repository_control_plane_is_valid` | live repo | no |
| `test_runnable_projection_offers_only_genuinely_dispatchable_work` | live repo | no |
| `test_bootstrap_prompt_covers_the_full_execution_protocol` | live + `tempfile` | yes (unterminated fence) |
| `test_cycle_is_rejected` | inline dict | yes |
| `test_unserialized_exact_path_overlap_is_rejected` | inline dict | yes |
| `test_unserialized_glob_subsumption_is_rejected` | inline dict | yes |
| `test_completed_state_requires_reachable_commit_and_evidence` | live repo + deep-copied state | yes (`does/not/exist`) |
| `test_root_state_must_not_contradict_its_children` | live repo + deep-copied state | yes (two shapes) |
| `test_atomic_json_write_is_complete` | `tempfile` | no |

The established idiom for a planted-failure case is
`json.loads(json.dumps(state))` to deep-copy live state, mutate it, and assert
`assertRaisesRegex(railway.RailwayError, ...)`. R07's governance fixtures should
follow that shape but will need a **new** on-disk fixture concept, because
ruleset JSON has no in-repository source today. Both files are pure-`unittest`,
no pytest, no third-party deps; `python3 -m unittest tests.test_ideal_base_railway`
currently passes 9/9 in 8.4s.

Neither `scripts/ideal_base_railway.py` nor its test file is referenced by any
workflow, `scripts/preflight.sh`, `scripts/test_fast.sh`, or `flake.nix`
`checks`. **The railway validator does not run in CI at all today.** The only
Python test wired into a Nix check is `tests/test_nix_distribution_policy.py`
(`flake.nix:294`). `scripts/preflight.sh` runs 12 gates (lines 102-112), none of
them the railway.

---

## 2. Live GitHub governance state (sanitized)

### 2.1 Rulesets

Two active repository rulesets. Full sanitized bodies:

```json
{
  "id": 18509013,
  "name": "protect-fork-rails",
  "target": "branch",
  "source_type": "Repository",
  "source": "jerudnik/jcode",
  "enforcement": "active",
  "conditions": { "ref_name": { "include": ["refs/heads/main"], "exclude": [] } },
  "rules": [ { "type": "deletion" } ],
  "created_at": "2026-07-04T10:22:12-04:00",
  "updated_at": "2026-07-27T00:40:10-04:00",
  "bypass_actors": [],
  "current_user_can_bypass": "never"
}
```

```json
{
  "id": 18509016,
  "name": "no-stray-branches",
  "target": "branch",
  "source_type": "Repository",
  "source": "jerudnik/jcode",
  "enforcement": "active",
  "conditions": {
    "ref_name": {
      "include": ["~ALL"],
      "exclude": ["refs/heads/main", "refs/heads/automation/**"]
    }
  },
  "rules": [ { "type": "creation" } ],
  "created_at": "2026-07-04T10:22:23-04:00",
  "updated_at": "2026-07-27T00:39:18-04:00",
  "bypass_actors": [ { "actor_id": 5, "actor_type": "RepositoryRole", "bypass_mode": "always" } ],
  "current_user_can_bypass": "always"
}
```

Effective rules for `main` (`GET /repos/jerudnik/jcode/rules/branches/main`):

```json
[ { "type": "deletion", "ruleset_source_type": "Repository",
    "ruleset_source": "jerudnik/jcode", "ruleset_id": 18509013 } ]
```

**`main` is protected against deletion and nothing else.** Every rule R07 clause
(a) enumerates is absent from the ruleset layer:

| R07 requirement | Ruleset today |
|---|---|
| deletion protection | present (`deletion`) |
| non-fast-forward protection | **absent** |
| changes through pull requests | **absent** |
| zero required approvals | n/a (no `pull_request` rule) |
| review-thread resolution | **absent** |
| merge commits only | **absent** |
| strict required checks | **absent** |
| no silent administrator bypass | `bypass_actors: []` on this ruleset (OK); `no-stray-branches` grants `RepositoryRole 5` (admin) `bypass_mode: always` |

### 2.2 Legacy classic branch protection (still live, and contradictory)

`GET /repos/jerudnik/jcode/branches/main/protection`:

```json
{
  "required_status_checks": { "strict": false, "contexts": ["Detect changes"],
                              "checks": [{"context": "Detect changes", "app_id": null}] },
  "required_signatures": { "enabled": false },
  "enforce_admins": { "enabled": false },
  "required_linear_history": { "enabled": false },
  "allow_force_pushes": { "enabled": false },
  "allow_deletions": { "enabled": false },
  "block_creations": { "enabled": false },
  "required_conversation_resolution": { "enabled": false },
  "lock_branch": { "enabled": false },
  "allow_fork_syncing": { "enabled": false }
}
```

Notable and load-bearing for the design:

- There is **no `required_pull_request_reviews` key at all**, so classic
  protection does not require a pull request either.
- `enforce_admins: false` — administrators bypass everything here silently.
  This is the "silent administrator bypass" the acceptance gate names, and it
  lives in *classic protection*, not in the rulesets. A design that only hardens
  rulesets and leaves this object in place will not close the gate.
- `strict: false` — required checks are not required to be up to date with base.
- Exactly one required context, `"Detect changes"`, `app_id: null` (any app may
  satisfy it).
- Classic protection and rulesets are **two independent layers** that GitHub
  unions. `scripts/fork-health.sh` check 4 reads only the ruleset layer, so this
  entire object is currently invisible to fork-health.

### 2.3 Repository merge settings

```json
{ "allow_merge_commit": true, "allow_squash_merge": true,
  "allow_rebase_merge": true, "allow_auto_merge": false,
  "delete_branch_on_merge": false, "default_branch": "main",
  "private": false, "visibility": "public", "archived": false }
```

"Merge commits only" is not enforced at the repository level either: squash and
rebase are both still allowed. R07 can enforce this via the ruleset
`pull_request` rule's `allowed_merge_methods`, via repository settings, or both;
whichever it picks, `scripts/fork-health.sh` must learn to read that surface.

### 2.4 Evidence that PR #38 merged under this weak regime

```json
{ "number": 38, "state": "closed", "merged": true,
  "base": "main", "head_sha": "0caffa85dd90a54ef1ee6c7a29151616ab86333c",
  "merge_commit_sha": "498249777c453c1d551aeb01fc45420d8ca0a585",
  "merged_by": "jerudnik", "mergeable_state": "unknown" }
```

It did produce a merge commit, and `Detect changes` was green, so it satisfies
the *intent*. Nothing on the server required either.

---

## 3. Check contexts emitted on `pull_request`

Nine workflow files. Enumerated with a YAML parse of each file (jobs, `name:`,
`if:`, `needs:`, matrix). "Context" is the check-run name GitHub reports, which
is the job `name:` when present, with matrix expansion.

### 3.1 Workflows that fire on `pull_request`

| Workflow | `on: pull_request` | Path filter? |
|---|---|---|
| `fork-ci.yml` | `branches: [main]` | **no** — always triggers |
| `security.yml` | `branches: [main]` | **no** — always triggers |
| `docs-impact.yml` | `branches: [main]`, types opened/synchronize/reopened/ready_for_review | **no** |
| `nix.yml` | `branches: [main]` | **YES** — 29-entry `paths:` allow-list |
| `ci.yml`, `fork-health.yml`, `freebsd-smoke.yml`, `nix-update.yml`, `release.yml` | none | n/a |

`ci.yml` is `workflow_dispatch` only. `release.yml` is `workflow_call` only,
invoked by `nix.yml`'s `release-metadata` job. `freebsd-smoke.yml` is
`workflow_dispatch`. `fork-health.yml` and `nix-update.yml` are schedule +
dispatch.

### 3.2 Every context, with its conditionality

| Context | Workflow | Always on PR? | Gate |
|---|---|---|---|
| `Detect changes` | fork-ci | **YES** | none; job has `continue-on-error: true` on both steps and defaults its outputs to `'true'` |
| `Quality Guardrails` | fork-ci | no | `if: rust=='true' \|\| scripts=='true' \|\| event!='pull_request'` |
| `Build & Test (macOS)` | fork-ci | no | `if: rust=='true' \|\| event!='pull_request'` |
| `Linux Tests` | fork-ci | no | `if: rust=='true' \|\| event!='pull_request'` |
| `Latest stable canary (advisory)` | fork-ci | no | `if: event=='schedule' \|\| 'workflow_dispatch'` — **never on PR** |
| `detect dependency changes` | security | **YES** | `if: event != 'schedule'` (true on PR) |
| `secret scan` | security | **YES** | `if: event != 'schedule'` (true on PR) |
| `dependency audit` | security | no | `if: event!='schedule' && (deps=='true' \|\| event!='pull_request')` |
| `full advisory report` | security | no | schedule/dispatch only — **never on PR** |
| `DOX Review Advisory` | docs-impact | **YES** | none |
| `fast validation` | nix | no | **workflow-level `paths:` filter** |
| `select build matrix` | nix | no | same |
| `build (x86_64-linux)` | nix | no | same, plus matrix (`build (aarch64-darwin)` is push/dispatch only) |
| `publish metadata-only release` | nix | no | `if: startsWith(github.ref,'refs/tags/v')` — **never on PR** |
| `Cursor Bugbot`, `Cursor Security Agent: …`, `Cursor Approval Agent: …` | third-party app | observed on both sampled PRs | not repository-controlled |

Only **four** repository-owned contexts are unconditionally emitted for every
pull request: `Detect changes`, `detect dependency changes`, `secret scan`,
`DOX Review Advisory`. Everything with real signal is conditional.

Observed on PR #38 head `0caffa85d` (docs-only change):

```
Quality Guardrails            success
build (x86_64-linux)          success
fast validation               success
select build matrix           success
Detect changes                success
detect dependency changes     success
secret scan                   success
DOX Review Advisory           success
publish metadata-only release skipped
dependency audit              skipped
Linux Tests                   skipped
Build & Test (macOS)          skipped
Latest stable canary          skipped
full advisory report          skipped
Cursor Bugbot                 neutral
Cursor Approval Agent: …      success
Cursor Security Agent: …      neutral
```

Observed on PR #35 head (Rust change): `Linux Tests`, `Build & Test (macOS)`,
`Quality Guardrails` all `success`; a context named **`security gate`** appears
that is absent from PR #38 and does not match any current job `name:` in
`security.yml` — a renamed or since-removed job. Treat historical check names as
unstable; the design must pin names it controls.

Note that `nix.yml` contexts *did* appear on the docs-only PR #38: its `paths:`
list includes `docs/agent-workflows.md`, `docs/BRANCHING.md`, and `.apm/**`. That
is luck, not a guarantee. A PR touching only `docs/fork/**` emits **no** nix
context at all.

`fork-ci.yml` already documents the trap at lines 44-46:

> Branch protection must require "Detect changes". The conditional checks
> "Quality Guardrails", "Build & Test (macOS)", and "Linux Tests" should be
> removed from required checks or made optional because they may be skipped.

**Implication for R07's "a context that can be absent is never required" gate.**
Requiring today's meaningful contexts directly would deadlock docs-only PRs.
The graph contract already anticipates this: *"fix conditional triggers or add
an always-running summary gate first."* The summary-gate pattern (an always-run
job with `if: always()`, `needs:` every conditional job, that fails unless each
dependency is `success` or `skipped`) is the standard resolution and is not
present anywhere in this repository yet.

Also worth flagging: `Detect changes`, the one context required today, has
`continue-on-error: true` on both of its steps and defaults its filter outputs
to `'true'`. It is close to unfailable. It is a routing signal, not a gate.

---

## 4. `STATE.json` schema and the reviewed-commit ancestry audit

### 4.1 Current schema

`docs/fork/ideal-base/STATE.json`, `schema_version: 1`. Top level:

```
schema_version   1
program          "ideal-durable-tui-cli-foundation"
program_state    "railway_ready"
active_graph_id  null
last_checkpoint  { node, state, commit, updated_at, summary }
nodes            { <node_id>: record }
```

57 records. **Every one has exactly the same five keys**, no exceptions:

```
("commit", "evidence", "state", "summary", "updated_at")
```

State distribution: `accepted` 35, `pending` 21, `in_progress` 1 (`W4`).

There is a **single `commit` field**. It carries the reviewed commit. There is
no representation of a published identity, and no field R07 could reuse. The
schema split in clause (b) is purely additive: `schema_version` must go to 2,
and `validate_state` plus `command_checkpoint` (which writes the record via
`record.update({...})` at `scripts/ideal_base_railway.py:659-668`) must both
learn the new shape. `scripts/ideal_base_railway.py:412` hard-rejects any
`schema_version != 1`, so the bump is a coordinated edit.

Note `docs/fork/ideal-base/STATE.json` is in `coordinator_owned_paths`, and R07's
`owned_paths` deliberately excludes it. R07 changes the *validator* and the
*schema contract*; the coordinator writes the file.

### 4.2 Ancestry audit — **33 of 35 accepted nodes cite a non-ancestral commit**

Method: for each accepted record, `git cat-file -e <commit>^{commit}` (the check
the validator performs today) and `git merge-base --is-ancestor <commit> HEAD`
(the check clause (c) demands).

```
object exists (current validator passes):   35 / 35
ancestor of HEAD (clause (c) semantics):     2 / 35
```

Only **F29** (`caa445ed12040565a6463c4c3a96aa652275aa0a`) and **W3**
(`f8b7b39e72d53ca505b8a82cb5f59928c671dfae`) are on `main`. This is not a
shallow-clone artifact: both ancestral commits and the non-ancestral ones are
fully readable here, and the non-ancestral ones resolve to real commits with
distinct authorship dates and subjects that also appear on `main` under
different SHAs. Nearly the entire accepted history was published by
patch-transplant (the 2026-07-26 curated-sync rewrite at `639f63309` and
neighbours), not by fast-forward.

This is precisely the false durability R07 exists to eliminate: `check` reports
`ideal-base railway OK: 7 roots, 50 child nodes, 57 state records, protected
hash intact` while 33 of 35 accepted nodes cite commits that no published branch
contains.

### 4.3 Reviewed-to-published mapping ledger

Equivalence method per node, computed against a patch-id index of all 601
commits reachable from `HEAD` (596 distinct trees, 597 distinct patch-ids).

| Node | Reviewed | Method | Published | Confidence |
|---|---|---|---|---|
| F01 | `a70db370025c5` | patch-id | `dd4b64c39b96e` | strong (unique) |
| F02 | `2b560788231e1` | patch-id | `a78d2d07f3744` | strong |
| F03 | `d8c223d29e35e` | patch-id | `d01545cd5afcf` | strong |
| F04 | `9c4c99897b884` | patch-id | `2dd1557092735` | strong |
| F05 | `9f4d34d11e9c5` | patch-id | `89bc657e562ec` | strong |
| F06 | `84dc0aa2b5989` | patch-id | `1a7808af54e7d` | strong |
| F07 | `58a80640134e7` | patch-id | `95177df94165e` | strong |
| F08 | `003419d9b71e3` | patch-id | `99acd179ba382` | strong |
| F09 | `d5d388028ed56` | patch-id | `cd591156491ae` | strong |
| F10 | `4b66de27c0a2c` | patch-id | `c955d7fd9d17b` | strong |
| F11 | `77582db053e54` | patch-id | `f4b2ddca1c94d` | strong |
| F12 | `a1c9075afa7fa` | patch-id | `3265193d2c811` | strong |
| F13 | `080bfc9d735d1` | patch-id | `569e2a1a0dbb8` | strong |
| F14 | `e47efacdb6b5c` | patch-id | `6144cdb2a7e42` | strong |
| F15 | `bb53da1476b83` | patch-id | `59c77dd85a910` | strong |
| F16 | `8971ed1dbe1ee` | patch-id | `140719594a7d7` | strong |
| F17 | `cdb2ee303fbec` | patch-id | `061bc3adef906` | strong |
| **F18** | `ca5f38bde4260` | **merge commit** | see below | needs a rule |
| **F19** | `a4dd576d46324` | **merge commit** | see below | needs a rule |
| **F20a** | `dc9ded88150f1` | **merge commit** | see below | needs a rule |
| **F20b** | `c01518181958c` | **merge commit** | see below | needs a rule |
| **F20c** | `c754004541f67` | **file-content** | `e1d17541eb120` | needs a rule |
| **F21** | `191b270932211` | **file-content** | `e1d17541eb120` | needs a rule |
| **F28** | `2df5a891ca7a8` | **file-content, superseded** | `e1d17541eb120` | needs a rule |
| F29 | `caa445ed12040` | identity | `caa445ed12040` | published |
| R01 | `e3736e7fbcc4f` | patch-id | `18c7088b94961` | strong |
| R03 | `a0676f781776f` | patch-id | `12abb57a60ef7` | strong |
| R04 | `c716284989519` | patch-id | `54ae87985ac3a` | strong |
| W0 | `b238d7034fdef` | patch-id | `fda646de9107a` | strong |
| W0.1 | `d5d0adaaf1120` | patch-id | `84c5a778ddefa` | strong |
| W0.2 | `fb00ab840df36` | patch-id | `f1cc1c2a047e8` | strong |
| W0.3 | `b238d7034fdef` | patch-id | `fda646de9107a` | strong (shares W0's commit — by design; W0.3 and W0 cite the same SHA in `STATE.json`) |
| W1 | `4df63d04ef645` | patch-id | `3192e77c2dd43` | strong |
| W2 | `61ba2e2656e25` | patch-id | `931e625cc116a` | strong |
| W3 | `f8b7b39e72d53` | identity | `f8b7b39e72d53` | published |

**28 of 35 map cleanly** (26 unique patch-id, 2 identity). **7 need an explicit
equivalence rule the design must state.**

#### The four merge commits (F18, F19, F20a, F20b)

`git show <merge>` produces no diff by default, so patch-id is undefined. The
defensible rule is to decompose the merge into its second-parent payload and
map each payload commit:

```
F18  ca5f38bde  (2 payload commits)
  f26cd4dbec62 -> 767afae9cf4c  PATCH-EQUAL   docs(F18): add CI proof of real nix build …
  01fcf0bba502 -> NO patch-id match          ci(nix): build + launch the real package on PRs (F18)
F19  a4dd576d4  (3 payload commits)  3/3 PATCH-EQUAL
F20a dc9ded881  (2 payload commits)  2/2 PATCH-EQUAL
F20b c01518181  (5 payload commits)  5/5 PATCH-EQUAL
```

**F19, F20a, F20b decompose completely.** F18 has one payload commit whose
patch-id has no match. Investigated: the reviewed `01fcf0bba502` and the
published `163e6e0d7` carry **byte-identical `.github/workflows/nix.yml`**
(`git diff 01fcf0bba502:.github/workflows/nix.yml 163e6e0d7:.github/workflows/nix.yml`
is empty). The patch-ids differ only because the reviewed commit bundled the
`nix.yml` change together with three `docs/fork/ideal-base/evidence/F18/` files,
while the published commit split them out — the evidence files landed under the
same subjects but in a different commit boundary. The change is present on
`main`; the commit boundary moved. F18 is defensible as a per-file content
equivalence but not as a per-commit patch-id equivalence. **The design must
decide whether that is acceptable or whether F18 reopens.**

#### The three file-content cases (F20c, F21, F28)

All three collapse into the single published commit `e1d17541eb1207db4d1bc…`
("F20c: retire the dead distribution surface (#31)", a 2-parent merge touching
120 files, +24542/−7657) — the boundary at which the curated sync landed. Their
subjects do not match it; only their file contents do.

| Node | Reviewed touches | Content at `e1d17541e` | Content at `HEAD` |
|---|---|---|---|
| F20c | `evidence/F20c/README.md` | identical | identical |
| F21 | `evidence/F21/{README.md,two-run-manifest.md,two-run.json}` | all 3 identical | all 3 identical |
| F28 | `crates/jcode-app-core/src/tool/tests.rs` | identical | **differs** |

F28's file was later modified by `ac8777d2d` ("fix(anthropic): restore OAuth
search tool dispatch"), a 7-line delta rewiring the assertion through
`anthropic_map_tool_name_from_oauth`. So F28's reviewed content *was* published
at `e1d17541e` and has since been legitimately superseded. A published-identity
rule stated as "content still matches HEAD" would wrongly reopen F28; stated as
"content matched at the published commit" it maps correctly. **The design must
state which.** Note F28 is also a naming collision hazard: `2df5a891ca7a` is
*also* the only accepted commit contained by no branch and only by the tag
`archive/f20c-retire-distribution`.

#### Recommended equivalence ladder (for the designer, not a decision I made)

1. identity (already an ancestor)
2. unique patch-id match in `HEAD` history
3. merge decomposition: every second-parent payload commit maps by (1) or (2)
4. per-file tree equality at a named published commit
5. otherwise reopen with an injected repair node

Rules 1-3 cover 31/35 unaided; rule 4 covers F18, F20c, F21, F28. Nothing in
the accepted set falls to rule 5 under this ladder, but the ladder itself is the
thing that needs review, since rule 4 is materially weaker than 1-3.

---

## 5. Validator commit semantics and CI depth

`scripts/ideal_base_railway.py`, 750 lines. Constants at 21-42:
`REPO_ROOT`, `GRAPH_PATH`, `STATE_PATH`, `PROTECTED_PROMPT` +
`PROTECTED_PROMPT_SHA256`, `ALLOWED_STATES`, and
`DEPENDENCY_COMPLETE = {"accepted", "authorization_blocked", "superseded"}`.

**The entirety of commit validation** (lines 118-126):

```python
def git_commit_reachable(commit: str) -> bool:
    result = subprocess.run(
        ["git", "cat-file", "-e", f"{commit}^{{commit}}"],
        cwd=REPO_ROOT, stdout=DEVNULL, stderr=DEVNULL, check=False,
    )
    return result.returncode == 0
```

Pure object existence. The function name says "reachable"; it tests presence in
the object database. An object in a dangling reflog entry, in an unreferenced
pack, or on any fetched remote branch satisfies it. That is why all 35 accepted
records pass today while 33 are not on any published branch.

Two call sites:

- `validate_state` (line 432): every `DEPENDENCY_COMPLETE` record must cite a
  `str` commit that is `git_commit_reachable`, plus a non-empty `evidence` list
  whose every path exists.
- `command_checkpoint` (line 638): a completed checkpoint requires a reachable
  `--commit` and at least one existing `--evidence` path.

Clause (c) replaces this with `git merge-base --is-ancestor <commit> <published
ref>`. Three design consequences:

1. **Ancestry needs a published ref, not `HEAD`.** In a PR checkout `HEAD` is a
   merge preview or the topic tip, so "ancestor of HEAD" answers a different
   question than "published on main". The design should name the ref explicitly
   (`refs/remotes/origin/main`, or `origin/main` after a fetch).
2. **Shallow clones make the answer silently wrong, not erroring.**
   `merge-base --is-ancestor` returns 1 (false) at a graft boundary exactly as
   it does for a genuinely unrelated commit. This clone is shallow *right now*
   (`.git/shallow` has one entry, 601 commits deep) and the accepted commits
   date back to 2026-07-14, comfortably inside that window — but this is
   luck. **Any check that runs ancestry must assert non-shallow**
   (`git rev-parse --is-shallow-repository` must print `false`) or the gate
   reports false negatives as authoritative failures.
3. **No workflow that could run the validator uses `fetch-depth: 0`.** Of the
   four occurrences in `.github/workflows/`, all are in workflows that never
   fire on `pull_request` (`release.yml:21`, `fork-health.yml:28`,
   `freebsd-smoke.yml:35`) or that fire but do not run the railway
   (`docs-impact.yml:24`). `fork-ci.yml` and `nix.yml` — the two PR-firing
   workflows with real gates — use bare `actions/checkout@v4/v5` with the
   default depth of 1. **A depth-1 checkout makes ancestry unanswerable for
   every commit.** Wiring the railway into an existing job without also adding
   `fetch-depth: 0` produces a gate that fails 100% of the time.

Also relevant: the validator asserts a `sha256` over
`docs/fork/recovery/ORCHESTRATOR_PROMPT.md` (lines 26-27, 507-511), and
`command_checkpoint` takes an `flock` on `$GIT_DIR/jcode-ideal-base-state.lock`
and validates the prospective state before `atomic_write_json`. It judges
expansion consistency as a *delta* (a checkpoint must not introduce or worsen a
violation, but may repair one) — a deliberate design at lines 675-694 that any
new validation R07 adds should mirror, or it will deadlock repair.

---

## 6. Recovery archive inventory

Remote `recovery-archive` -> `https://github.com/jerudnik/jcode-recovery-archive.git`.

`GET /repos/jerudnik/jcode-recovery-archive` returns **404** with the admin
token used for the `jerudnik/jcode` reads. The repository is reachable over Git
(`git ls-remote` succeeds) but not over the REST API with this credential. So
its visibility, rulesets, and settings **could not be confirmed via API**; only
Git-level facts below are established. Treat "private" as asserted by the task
brief, not verified by me.

### 6.1 `git ls-remote recovery-archive` — 45 refs, **zero tags**

```
152ececccc57c15  HEAD
e02f40c91e3759a  refs/heads/agent/hotpath-stabilization
6fc5623a540e667  refs/heads/agent/marker-hardening
59c6a0ba0923b5c  refs/heads/archive/detached/jcode-up-2026-07-17
4a3f2e19dcb69d2  refs/heads/archive/local/ci-cleanup-drop-windows-cicd-2026-07-27
696dda16a7f3f5f  refs/heads/archive/local/f17-local-dirty-backup-2026-07-27
2ccf43fd79c7802  refs/heads/backup/pre-stabilization-2026-07-14
e601b95b299c5a0  refs/heads/distro/nix
47f848494f51c4d  refs/heads/feat/nix-managed-mode
e0a8de8e8a34c8f  refs/heads/fix/mcp-selfspawn-supervision-hardening
7195b6a313a3ed5  refs/heads/follow-upstream
152ececccc57c15  refs/heads/main
42aa9cc64183741  refs/heads/normalize/integration
18f9fa1b88b39fe  refs/heads/orch/f5-name-resolution
5e802effebed623  refs/heads/orch/failure-scoreboard
9bd1c4fc0ca8f35  refs/heads/orch/w1-control-log
ed88e1bde7a7b24  refs/heads/orch/w3-lifecycle
8a81c60b25b2da9  refs/heads/recovery/2026-07-15
6cc72ef780af5c3  refs/heads/recovery/close-w4-r02-compose-2026-07-16
2ab8135b2fa38c0  refs/heads/recovery/docs-fork-governance-2026-07-16
c53022f4d4135b4  refs/heads/recovery/fix-gate-parser-2026-07-15
a888ba86ac24385  refs/heads/recovery/fix-r01-r03a-identity-20260715
cdb115a9d2efee1  refs/heads/recovery/fix-r02-tier-20260715
566d7930606f96a  refs/heads/recovery/fix-r04-lifecycle-widening-2026-07-16
0f8bd8d9f5556ac  refs/heads/recovery/fix-r04-marker-20260715
7be320f4942522f  refs/heads/recovery/fix-r05b-spawn-reclaim-2026-07-15
f0e77020c209209  refs/heads/recovery/fix-r12-evidence-20260715
63309f670ee27e4  refs/heads/recovery/fix-r12-terminal-evidence-2026-07-15
52aed00e95887f8  refs/heads/recovery/fix-w5-onboarding-consent-2026-07-16
19d90af988a52ad  refs/heads/recovery/fix-w6-r10-acquisition-2026-07-16
57b31756fc435c8  refs/heads/recovery/light-control-20260715
d1388625f4d6dcf  refs/heads/recovery/light-ledgers-20260715
914916719bb1af3  refs/heads/recovery/light-pilot-20260715
6ca1fcf2ec2366c  refs/heads/recovery/orchestrator-s4-20260715
6ca1fcf2ec2366c  refs/heads/recovery/orchestrator-s5-20260715
6ca1fcf2ec2366c  refs/heads/recovery/orchestrator-s6-20260715
d5898df4c03297c  refs/heads/recovery/pilot-prereq-ledgers-20260715
7c858453768dcd7  refs/heads/recovery/seam-r01-20260715
3217dbcbf22ea6e  refs/heads/recovery/seam-r02-20260715
73e4e9e62b33177  refs/heads/recovery/seam-r03a-20260715
557cf7ddbcfedd6  refs/heads/recovery/seam-r04-20260715
9385e9a46afe598  refs/heads/recovery/seam-r05b-20260715
3831f4afbc1bdb7  refs/heads/recovery/seam-r12-20260715
0ad2278ab913eb1  refs/heads/sync/upstream-v0.46
631935dd1d3b2e3  refs/heads/vendor/upstream
```

44 heads + HEAD. **No `refs/tags/*` whatsoever.** The `orchestrator-s4/s5/s6`
refs all point at the same commit.

### 6.2 `archive/stash-*` tags: local only, **nowhere on any remote**

Seven `archive/*` tags exist in the local clone. All are lightweight (tag object
SHA == commit SHA). None is present on `github`/`origin` (correct — they must
never be pushed to the public fork) and **none is present on `recovery-archive`
either** (the durability gap clause (d) exists to close).

| Tag | Commit | On `github`? | On archive? | Ancestor of main? | Subject |
|---|---|---|---|---|---|
| `archive/f20c-retire-distribution` | `99857a8d105d` | no | **no** | no | docs(f28): checkpoint F28 as accepted |
| `archive/stash-0` | `c88c7a26e2a1` | no | **no** | no | On ci-validation: f17-local-variant-full-1784783446 |
| `archive/stash-1` | `dbe9ba2404df` | no | **no** | no | On main: ideal-base F02 aborted partial impl after 429 … |
| `archive/stash-2` | `1f54abc9fbb0` | no | **no** | no | On main: WIP fix-config-hotpath-spam part 3 … |
| `archive/stash-3` | `975b91b83361` | no | **no** | no | On main: WIP fix-config-hotpath-spam part 2 … |
| `archive/stash-4` | `5dc53ed77b98` | no | **no** | no | On main: WIP fix-config-hotpath-spam … |
| `archive/stash-5` | `29d49b250a6a` | no | **no** | no | On main: wip before upstream sync |

`archive/stash-0` through `-5` are converted stashes; their commits are
multi-parent stash commits whose content exists nowhere else. `fork-point` and
the `v*` release tags *are* on `github`; the `presync/*` and `tb21-*` and
`tui-refactor-base-*` tags were not audited (out of scope; only `archive/stash-*`
is named by clause (d)).

### 6.3 Which reviewed commits the archive already covers

Containment computed against `refs/remotes/recovery-archive/*` (already fetched
locally, so this reflects the remote's actual ref graph).

```
accepted nodes:                                        35
already published (ancestor of main):                   2   F29, W3
non-ancestral, contained by an archive branch:         25
non-ancestral, contained by NO archive ref:             8
```

The 25 covered nodes are all reachable from **both**
`archive/local/f17-local-dirty-backup-2026-07-27` (`696dda16a`) and
`archive/local/ci-cleanup-drop-windows-cicd-2026-07-27` (`4a3f2e19d`). Those two
branches are the load-bearing preservation refs for F01-F16, R01, R03, R04, W0,
W0.1, W0.2, W0.3, W1, W2.

**The eight uncovered reviewed commits:**

| Node | Reviewed commit | Held by | Risk |
|---|---|---|---|
| F17 | `cdb2ee303fbe` | reflog only | **at risk** |
| F18 | `ca5f38bde426` | reflog only | **at risk** |
| F19 | `a4dd576d4632` | reflog only | **at risk** |
| F20a | `dc9ded88150f` | reflog only | **at risk** |
| F20b | `c01518181958` | reflog only | **at risk** |
| F20c | `c754004541f6` | reflog only | **at risk** |
| F21 | `191b27093221` | local tag `archive/f20c-retire-distribution` | tag is unpushed |
| F28 | `2df5a891ca7a` | local tag `archive/f20c-retire-distribution` | tag is unpushed |

Six of these are held **only by this clone's reflog**. `gc.reflogExpire`
defaults to 90 days and `gc.reflogExpireUnreachable` to 30 days; neither is
configured here. `git gc --prune` after that window deletes them permanently.
The reviewed identities for F17-F20c are, today, one `git gc` away from being
irrecoverable. This is the single most time-sensitive finding in this report.

Also uncovered: `01fcf0bba502`, the F18 second-parent payload commit whose
patch-id has no match on `main` (§4.3), is contained by no ref at all.

### 6.4 Archive branch `main` diverges

`recovery-archive/main` is at `152ececccc57c15`, unrelated to the fork's current
`main` (`498249777`). Not a problem, just a note: do not assume the archive's
`main` tracks anything.

---

## 7. Consolidated gap list against R07's acceptance gates

| Gate | Status now | What's missing |
|---|---|---|
| Ruleset + PR show intended enforcement | **fails** | Only `deletion` is enforced; classic protection has `enforce_admins:false`, `strict:false`, one weak required context, no PR requirement. Squash/rebase both allowed at repo level. |
| Every required context present and green | **fails** | Only 4 repo-owned contexts always fire on PR, and the only required one (`Detect changes`) is near-unfailable. No summary gate exists. |
| Governance fixtures + live read-only mode | **absent** | `scripts/required-checks.json` does not exist. `fork-health.sh` check 4 reads only `conditions.ref_name`, never `rules[]`/`bypass_actors[]`/`enforcement`, and skips silently without `gh`. No fixture directory in `tests/`. |
| Every completed record has a proved published identity | **fails** | 33/35 accepted records cite non-ancestral commits. 28 map strongly; F18/F20c/F21/F28 need an explicit weaker rule or reopening. Schema has one `commit` field and `schema_version` is pinned to 1. |
| Reviewed identities + stash tags have archive refs | **fails** | Zero tags on `recovery-archive`; all seven `archive/*` tags are local-only. 8 reviewed commits uncovered, 6 of them reflog-only. |
| Independent reviewer finds no bypass/lockout/false-durability | **n/a** | Depends on the above. The lockout risk to watch is requiring a conditional context, or wiring ancestry into a depth-1 checkout. |

---

## 8. Method, and what I did not check

Commands used, all read-only: `git ls-remote`, `git for-each-ref --contains`,
`git merge-base --is-ancestor`, `git cat-file -e`, `git rev-list --parents`,
`git patch-id --stable` over `git log -p --no-merges HEAD`, `git diff` between
blob paths, `git reflog --all`, `git config --get`, and `gh api` GETs against
`repos/jerudnik/jcode{,/rulesets,/rulesets/:id,/rules/branches/main,/branches/main/protection,/pulls/:n,/commits/:sha/check-runs}`
plus a 404ing GET on `repos/jerudnik/jcode-recovery-archive`. Workflow structure
was extracted with a PyYAML parse (via `nix shell`) rather than by grep.

Not checked:

- **Whether the archive repository is actually private, and its ruleset/settings.**
  The REST API returns 404 for it with this token. Only Git-level facts are
  established.
- **Whether any `github`-side ruleset applies at the *organization* level.**
  Checked and negative: `GET repos/jerudnik/jcode/rulesets?include_parents=true`
  returns only the two `source_type: Repository` rulesets above. No parent
  ruleset exists. (`jerudnik` is a user account, so this is expected.)
- **`git fsck --unreachable` for the six reflog-only commits.** The scan
  exceeded a two-minute budget and was abandoned; reflog containment was
  confirmed instead by direct grep. So I have not proven they are *unreachable*
  objects, only that no ref contains them and the reflog does.
- **Whether the patch-id-strong mappings are also tree-equal.** Patch-id
  matching was unique (1 candidate) in all 26 cases, which is strong, but I did
  not additionally verify tree equality or re-derive each pairing by a second
  independent method.
- **Historical check-run names beyond PRs #35 and #38.** I sampled two PRs. The
  `security gate` context on #35 shows names have churned; I did not enumerate
  the full historical set.
- **Whether required contexts can be set via the ruleset layer while classic
  protection remains.** GitHub unions the two layers; I did not test the
  interaction empirically (that would require a write).
- **`presync/*`, `tb21-*`, `tui-refactor-base-*` tags.** Out of scope for
  clause (d), which names `archive/stash-*` specifically.
- **Anything about the 21 pending / 1 in-progress nodes.** Only the 35 accepted
  records were audited.
