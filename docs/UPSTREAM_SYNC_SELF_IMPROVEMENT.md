# Upstream Sync Self-Improvement Loop

Shared coordination document for subagent-based improvement of upstream integration while preserving fork customizations.

## Goals

Implement or reject, with evidence, these eight improvement directions:

1. Define bespoke customization boundaries.
2. Add a repeatable upstream-sync script.
3. Prefer Git rerere for repeated conflict resolution.
4. Track fork-only patches explicitly.
5. Use diff-focused review after merge.
6. Split upstream sync from local fixes.
7. Add validation filters for known conflict areas.
8. Consider a fork-custom feature/config flag.

## Loop protocol

Each subagent appends findings here via normal file edits only when assigned. Keep sections concise and auditable.

### Step 1: Repo State

Findings from 2026-05-28 UTC:
- Current branch is `dev`, tracking `origin/dev` at `fbff0919 test: update model picker loading expectations`; `git status` shows only `?? docs/UPSTREAM_SYNC_SELF_IMPROVEMENT.md`.
- Remotes are configured as `origin` = `https://github.com/jerudnik/jcode.git` for fetch/push and `upstream` = `https://github.com/1jehuang/jcode.git` for fetch with push disabled.
- Recent history includes upstream/candidate-master merge commits and local dev commits; no cleanup was performed.
- The only untracked file is this shared loop document, which is expected loop state. No other modified or untracked files were found, so there is no safe cleanup recommendation beyond preserving this file.
- `rerere.enabled` is currently `true` from `/Users/jrudnik/.config/git/config`; no `rerere.autoupdate` value is set. Config was inspected only.
- Existing upstream-sync automation: `.github/workflows/sync-dev-upstream.yml` validates an upstream/master merge into candidate master, pushes master after build, rebases dev, validates, then force-pushes dev with lease.
- Existing validation scripts include `scripts/dev_cargo.sh`, `scripts/remote_build.sh`, `scripts/security_preflight.sh`, budget/check scripts, auth/e2e smoke scripts, and other test helpers; no dedicated local upstream-sync script was found under `scripts/`.
- Existing docs mention upstream/fork concepts only generally; no dedicated fork customization boundary or fork-only patch tracking doc was found in the targeted search.

### Step 2: Open Tasks and Directions

Findings from 2026-05-28 UTC:
- Backlog search/list found no direct open task for upstream-sync automation or fork-customization preservation. Related open work: TASK-17 (CI guardrail cleanup), TASK-18 (repo hygiene/portable-data boundaries), TASK-19/TASK-25 (validation strategy/CI checks), TASK-20 (integration boundary), and TASK-23/TASK-26 (portable/runtime boundary design).
- Existing workflow: `.github/workflows/sync-dev-upstream.yml` already validates `upstream/master` into candidate `master`, builds, pushes `master`, rebases `dev`, builds, then force-pushes with lease.
- Existing long-term direction: `docs/AGENT_NATIVE_VCS_CORE_BEHAVIOR.md` describes per-customization maintenance packets; `docs/MODULAR_ARCHITECTURE_RFC.md` and `docs/COMPILE_PERFORMANCE_PLAN.md` place future customization records/migration logic under `jcode-selfdev`.
- Gap: no local `scripts/` upstream-sync command, no concise boundary doc for fork customizations, and no explicit fork-only patch manifest/maintenance-packet format.

