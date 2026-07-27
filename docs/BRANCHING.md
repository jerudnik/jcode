# Fork branch maintenance

This is a **hard fork** of `1jehuang/jcode`. It does not track upstream.

```mermaid
gitGraph
  commit id: "fork-point (tag)"
  branch "distro/nix"
  checkout "distro/nix"
  commit id: "flake packaging"
  branch main
  checkout main
  commit id: "fork work"
```

## Branch roles

| Branch | Contents | Rule |
|---|---|---|
| `distro/nix` | `fork-point` plus reusable flake packaging | Nix flake, lockfile, `nix/`, cache/Cachix, **all** workflows, and packaging docs only. |
| `main` | `distro/nix` plus fork work | Daily development branch for app behavior, mobile/web/server work, tests, and fork docs. |

`main` must be a descendant of `distro/nix`.

## The `fork-point` tag

`fork-point` is an annotated tag on `631935dd1d`, the `1jehuang/jcode` commit
this fork diverged from. It is immutable and must never be moved.

It is load-bearing, not commemorative. The fork-touched clippy and rustfmt
gates compute their file set as `git diff fork-point HEAD -- '*.rs'`, so lints
are blocking in code this fork owns and merely advisory in untouched upstream
code. Moving or deleting the tag silently changes what those gates measure.
`scripts/fork-health.sh` check 2 verifies it is still an ancestor of both rails.

## Why the fork is hard

The fork previously maintained a third rail, `vendor/upstream`: a byte-identical
mirror of upstream `master`, fast-forwarded every six hours, with `distro/nix`
and `main` rebased onto it. That model was retired for reasons that were
measured rather than assumed:

- **It had already stopped working.** `sync.yml` failed 29 of its last 30 runs;
  the last success was July 4. It was blocked for roughly three weeks on a
  one-line conflict in `release.yml`. The failure alert *itself* failed
  (`Resource not accessible by integration`), so it failed silently.
- **The cost was large and growing.** A sync at retirement time meant 247
  conflicted files across 651 hunks, 387 of them semantic Rust conflicts.
- **The accounting cost exceeded the benefit.** `.rerere-cache`, the recorded
  conflict resolutions that made repeated rebases survivable, had grown to
  202k lines: 60% of every new file in the fork, and pure scar tissue.
- **The benefit was small.** Of 678 upstream commits at retirement, only 52
  touched files this fork had never modified, and only 20 cherry-picked
  cleanly. Those 20 were triaged and the worthwhile 8 were taken; see the
  harvest commits preceding `c91cb4bac`.

Divergence is intentional. This fork has substantially rewritten storage,
telemetry consent, session handling, and CI policy, and has quality gates
(warning budget, swallowed-error and panic ratchets, code-size ceilings,
dependency boundaries) that upstream code does not satisfy.

`upstream` remains configured as a **read-only reference remote**. Fetch it to
read code or cherry-pick a specific fix; never rebase a rail onto it.

## Placement rules

- Put behavior changes on `main`.
- Put reusable packaging and distribution glue, and **all** workflow files, on
  `distro/nix`.
- Use `--force-with-lease` for maintained branch updates.

## Taking a specific fix from upstream

There is no automated sync. To adopt an individual upstream fix:

```sh
git fetch upstream
git log --oneline fork-point..upstream/master   # browse
git cherry-pick -x <sha>                        # -x records the origin
```

Then run `scripts/preflight.sh`. Upstream code frequently does not satisfy this
fork's gates (oversized files, swallowed errors, clippy lints), and the
harvested commit must be brought up to standard rather than the budget raised
to accommodate it. Applying cleanly says a patch does not textually conflict;
it says nothing about whether the result is correct here.

## Local development

Work on `main` unless you are intentionally changing packaging. Topic branches
should start from `main` and be folded back into `main`. Do not keep durable
remote topic branches in this fork.

The dev shell installs a pre-push guard that refuses accidental pushes to
`distro/nix` (opt in with `JCODE_ALLOW_DISTRO_NIX_PUSH=1`) and unconditionally
refuses to recreate `vendor/upstream`.

## Audits

```sh
scripts/fork-health.sh
```

Checks: the rail set is exactly `{main, distro/nix}`; `fork-point` is an
ancestor of both; `distro/nix` is an ancestor of `main`; the `distro/nix`
payload stays within the packaging/CI-policy scope; and `main` adds no workflow
changes.

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
| `fork-health.yml` | Rail invariant enforcement via `scripts/fork-health.sh` | daily, manual |
| `nix-update.yml` | Weekly `flake.lock` bump PR against `distro/nix` | weekly, manual |
| `ci.yml`, `freebsd-smoke.yml`, `windows-smoke.yml`, `release.yml`, `require-issue.yml` | Inherited upstream workflows; dispatch-only or trigger-neutered | manual dispatch |

`main` must not modify `.github/workflows/`. `scripts/fork-health.sh` fails
when it does.
