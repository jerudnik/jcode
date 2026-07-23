# Fork branch maintenance

This fork has exactly three maintained branches.

```mermaid
gitGraph
  commit id: "upstream jcode"
  branch "vendor/upstream"
  checkout "vendor/upstream"
  commit id: "mirror only"
  branch "distro/nix"
  checkout "distro/nix"
  commit id: "flake packaging"
  branch main
  checkout main
  commit id: "custom fork work"
```

## Branch roles

| Branch | Contents | Rule |
|---|---|---|
| `vendor/upstream` | Upstream `1jehuang/jcode` exactly | First landing place for upstream syncs. No downstream edits. |
| `distro/nix` | `vendor/upstream` plus reusable flake packaging | Nix flake, lockfile, `nix/`, cache/Cachix, packaging workflows, and packaging docs only. |
| `main` | `distro/nix` plus fork customizations | Daily development branch for app behavior, mobile/web/server work, compatibility shims, tests, and fork docs. |

`main` must be a descendant of `distro/nix`, and `distro/nix` must be a descendant of `vendor/upstream`.

## Placement rules

- Put behavior changes on `main`.
- Put mobile, web, server, gateway, assistant, provider, test, and roadmap work on `main`.
- Put reusable packaging and distribution glue on `distro/nix`.
- Put clean upstream updates on `vendor/upstream` first, then rebase `distro/nix`, then rebase `main`.
- Do not merge upstream into downstream branches. Rebase the stack.
- Use `--force-with-lease` for maintained branch updates.

## Manual sync outline

```sh
git fetch github upstream

git switch vendor/upstream
git reset --hard upstream/master

git switch distro/nix
git rebase vendor/upstream
nix flake show --all-systems --json

git switch main
git rebase distro/nix
```

Push only after validation:

```sh
git push --force-with-lease github vendor/upstream
git push --force-with-lease github distro/nix
git push --force-with-lease github main
```

## Local development

Work on `main` unless you are intentionally changing packaging. Topic branches should start from `main` and be folded back into `main` or upstreamed. Do not keep durable remote topic branches in this fork.

The dev shell installs a pre-push guard that refuses accidental pushes to `distro/nix` and `vendor/upstream`. Intentional maintenance can opt in with the documented environment flags.

## Audits

Check the branch contract with:

```sh
git fetch github upstream
git rev-list --left-right --count upstream/master...github/vendor/upstream
git rev-list --left-right --count github/vendor/upstream...github/distro/nix
git rev-list --left-right --count github/distro/nix...github/main
git diff --name-only github/vendor/upstream..github/distro/nix
git diff --name-only github/distro/nix..github/main
```

Or run the codified version of the same checks (used by CI daily and after
every sync):

```sh
scripts/fork-health.sh
```

Expected `distro/nix` touched areas are packaging and fork CI policy:
`.github/workflows/**` (all workflow ownership lives here, never on `main`),
`flake.nix`, `flake.lock`, `nix/**`, `.cargo/audit.toml`, `docs/NIX.md`,
`docs/BRANCHING.md`, packaging-related README sections, and packaging/health
helper scripts. The authoritative allowlist is `allowed_scope_regex` in
`scripts/fork-health.sh`; update both together.

## CI ownership

The `distro/nix` layer owns every file under `.github/workflows/`:

| Workflow | Role | Trigger |
|---|---|---|
| `fork-ci.yml` | The fork's real gate: quality + macOS build/test, advisory Linux tests | push/PR to `main`, weekly strict run |
| `nix.yml` | Flake validation + x86_64-linux/aarch64-darwin builds + Cachix | push/PR touching build inputs |
| `security.yml` | Secret scan + triaged cargo-audit gate; weekly full advisory report | push/PR touching deps, weekly |
| `sync.yml` | 6h upstream mirror + rail rebase (rerere self-healing) | schedule, manual |
| `fork-health.yml` | Rail invariant enforcement via `scripts/fork-health.sh` | daily, after sync, manual |
| `nix-update.yml` | Weekly `flake.lock` bump PR against `distro/nix` | weekly, manual |
| `ci.yml`, `freebsd-smoke.yml`, `release.yml`, `require-issue.yml` | Upstream's workflows, kept byte-close to `vendor/upstream`; dispatch-only or trigger-neutered | manual dispatch (before upstreaming patches) |

`main` must not modify `.github/workflows/` -- that recreates the per-sync
conflict problem the layering exists to solve. `scripts/fork-health.sh` fails
when it does.