Chained improvement subtasks:
1. **Map customization boundaries**. Depends on: none. AC: list fork-owned files/features, upstream-compatible files, and risky overlap zones; link each boundary to validation commands.
2. **Define patch/customization manifest format**. Depends on: 1. AC: minimal record includes why, attached upstream area, behavior contract, tests, and drop/adapt criteria. Use this before implementing any storage/tooling.
3. **Add local upstream-sync runbook/script wrapper**. Depends on: 1. AC: dry-run-safe workflow documents fetch, candidate merge, build validation, dev rebase, diff review, and final push commands without bypassing existing CI.
4. **Document rerere policy**. Depends on: 3. AC: state `rerere.enabled` recommendation, when not to trust recorded resolutions, and how to inspect/forget bad resolutions. Low implementation cost, likely worthwhile.
5. **Add diff-focused post-merge review checklist**. Depends on: 1 and 3. AC: checklist covers fork boundary files, generated/build files, provider/auth/session paths, and workflow files before pushing.
6. **Separate sync commits from local fixes**. Depends on: 3 and 5. AC: runbook requires pure upstream merge/rebase commits plus separate follow-up fix commits; rejects mixed conflict-fix/product-change commits.
7. **Create validation filter matrix for known conflict areas**. Depends on: 1 and 5. AC: map paths to existing commands such as `scripts/dev_cargo.sh`, `cargo build --profile selfdev -p jcode --bin jcode`, security/budget scripts, and targeted auth/e2e smoke tests.
8. **Evaluate fork-custom feature/config flag**. Depends on: 1 and 2. AC: decision record says whether the customization is runtime behavior, build-time packaging, or docs/process only. Marked **defer by default** because a feature flag may add product complexity before boundaries and patch records prove a real runtime need.

Suggested execution order: 1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7, with 8 deferred until at least one concrete customization cannot be preserved cleanly by documentation, patch records, or script validation.

### Step 3: Research Findings

Findings from 2026-05-28 UTC:

Sources consulted: Git rerere book/docs (`https://git-scm.com/book/en/v2/Git-Tools-Rerere`, `https://git-scm.com/docs/git-rerere`), Git rebasing book (`https://git-scm.com/book/en/v2/Git-Branching-Rebasing`), GitHub Community fork-sync discussion (`https://github.com/orgs/community/discussions/153608`), Atlassian merge-vs-rebase overview (`https://www.atlassian.com/git/tutorials/merging-vs-rebasing`), existing `.github/workflows/sync-dev-upstream.yml`, `scripts/dev_cargo.sh`, `docs/AGENT_NATIVE_VCS_CORE_BEHAVIOR.md`, `docs/MODULAR_ARCHITECTURE_RFC.md`. The Exa-backed `codesearch` tool was attempted for code-intelligence research but returned an MCP tool-not-found error, so local repo search/read plus websearch/webfetch were used instead.

Best-practice synthesis:
- `rerere` is a strong fit for repeated long-lived-branch conflicts: Git docs explicitly frame it for resolving the same conflicts over and over and provide `git rerere status`, `git rerere diff`, `git rerere forget <pathspec>`, `git rerere remaining`, and `git rerere gc` for audit/correction.
- Keep `rerere.autoupdate` off by default for this repo. Git's own rerere option docs describe `--no-rerere-autoupdate` as a way to double-check rerere's result and catch mismerges before staging, which matters more than convenience during upstream syncs.
- A downstream fork should separate integration mechanics from local product changes: fetch upstream/origin, merge or rebase in a controlled branch, validate, then push with lease only after review. The existing workflow already encodes this at CI level: candidate `master` merge from `upstream/master`, build validation, push `master`, rebase `dev`, build validation, `--force-with-lease` push.
- Script style conventions for any future local wrapper should match `scripts/dev_cargo.sh`: `#!/usr/bin/env bash`, `set -euo pipefail`, derive `repo_root`, `cd` to it, small `log()` helper, explicit env/config validation, arrays for argv, and no hidden destructive push unless explicitly requested.
- Existing customization-record direction is already documented but not implemented as sync tooling: `AGENT_NATIVE_VCS_CORE_BEHAVIOR.md` requires a maintenance packet with why/behavior/upstream attachment/assumptions/tests/drop criteria; `MODULAR_ARCHITECTURE_RFC.md` assigns future customization records and migration logic to `jcode-selfdev`.

