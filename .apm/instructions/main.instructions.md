---
description: Agent guide and working rules for this Jcode downstream fork.
applyTo: "**"
---

# Repository Guidelines

This repo is John's downstream fork of Jcode, an open-source terminal/desktop agent harness. Upstream lives at `https://github.com/1jehuang/jcode`; the GitHub fork lives at `https://github.com/jerudnik/jcode`.

## Development Workflow

- **Commit as you go** - Make small, focused commits after completing each feature or fix.
- If the git state is not clean, or there are other agents working in the codebase in parallel, do your best to still commit only your work.
- **Push when done** - Push all commits to the appropriate fork remote when finishing a task or session, unless the user says not to.
- **Use fast iteration by default** - Prefer `cargo check`, targeted tests, and dev builds while iterating.
- **Rebuild when done** - When you are done making source changes, build the relevant target. In self-dev sessions prefer `selfdev build` or `selfdev build-reload` for the TUI unless the task is desktop-specific.
- **Bump version for releases** - Update `Cargo.toml` only when cutting a release; choose patch/minor based on changes since the last release.
- **Remote builds available** - Use `scripts/remote_build.sh` to offload heavy cargo work if local resources are insufficient.

## Logs

- Logs are written to `~/.jcode/logs/` (daily files like `jcode-YYYY-MM-DD.log`).

## Debug Socket

- Use the debug socket for runtime-level debugging.

## Install Notes

- `~/.local/bin/jcode` is the launcher symlink used from `PATH`.
- `~/.jcode/builds/current/jcode` is the active local/source-build channel; self-dev builds and `scripts/install_release.sh` point the launcher here.
- `~/.jcode/builds/stable/jcode` is the stable release channel; `scripts/install.sh` installs this and points the launcher here.
- `~/.jcode/builds/versions/<version>/jcode` stores immutable binaries.
- `~/.jcode/builds/canary/jcode` still exists for canary/testing flows, but it is not the primary self-dev install path.
- On Windows, the equivalents are `%LOCALAPPDATA%\\jcode\\bin\\jcode.exe` for the launcher, `%LOCALAPPDATA%\\jcode\\builds\\stable\\jcode.exe` for stable, and `%LOCALAPPDATA%\\jcode\\builds\\versions\\<version>\\jcode.exe` for immutable installs; `scripts/install.ps1` currently installs the stable channel.
- Ensure `~/.local/bin` is **before** `~/.cargo/bin` in `PATH`.

## APM-managed agent files

Agent-facing primitives are described once and distributed by **APM** (Agent Package Manager).

- **Edit these sources:** `apm.yml`, `apm.lock.yaml`, and `.apm/instructions/*.instructions.md` / `.apm/skills/<name>/SKILL.md`.
- **Do not hand-edit generated outputs:** `AGENTS.md`, `GEMINI.md`, `CLAUDE.md`, `.claude/rules/`, `.github/instructions/`, `.claude/skills/`, `.agents/skills/`, `.mcp.json`, `.codex/`, and `.gemini/`.
- Use `apm compile` when only local primitives changed. Use `apm install` after changing `apm.yml`, dependencies, or MCP declarations.
- Remote APM deps edited by humans or agents use object form with explicit `git`, `path`, `ref`, and optional `alias`.
- Tool-generated agent primitives are not accepted directly. If an installer writes useful skills/instructions into generated surfaces, import the reviewed durable parts into `.apm/` and regenerate with APM.
- No secrets in the tree. `apm.yml` may contain env var names like `${GITHUB_MCP_PAT}`, never token values.

## Fork and upstream workflow

This fork is maintained as a downstream stack:

- `vendor/upstream` mirrors upstream `1jehuang/jcode`.
- `distro/nix` carries reusable Nix packaging, Home Manager module, cache, release, and CI work.
- `main` carries stable custom fork patches on top of `distro/nix`.

Rules:

- Keep upstream mirror commits separate from downstream commits.
- Prefer rebasing the downstream stack on upstream rather than merging upstream into `main`.
- Use `--force-with-lease`, not plain `--force`, when updating maintained fork branches.
- Track temporary shims and planned upstream work in `docs/fork/patch-ledger.md`.
- Use commit prefixes that describe why downstream changes exist: `compat(...)`, `shim(...)`, `feature(...)`, `behavior(...)`, `distro(...)`, `docs(...)`, and `test(...)`.
- Git remotes use surface names: `github` for GitHub, `upstream` for `1jehuang/jcode`, and `forgejo` for the 4nix Forgejo mirror. Avoid durable docs/scripts that assume `origin`.

## 4nix integration

This fork is consumed by the 4nix flake input `jcode.url = "github:jerudnik/jcode/main"`.

- Preserve the Nix flake outputs, Home Manager module, binary cache metadata, and fork maintenance workflows unless intentionally changing 4nix integration.
- Before changing packaging or app exposure, consider the 4nix `binary-wiring` convention: user-facing CLI exposure belongs in profiles; app-internal subprocesses should prefer direct evaluated store paths when possible.
- Avoid duplicate `bin/jcode` providers in one evaluated profile.
- Keep `docs/NIX.md`, `docs/BRANCHING.md`, and `docs/fork/patch-ledger.md` aligned with packaging/fork behavior.
- If changing branch topology or packaging contracts, update the corresponding 4nix input/runbook expectations.

## Verification

Choose the narrowest useful check while iterating, then run the right final gate:

- Rust logic: targeted `cargo test` or `cargo check -p <crate>`.
- Workspace-impacting Rust changes: `cargo check --workspace`.
- Nix packaging changes: `nix flake show --all-systems --json`, package dry-run/build checks, and relevant workflow docs.
- APM changes: `apm compile --validate`; after dependency changes, `apm install` and `apm audit --ci --no-policy`.
