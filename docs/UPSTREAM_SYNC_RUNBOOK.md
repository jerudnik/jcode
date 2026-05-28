# Upstream Sync Runbook

This runbook preserves local fork customizations while integrating `upstream/master` into this fork. It is intentionally process-oriented: it documents boundaries, review discipline, rerere use, validation filters, and patch-record expectations without adding product/runtime behavior.

## Non-goals

- Do not bypass `.github/workflows/sync-dev-upstream.yml` or existing CI.
- Do not silently stage, commit, force-push, or create pull requests.
- Do not add a fork-custom runtime/config flag until a concrete shipped behavior requires it.

## 1. Customization boundaries

| Boundary | Meaning | Current examples | Required review/validation |
| --- | --- | --- | --- |
| `fork-owned` | Local process, self-dev, or fork maintenance content that should be preserved unless deliberately retired. | Local launcher/install channel notes, self-dev docs, `docs/UPSTREAM_SYNC_*`, `docs/fork-patches/`, local fork maintenance records. | Diff review plus docs/link sanity checks. If scripts are touched, run syntax/help dry-runs. |
| `upstream-compatible` | Normal repo code and docs expected to remain mergeable with `upstream/master`. | Rust source, tests, scripts, docs, config where changes are intended for eventual upstream compatibility. | Use changed-path validation filters below, with build/test checks for code. |
| `risky-overlap` | Areas where upstream and fork work are likely to collide or where small conflicts can change behavior. | `.github/workflows/`, `scripts/`, auth/provider/session paths, build/install scripts, lock/config files, generated or budget artifacts, and every path listed in a fork patch record. | Mandatory diff-focused review, rerere audit when conflicts occur, and targeted validation from the matrix. |

## 2. Repeatable upstream-sync procedure

Use `scripts/upstream_sync.sh` as a guarded local helper. Its default mode is a dry-run plan that prints checks and commands rather than mutating history. `--execute` is for candidate-branch setup only: it may fetch, reset/switch the candidate branch, and start the no-commit merge, then it stops for manual audit. The helper never stages, commits, rebases `dev`, or pushes; any push commands it prints are manual follow-up after operator review.

1. Confirm the worktree is clean except intentional loop/docs work. If needed for local planning only, pass narrow `--allow-dirty <path>` entries for known files.
2. Confirm `origin` and `upstream` remotes exist. `upstream` must not have an active push URL.
3. Confirm `rerere.enabled=true` and `rerere.autoupdate` is unset or false.
4. Fetch both remotes.
5. Create or reset a local candidate branch from the documented base, default `origin/master`.
6. Merge `upstream/master` into the candidate branch with no automatic commit.
7. Inspect conflicts, `git rerere status`, `git rerere diff`, `git rerere remaining`, and the full `git diff` before staging any conflict resolution.
8. Run validation filters based on changed paths.
9. Rebase or update `dev` only after candidate validation.
10. Push only by explicit operator action outside the helper. Use `--force-with-lease` where history is rewritten.

Keep the upstream integration commit/rebase separate from local fixes. If a conflict exposes a local bug, finish the integration first, then make a follow-up fix commit/task.

## 3. Rerere policy

Recommended setup:

```bash
git config rerere.enabled true
git config --unset rerere.autoupdate || true
```

Policy:

- Keep `rerere.autoupdate` off by default so reused resolutions stay visible for review before staging.
- During conflicts, run `git rerere status`, `git rerere diff`, `git rerere remaining`, and `git diff` before `git add`.
- If rerere reuses a bad resolution, run `git rerere forget <pathspec>` before resolving again.
- Use `git rerere gc` only for cleanup of stale records, not as part of normal sync.
- Never trust rerere alone for `risky-overlap` files.

## 4. Fork-only patch tracking

Concrete fork-only changes live as maintenance packets under `docs/fork-patches/`. If there are no records, there are no known justified fork-only runtime patches.

Template:

```markdown
# <short patch title>

- id: fork-patch-YYYYMMDD-slug
- owner: @owner-or-team
- status: proposed | active | adapting | retired
- upstream attachment: upstream issue/PR/commit/path, or `none`
- touched paths:
  - path/from/repo/root

## Why

What local need this patch preserves.

## Behavior contract

Observable behavior that must survive upstream syncs.

## Assumptions

What must remain true for this patch to be valid.

## Validation

Commands or manual checks that prove the behavior contract.

## Drop/adapt criteria

When to remove, upstream, or redesign this patch.
```

## 5. Diff-focused review checklist

Before any push:

- Review candidate merge diff and dev rebase diff.
- Review all `.github/workflows/` and `scripts/` changes.
- Review auth/provider/session paths.
- Review build/install paths and launcher/channel conventions.
- Review lock/config files and generated or budget artifacts.
- Review every path named by a fork patch record.
- Run history sanity check:

```bash
git log --graph --oneline --decorate --max-count=30 --all
```

## 6. Sync-vs-fix commit policy

- Integration commits contain only upstream merge/rebase changes and necessary conflict resolutions.
- Local product fixes, test fixes, and refactors must be separate follow-up commits/tasks.
- Reject mixed commits unless the final summary explicitly explains why a conflict fix could not be separated.
- Never stage, commit, rebase `dev`, or push from `scripts/upstream_sync.sh`; the operator must review and act explicitly.

## 7. Validation filter matrix

| Changed paths | Minimum validation |
| --- | --- |
| Rust source, Cargo files, build-sensitive code | `scripts/dev_cargo.sh check`; final `cargo build --profile selfdev -p jcode --bin jcode` or `scripts/remote_build.sh` if local resources are constrained. |
| Scripts or workflows | Shell syntax checks where applicable, plus targeted `--help` or dry-run output. |
| Auth/provider/session paths | `scripts/auth_regression_matrix.sh`, `scripts/test_auth_e2e.sh`, or `scripts/real_provider_smoke.sh` when applicable. If unavailable for the touched path, document the missing validation risk. |
| Security-sensitive paths | `scripts/security_preflight.sh`. |
| Budget/performance-sensitive paths | Relevant budget scripts such as `scripts/check_code_size_budget.py`, `scripts/check_panic_budget.py`, `scripts/check_swallowed_error_budget.py`, `scripts/check_test_size_budget.py`, `scripts/check_startup_budget.sh`, or `scripts/check_warning_budget.sh`. |
| Docs-only changes | Markdown review, path/link sanity checks, and script syntax checks for embedded runnable examples when practical. |

## 8. Fork-custom feature/config flag decision

Deferred for this iteration. Step 3 evidence shows the immediate problem is preserving sync process, patch records, rerere review, and validation discipline, not selecting alternate shipped behavior at runtime or build time. A fork-custom feature/config flag should be reconsidered only when a maintenance packet identifies concrete shipped behavior that cannot be preserved by documentation, patch records, conflict-review policy, or validation filters.