Implications for the eight directions:
1. **Define bespoke customization boundaries — implementation-worthy now.** Needed before automation can decide what conflicts are dangerous. Should be a small doc or manifest first, not source-code changes. Boundaries should classify fork-owned, upstream-compatible, and risky overlap areas, then attach validation commands.
2. **Add a repeatable upstream-sync script — implementation-worthy, but as a guarded wrapper/runbook.** Because CI already performs the real sync, the local script should dry-run/status by default, expose exact commands, validate branch/remotes/clean worktree, and optionally run fetch/merge/rebase/build steps. It should not silently push or bypass the GitHub workflow.
3. **Prefer Git rerere — implementation-worthy as policy/docs, not custom code.** Enable `rerere.enabled` is already true globally; document repo policy to keep `rerere.autoupdate` unset/off, require `git rerere diff` and normal `git diff` review before staging, and use `git rerere forget <pathspec>` for bad recorded resolutions.
4. **Track fork-only patches explicitly — implementation-worthy after boundaries.** Best representation is a maintenance-packet manifest matching existing VCS docs: reason, upstream attachment, behavior contract, assumptions, tests, and drop/adapt criteria. Avoid inventing runtime machinery until a simple manifest proves useful.
5. **Use diff-focused review after merge — implementation-worthy now.** Add checklist/runbook steps comparing candidate sync results against boundary files, generated/build files, provider/auth/session paths, workflow files, and lock/config files. Rerere makes this more important because repeated resolutions may be applied automatically.
6. **Split upstream sync from local fixes — implementation-worthy now.** Existing workflow's separate candidate master merge and dev rebase supports this. The local policy should reject mixed commits that combine upstream integration/conflict resolution with unrelated product fixes; follow-up fixes should be separate commits/tasks.
7. **Add validation filters for known conflict areas — implementation-worthy after boundary mapping.** Map path patterns to existing commands such as `scripts/dev_cargo.sh check/build`, `cargo build --profile selfdev -p jcode --bin jcode`, security preflight, budget checks, and targeted auth/e2e smoke scripts. This should be data-driven documentation/script config, not hardcoded guesswork first.
8. **Consider a fork-custom feature/config flag — defer by default; potentially misguided if used for process-only deltas.** A runtime/build flag is justified only when a concrete customization changes shipped behavior and cannot be isolated through docs, patch records, or validation. For upstream-sync process, rerere policy, and patch manifests, a feature flag would add product complexity without preserving anything.

Overall recommendation: implement directions 1, 3, 5, and 6 as documentation/policy first; implement 2 and 7 as guarded script/runbook work after boundaries exist; implement 4 as a minimal manifest before any `jcode-selfdev` productization; defer 8 until a runtime customization proves the need.

### Step 4: Implementation Plan

Plan from 2026-05-28 UTC:

Scope: implement directions 1-7 with a simple documented process plus one guarded local helper script. Explicitly defer direction 8 because current evidence shows the problem is sync/process preservation, not shipped runtime behavior. Revisit a fork-custom feature/config flag only if the manifest identifies a concrete shipped behavior that cannot be preserved with boundaries, patch records, rerere review, or validation.

Minimal elegant implementation:
- Add a concise runbook as the source of truth for fork customization boundaries, upstream-sync procedure, rerere policy, review discipline, validation filters, and patch-record format.
- Add a small Bash wrapper that makes the runbook executable enough for repeatable local use: preflight checks, command printing, optional guarded execution of fetch/merge/rebase/validation phases, and no push unless an explicit flag is passed.
- Avoid product/runtime code and avoid `jcode-selfdev` schema/tooling until simple Markdown records prove their value.

