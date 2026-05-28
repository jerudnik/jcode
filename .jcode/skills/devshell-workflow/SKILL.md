---
name: devshell-workflow
description: Dev shell setup, non-interactive Cargo/Nix commands, and hook execution guidance for the Jcode repository.
allowed-tools: bash, read, agentgrep, batch
---

# Dev shell and hook execution workflow

Use this skill when working in the Jcode repository and you need to run Cargo, Nix, pre-commit hooks, repo checks, or diagnose missing development tools.

## Core environment

- The repository ships a Nix dev shell in `flake.nix`.
- The shell provides the Rust toolchain and common native dependencies, including `cargo`, `rustc`, `clippy`, `rustfmt`, `rust-analyzer`, `sccache`, `pkg-config`, `openssl`, and `cmake`.
- It also provides platform-specific native dependencies, including the required Darwin frameworks on macOS and Linux native libraries where applicable.
- `.envrc` contains `use flake` and is intended to auto-load the shell via `nix-direnv`.

## Interactive shell workflow

1. On a fresh checkout, run `direnv allow` once.
2. After that, entering the repository should put the dev shell on `PATH` automatically.
3. Verify with `command -v cargo rustc` or `cargo --version` if tool resolution looks suspicious.

## Non-interactive agent and CI workflow

Agents, CI jobs, and scripts that bypass `direnv` should not assume the dev shell is already loaded.

Prefer one of these forms:

```bash
nix develop -c cargo <args>
direnv exec . cargo <args>
```

Examples:

```bash
nix develop -c cargo check
nix develop -c cargo test -p jcode-core
direnv exec . cargo fmt --check
```

Use the same wrapper pattern for related tools when they come from the dev shell:

```bash
nix develop -c cargo clippy --workspace --all-targets
nix develop -c cargo fmt --all --check
nix develop -c pre-commit run --all-files
```

## Hook execution guidance

- If a hook fails because a tool is missing, rerun the hook inside the dev shell before changing code.
- If `direnv` is not trusted or not loaded in the current process, use `nix develop -c ...` for reproducibility.
- For expensive checks, iterate with targeted commands first, then run the broader hook or workspace command before reporting completion.
- Do not bypass hooks by changing hook configuration unless the task explicitly requires hook maintenance.

## Resource-aware builds

- Prefer fast iteration with `cargo check`, targeted tests, and dev builds.
- When finishing a coding task, rebuild or run the relevant broader validation.
- If local builds are killed or the machine appears resource constrained, use `scripts/remote_build.sh` as described in `AGENTS.md`.
