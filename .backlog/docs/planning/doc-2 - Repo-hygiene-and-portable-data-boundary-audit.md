---
id: doc-2
title: Repo hygiene and portable-data boundary audit
type: guide
created_date: '2026-05-28 02:49'
tags:
  - hygiene
  - portable-data
  - flake
  - boundaries
---
# Repo hygiene and portable-data boundary audit

## Scope

This audit supports TASK-18 and the flake transition roadmap. It inventories the current repository boundary before additional flake work and documents cleanup recommendations without deleting runtime state.

Inputs reviewed:
- `AGENTS.md`
- `.gitignore`
- `.backlog/docs/planning/doc-1 - Agents-flake-transition-roadmap.md`
- `git ls-files` tracked inventory
- `git status --short --ignored`
- targeted scans for runtime, secret-like, and generated paths

## Category 1: portable source data

These files are appropriate to keep tracked and portable across machines:

- Rust workspace source and package metadata: `Cargo.toml`, `Cargo.lock`, `build.rs`, `src/`, `crates/`, `tests/`.
- Project docs and governance: `README.md`, `CONTRIBUTING.md`, `RELEASING.md`, `TELEMETRY.md`, `OAUTH.md`, `AGENTS.md`, `docs/`.
- Backlog planning data: `.backlog/config.yml`, `.backlog/tasks/`, `.backlog/milestones/`, `.backlog/docs/`.
- Portable agent content and format documentation: `.jcode/skills/`, `docs/skills/`, `docs/prompts/`, `docs/permissions/`, `docs/specs/`, `.jules/sentinel.md`.
- CI and validation definitions: `.github/`, `scripts/validate_agent_content.py`, budget baselines under `scripts/*_budget.json`, and related validation scripts.
- Fixtures and intentional media/assets used by docs, demos, tests, or product packaging: `assets/`, `docs/images/`, tracked simulator/test JSON, app icons, and README/demo media.

## Category 2: generated artifacts and caches

Observed local generated/cache state that should remain ignored and local:

- `target/` at approximately 61 GiB in this checkout.
- `.direnv/`.
- `scripts/__pycache__/`.

Existing `.gitignore` already covers these through `/target`, `/.direnv/`, and `__pycache__/`. It also covers local scratch/generated locations such as `/tmp/`, `/.wrangler/`, `/.jcode/generated-images/`, and `.envrc.local`.

Tracked generated-like assets appear intentional rather than transient: demo videos, screenshots, app icons, golden JSON, and budget JSON baselines. They should not be removed as part of hygiene work unless a separate size/artifact policy task decides to externalize them.

## Category 3: local runtime state

Runtime state remains outside this repository. Current guidance in `AGENTS.md` points runtime logs and active installs to user-local locations, not tracked repo paths:

- Logs: `~/.jcode/logs/`.
- Active source build channel: `~/.jcode/builds/current/jcode`.
- Stable channel: `~/.jcode/builds/stable/jcode`.
- Versioned builds: `~/.jcode/builds/versions/<version>/jcode`.
- Windows equivalents under `%LOCALAPPDATA%\\jcode`.

No tracked `*.log`, `*.sqlite`, `*.db`, `*.pem`, or `*.key` files were found by the targeted scan. The only tracked env-like file found was `.envrc`, which is a development-shell entrypoint; `.envrc.local` is ignored for machine-local overrides.

## Category 4: deployment-policy concerns

These paths are useful source inputs or examples, but they are not the right place to encode host-specific deployment policy for the upcoming agent-data flake boundary:

- `flake.nix` and `flake.lock`: may expose pure checks/data, but should avoid host-specific home-manager, nix-darwin, launchd, or secret policy.
- `.cargo/config.toml`, `.envrc`, and `.claude/mcp.json`: development/tooling configuration that should stay portable and avoid machine secrets.
- `telemetry-worker/`, `packaging/`, `ios/`, `figma/`, and `mockups/`: product/deployment-adjacent source assets. They are tracked source data, but deployment decisions and runtime provisioning belong elsewhere.
- The roadmap names `/Users/jrudnik/infrastructure/nix-config` as the consumer/integrator. That repo should own installation locations, activation behavior, runtime services, launchd/home-manager/nix-darwin wiring, and secret material.

## Cleanup recommendations

No runtime files should be deleted as part of this task.

Recommended documentation/ignore follow-ups:

1. Keep `.envrc.local` as the documented place for machine-local devshell overrides.
2. Consider adding explicit ignore entries for common local runtime stores if they appear in future work: `.serena/`, `serena/`, `.jcode/logs/`, `.jcode/builds/`, `*.sqlite`, `*.db`, and `*.log`. They were not observed as untracked files in this checkout, so this is preventive rather than urgent.
3. Keep tracked demo media and screenshots unless a dedicated artifact-size policy is adopted. They are currently part of docs/product storytelling, not accidental build output.
4. Keep flake outputs pure-data/check oriented for this repo. Avoid adding host deployment modules here unless they are thin data adapters with no secrets or service policy.
5. Document any future migration of runtime state as a separate task before moving files between `~/.jcode`, nix-config, and this repo.

## Boundary statement

Secrets and runtime services remain outside this repository.

This repository should export portable agent/source data and validation metadata. It should not store tokens, credentials, machine-local databases, logs, build installs, daemon state, or host service definitions. Consumer repositories, especially nix-config, should translate this repo's portable data into deployment-specific configuration and manage secrets through their own secret-management systems.