Files to add/change:
1. `docs/UPSTREAM_SYNC_RUNBOOK.md` (add)
   - Purpose and non-goals.
   - Customization boundary table for direction 1:
     - `fork-owned`: local launcher/install channel conventions, self-dev docs, local-only workflow/runbook files, and any fork maintenance records.
     - `upstream-compatible`: normal Rust source, tests, scripts, and docs intended to remain mergeable with `upstream/master`.
     - `risky-overlap`: `.github/workflows/`, `scripts/`, auth/provider/session code paths, build/install scripts, lock/config files, generated/budget artifacts, and any file named in a patch record.
   - Repeatable upstream-sync procedure for direction 2:
     1. Confirm clean worktree except intentional loop/docs work.
     2. Confirm `origin` and fetch-only `upstream` remotes.
     3. Confirm `rerere.enabled=true` and `rerere.autoupdate` unset/false.
     4. Fetch both remotes.
     5. Create/update a local candidate branch from `origin/master` or the documented base.
     6. Merge `upstream/master` into candidate.
     7. Inspect conflicts, rerere status/diff, and full diff.
     8. Run validation filters based on changed paths.
     9. Rebase or update `dev` only after candidate validation.
     10. Push only with explicit operator action and `--force-with-lease` where required.
   - Rerere policy for direction 3:
     - Recommend `git config rerere.enabled true`.
     - Keep `rerere.autoupdate` off by default.
     - Require `git rerere status`, `git rerere diff`, `git diff`, and `git rerere remaining` review before staging conflict resolutions.
     - Use `git rerere forget <pathspec>` for bad resolutions and `git rerere gc` for cleanup.
   - Fork-only patch tracking format for direction 4:
     - Markdown maintenance packet template with `id`, `owner`, `status`, `upstream attachment`, `why`, `behavior contract`, `assumptions`, `validation`, `drop/adapt criteria`, and `touched paths`.
     - Initial records can live under `docs/fork-patches/` as plain Markdown if/when concrete patches are identified.
   - Diff-focused review checklist for direction 5:
     - Review candidate merge diff, dev rebase diff, workflow/script changes, auth/provider/session paths, build/install paths, lock/config changes, generated artifacts, and every patch-record path.
     - Require `git log --graph --oneline --decorate --max-count=30 --all` sanity check.
   - Sync-vs-fix commit policy for direction 6:
     - Integration commits contain only upstream merge/rebase and conflict resolutions.
     - Local product fixes, test fixes, and refactors happen as separate follow-up commits/tasks.
     - Reject mixed commits unless the final summary explicitly justifies an inseparable conflict fix.
   - Validation filter matrix for direction 7:
     - Rust/source/build changes: `scripts/dev_cargo.sh check` and final `cargo build --profile selfdev -p jcode --bin jcode` or remote build if resources are constrained.
     - Script/workflow changes: shell syntax checks where applicable plus targeted dry runs/help output.
     - Auth/provider/session changes: existing targeted auth/e2e smoke scripts when available, otherwise document the exact missing validation as a risk.
     - Security-sensitive changes: `scripts/security_preflight.sh`.
     - Budget/performance-sensitive changes: existing budget/check scripts discovered in `scripts/`.
     - Docs-only changes: markdown review plus link/path sanity checks.
   - Direction 8 decision: deferred/rejected for this iteration because no runtime feature flag is justified by Step 3 evidence.

2. `scripts/upstream_sync.sh` (add)
   - Bash style matches existing scripts: `#!/usr/bin/env bash`, `set -euo pipefail`, derive `repo_root`, `cd` there, `log()` helper, arrays for commands.
   - Default mode is dry-run/plan: print checked state and exact commands without mutating history.
   - Suggested flags:
     - `--execute`: allow non-push mutating operations such as fetch and local branch creation/merge.
     - `--base <branch>` default `origin/master` for candidate sync base.
     - `--upstream <ref>` default `upstream/master`.
     - `--dev <branch>` default `dev`.
     - `--candidate <branch>` default `sync/upstream-master` or timestamped local branch.
     - `--validate`: run validation commands selected from changed paths.
     - `--push`: allowed only with `--execute` and after validation; still prints a warning and uses safe push forms such as `--force-with-lease` for dev.
     - `--no-push`: default.
   - Required behavior:
     - Refuse to run mutating operations with a dirty worktree unless `--allow-dirty-doc docs/UPSTREAM_SYNC_SELF_IMPROVEMENT.md` or equivalent narrow override is provided.
     - Verify remotes exist and `upstream` push URL is disabled or absent.
     - Warn if `rerere.enabled` is not true or `rerere.autoupdate` is true.
     - Print rerere audit commands whenever a merge/rebase reports conflicts.
     - Never silently resolve, stage, commit, or push conflict results.
     - Produce a short post-run checklist pointing back to `docs/UPSTREAM_SYNC_RUNBOOK.md`.
   - Intentionally not included:
     - No Rust product code.
     - No config schema.
     - No automatic PR creation.
     - No custom rerere storage.

3. `docs/fork-patches/README.md` (add)
   - Explain that this directory holds explicit fork-only maintenance packets.
   - Include the packet template copied from the runbook.
   - State that empty/no patch records means no known fork-only runtime patches have been justified yet.

4. `docs/UPSTREAM_SYNC_SELF_IMPROVEMENT.md` (change in later steps only)
   - Step 5 should record implementation details and exact validation commands run.
   - Step 6/7 should review and test the new runbook/script.

Docs structure:
```text
docs/
  UPSTREAM_SYNC_SELF_IMPROVEMENT.md   # loop coordination only
  UPSTREAM_SYNC_RUNBOOK.md            # durable operator runbook
  fork-patches/
    README.md                         # packet index/template
scripts/
  upstream_sync.sh                    # guarded local helper
```

Validation strategy:
- Documentation validation:
  - Confirm all directions 1-7 have explicit sections in `docs/UPSTREAM_SYNC_RUNBOOK.md`.
  - Confirm direction 8 is explicitly deferred with rationale.
  - Confirm `docs/fork-patches/README.md` contains the required packet fields.
- Script static validation:
  - `bash -n scripts/upstream_sync.sh`.
  - Run `scripts/upstream_sync.sh --help` and default dry-run mode from a clean or narrowly allowed worktree.
  - Verify dry-run does not change `git status --short`.
- Script behavior validation:
  - In dry-run, verify it prints remote checks, rerere checks, planned fetch/merge/rebase commands, validation hints, and no push action.
  - In guarded execution tests, use a temporary clone or throwaway branch if mutating behavior is tested.
- Repository validation:
  - For docs/script-only change, run lightweight checks first: shell syntax/help plus targeted grep for direction headings.
  - Final build is not required for documentation/script-only changes unless later implementation touches build/source files; if run, prefer `scripts/dev_cargo.sh check` and selfdev build per repo policy.

Acceptance criteria:
- `docs/UPSTREAM_SYNC_RUNBOOK.md` exists and covers directions 1-7 with actionable operator steps.
- Direction 8 is explicitly deferred/rejected for this iteration with evidence-based rationale and revisit criteria.
- `scripts/upstream_sync.sh` exists, defaults to non-mutating dry-run, and refuses hidden staging/commit/push behavior.
- The script documents and enforces rerere audit expectations without enabling `rerere.autoupdate`.
- `docs/fork-patches/README.md` provides a complete maintenance-packet template.
- Validation commands above pass, and dry-run leaves git status unchanged except for the intended added files.
- Commit history can be split into reviewable groups as described below.

Risks and mitigations:
- Risk: script accidentally mutates or pushes history. Mitigation: dry-run default, explicit `--execute`, explicit `--push`, clean-worktree guard, and no automatic staging/commit.
- Risk: boundary table becomes stale. Mitigation: make patch records reference touched paths and require runbook review during upstream syncs.
- Risk: validation matrix overfits current scripts. Mitigation: keep it path-pattern based and document missing targeted validation as risk rather than hardcoding unsupported commands.
- Risk: rerere reuses a stale or wrong resolution. Mitigation: keep autoupdate off, require rerere diff/full diff review, and document `git rerere forget`.
- Risk: feature flag pressure adds complexity. Mitigation: defer direction 8 until a concrete shipped behavior requires runtime/build selection.

Commit grouping for later implementation, without committing in this Step 4 task:
1. `docs: add upstream sync runbook` containing `docs/UPSTREAM_SYNC_RUNBOOK.md` and `docs/fork-patches/README.md`.
2. `tools: add guarded upstream sync helper` containing `scripts/upstream_sync.sh` plus script validation notes.
3. Optional follow-up `docs: record upstream sync implementation results` updating Step 5+ of `docs/UPSTREAM_SYNC_SELF_IMPROVEMENT.md` after implementation/review.


### Step 5: Implementation Log

Implementation from 2026-05-28 UTC:
- Added `docs/UPSTREAM_SYNC_RUNBOOK.md` as the durable operator runbook. It covers directions 1-7 with sections for customization boundaries, repeatable sync procedure, rerere policy, fork-only patch records, diff-focused review, sync-vs-fix commit policy, and validation filters.
- Explicitly deferred direction 8 in the runbook because current evidence points to process/patch-record preservation rather than runtime or build-time shipped behavior selection. Revisit only when a maintenance packet identifies concrete shipped behavior that docs, patch records, rerere review, and validation cannot preserve.
- Added `docs/fork-patches/README.md` with the fork-only maintenance-packet template and a note that no patch records means no justified fork-only runtime patches are known yet.
- Added executable `scripts/upstream_sync.sh`. The helper defaults to dry-run, validates worktree/remotes/rerere, prints planned fetch/candidate merge/dev rebase/push commands, never stages/commits/conflict-resolves, and requires both `--execute` and `--push` before any push can run.
- During validation, the first dry-run showed that untracked directories appear as `?? docs/fork-patches/`; adjusted the dirty-path allowlist to support explicitly allowed directories while still requiring narrow paths.

Validation run in this step:
- `bash -n scripts/upstream_sync.sh` passed.
- `scripts/upstream_sync.sh --help` passed and showed the dry-run/safety options.
- `scripts/upstream_sync.sh --allow-dirty docs/UPSTREAM_SYNC_SELF_IMPROVEMENT.md --allow-dirty docs/UPSTREAM_SYNC_RUNBOOK.md --allow-dirty docs/fork-patches/README.md --allow-dirty scripts/upstream_sync.sh` passed as a non-mutating dry-run and printed planned commands plus rerere audit and validation-filter guidance.
- No mutating sync operations, commits, or pushes were run.

### Step 6: Critical Review

Review from 2026-05-28 UTC:

Files inspected:
- `docs/UPSTREAM_SYNC_RUNBOOK.md`
- `docs/fork-patches/README.md`
- `scripts/upstream_sync.sh`
- Step 5 implementation log in this shared document

Overall assessment:
- The runbook and fork-patch README cover directions 1-7 at a useful operator level, and direction 8 is deferred with a clear evidence-based rationale.
- The helper script has the right safety intent: dry-run default, explicit `--execute`, no staging/commit/conflict resolution, rerere reminders, and explicit `--push` gating.
- However, several script behaviors are unsafe or misleading enough that they should be fixed before committing the Step 5 implementation.

Must-fix before commit:
1. Dirty worktree directory allowlist can hide unintended files. `git status --short` reports an untracked directory as `?? docs/fork-patches/`; `path_allowed_dirty` currently allows that directory when a single contained file such as `docs/fork-patches/README.md` is allowlisted. That can mask extra untracked files in the same directory and weakens the narrow allowlist safety model.
2. `--execute --push` can push `dev` without executing the printed dev update. The script only logs `git switch $dev_branch` and `git rebase $candidate_branch`; it does not run them. It then calls `run_push_or_print git push --force-with-lease origin "$dev_branch"`, so a push-enabled execute run can force-push the current local `dev` ref even though the candidate validation/update sequence was not actually performed by the helper.
3. Push logging is misleading when push is disabled. Dry-run/default mode still prints `planned push:` lines for candidate and dev. This contradicts the visible `push: disabled` message and can confuse operators about whether push commands are part of the normal plan or gated follow-up actions.
4. Validation coverage is overstated. `--validate` only runs `bash -n scripts/upstream_sync.sh` and prints the validation matrix; it does not select or run validations from changed paths. The runbook says to use changed-path validation filters, and Step 5 describes selected validation commands "when possible", so the helper should either implement changed-path selection or clearly label this as lightweight self-validation only.
5. Execute mode may leave the repository in an in-progress merge state by design after `git merge --no-commit --no-ff`, but the script continues to print later push/post-run guidance unconditionally. It should detect merge success/conflict state and stop with explicit next steps before any push path is reachable.

Nice-to-have / follow-up:
1. Worktree parsing uses human-oriented `git status --short` substring handling. A future hardening pass should prefer porcelain v1/v2 with `-z` to handle spaces, quoting, renames, and unusual path names reliably.
2. `check_remotes` intentionally rejects an active upstream push URL, but Git often reports the fetch URL as the push URL when no separate push URL is configured. The runbook should be precise about the expected disabled-push configuration, or the script should distinguish default push behavior more explicitly.
3. The runbook could state that `scripts/upstream_sync.sh --execute` is for throwaway/candidate contexts only and may intentionally leave a local branch mid-merge for manual audit.
4. The helper could avoid printing push commands entirely unless `--push` is set, and print them as manual follow-up instructions otherwise.
5. The validation matrix is useful but broad; future iterations could add a small changed-path classifier to map actual diffs to concrete commands.

Coverage of the eight directions:
1. Customization boundaries: covered in runbook section 1.
2. Repeatable upstream-sync script: partially covered, but the helper needs the must-fix safety corrections above.
3. Git rerere policy: covered with recommended config and audit commands.
4. Fork-only patch tracking: covered by runbook template and `docs/fork-patches/README.md`.
5. Diff-focused review: covered in runbook section 5.
6. Split upstream sync from local fixes: covered in runbook section 6.
7. Validation filters: documented, but helper support is only advisory/self-validation, not changed-path-driven execution.
8. Fork-custom feature/config flag: properly deferred for this iteration with revisit criteria.

No source, script, or durable doc fixes were made in this review step; only this Step 6 section was updated.

### Step 7: Test and Fix Log

Fixes from 2026-05-28 UTC:
- Tightened dirty worktree allowlist safety in `scripts/upstream_sync.sh` by reading `git status --short --untracked-files=all`, so untracked files inside `docs/fork-patches/` are evaluated individually. A single-file allowlist such as `docs/fork-patches/README.md` no longer allows sibling files in that directory.
- Removed executable push behavior from the helper. `--push` is now a deprecated safety no-op that warns operators; the helper only prints push commands as manual follow-up after review and never calls `git push`.
- Fixed misleading push logging by replacing `planned push:` output with `manual:` follow-up commands while always reporting `push: disabled (manual follow-up only)`.
- Relabeled `--validate` behavior as lightweight helper self-validation only. It runs `bash -n scripts/upstream_sync.sh` and prints the validation matrix, but does not claim changed-path-driven coverage.
- Changed `--execute` flow to stop immediately after candidate merge setup with explicit rerere/diff audit and manual next steps. It no longer proceeds to dev rebase or any push path after a no-commit merge.
- Updated `docs/UPSTREAM_SYNC_RUNBOOK.md` to match the safer helper behavior: execute mode is candidate setup only, and staging, committing, rebasing `dev`, and pushing are manual operator actions outside the helper.

Validation run in this step:
- `bash -n scripts/upstream_sync.sh` passed.
- `scripts/upstream_sync.sh --help` passed and showed the updated no-push/candidate-setup safety model.
- `scripts/upstream_sync.sh --allow-dirty docs/UPSTREAM_SYNC_SELF_IMPROVEMENT.md --allow-dirty docs/UPSTREAM_SYNC_RUNBOOK.md --allow-dirty docs/fork-patches/README.md --allow-dirty scripts/upstream_sync.sh` passed as a non-mutating dry-run and printed planned fetch/candidate merge/dev update commands plus manual-only push follow-up.
- Negative dirty-allowlist test passed: after creating temporary `docs/fork-patches/NEGATIVE_ALLOWLIST_TEST.md`, the same allowlist that permitted only `docs/fork-patches/README.md` failed with `dirty path not allowed: ?? docs/fork-patches/NEGATIVE_ALLOWLIST_TEST.md`; the temporary file was removed.
- `scripts/upstream_sync.sh --push ...` passed as a dry-run safety test: it warned that `--push` is a no-op, emitted no `planned push:` lines, and printed only manual follow-up push commands.
- `scripts/upstream_sync.sh --validate ...` passed and printed `running lightweight self-validation for this helper only`, confirming validation coverage is no longer overstated.
- No mutating sync operations, commits, or pushes were run.

### Step 8: Documentation / Next Loop

Final pass from 2026-05-28 UTC:

Implemented scope:
- Added `docs/UPSTREAM_SYNC_RUNBOOK.md` as the durable upstream-sync operator runbook. It documents customization boundaries, a repeatable guarded sync procedure, rerere policy, fork-only patch tracking, diff-focused review, sync-vs-fix commit discipline, validation filters, and the fork-custom flag decision.
- Added `docs/fork-patches/README.md` as the index/template for explicit fork-only maintenance packets. The current state records that no concrete fork-only runtime patches are justified until a packet is added.
- Added `scripts/upstream_sync.sh` as a local dry-run-first helper. It checks worktree/remotes/rerere, prints planned candidate-merge/dev-update/manual-push commands, supports narrow dirty-path allowlists for planning, and intentionally never stages, commits, conflict-resolves, rebases `dev`, or pushes.
- Updated earlier loop sections with implementation, review, and fix evidence. Step 7 resolved the Step 6 safety issues by making push behavior manual-only, making execute mode stop after candidate setup, tightening untracked dirty-path handling, and relabeling `--validate` as lightweight helper self-validation.

Validation evidence:
- Step 7 validation passed for shell syntax, help output, allowed dry-run, negative dirty-allowlist behavior, deprecated `--push` no-op behavior, and `--validate` self-validation labeling.
- Final Step 8 lightweight validation was run after this update:
  - `bash -n scripts/upstream_sync.sh`
  - `scripts/upstream_sync.sh --help`
  - `scripts/upstream_sync.sh --allow-dirty docs/UPSTREAM_SYNC_SELF_IMPROVEMENT.md --allow-dirty docs/UPSTREAM_SYNC_RUNBOOK.md --allow-dirty docs/fork-patches/README.md --allow-dirty scripts/upstream_sync.sh`
- No mutating sync operation, commit, or push was run in this loop.

Eight-direction disposition:
1. Define bespoke customization boundaries: implemented in runbook section 1.
2. Add a repeatable upstream-sync script: implemented as a guarded helper with dry-run default and candidate-setup-only execute mode.
3. Prefer Git rerere for repeated conflict resolution: implemented as documented policy and helper audit reminders.
4. Track fork-only patches explicitly: implemented as a runbook template plus `docs/fork-patches/README.md` packet directory.
5. Use diff-focused review after merge: implemented in the runbook checklist and helper post-run guidance.
6. Split upstream sync from local fixes: implemented as explicit runbook policy and helper non-commit behavior.
7. Add validation filters for known conflict areas: implemented as a runbook matrix and helper-printed guidance; changed-path-driven automatic validation remains future work.
8. Consider a fork-custom feature/config flag: deferred/rejected for this iteration because current evidence is docs/process/tooling-only, not shipped runtime or build-time behavior. Revisit only when a concrete maintenance packet identifies behavior that cannot be preserved through records, review policy, rerere audit, and validation filters.

Remaining follow-ups for a future loop:
- Optionally add a changed-path classifier to `scripts/upstream_sync.sh` that turns actual candidate diffs into concrete validation command suggestions without running broad checks unexpectedly.
- Consider switching dirty-worktree parsing to porcelain `-z` for unusual paths and stronger rename/copy handling.
- Add real patch records under `docs/fork-patches/` only when a concrete fork-only runtime or build-time customization is identified.
- Keep the existing CI workflow as the authoritative automated sync path; use the local helper for planning/candidate setup and manual audit only.
