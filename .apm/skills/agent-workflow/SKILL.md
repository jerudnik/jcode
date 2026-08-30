---
name: agent-workflow
description: Use when building, testing, or validating code in the jcode repo. Prefer selfdev, remote builders, and Nix; avoid guessing flags or leaking config.
allowed-tools: bash, read, write, edit, agentgrep, batch, todo
---

# Agent workflow

- **Selfdev first.** Start with `selfdev status` to confirm the repository, mode, and published binary. Inside a self-development session, use `selfdev build` for a coordinated build and `selfdev build-reload` when the successful build should replace the running binary. Outside self-development, use `selfdev enter` or `selfdev setup`; build actions are intentionally unavailable there. Continue automatically after reload and confirm with `debug_socket` testers for TUI changes. Use direct local Cargo only when `selfdev` is unavailable or the documented fallback is required.
- **Cargo runs.** Route Cargo through `scripts/dev_cargo.sh` so the pinned environment, remote-build policy, and cache policy stay consistent. For resource-heavy work such as tests, Clippy, and full checks, use `scripts/remote_build.sh`. Inspect current options instead of memorizing flags.
- **Verify remote offload.** `JCODE_REMOTE_CARGO_FALLBACK` defaults to `local`, so a broken remote silently falls back to a local build. `scripts/dev_cargo.sh` prints a banner when this happens. Watch for the banner. Set `JCODE_REMOTE_CARGO=0` only when a specifically local Cargo run is required.
- **Nix and final gates.** Prefer `nix flake check --accept-flake-config` for reproducible final checks. Discover flake outputs before naming them with `nix flake show --all-systems --json`. Treat Cachix as read-only unless an authenticated publishing workflow is explicitly requested. Never reveal, copy, log, or commit Cachix credentials.
- **Quick feedback.** Reach for the cheapest gate that answers the question. `scripts/preflight.sh --ratchets-only` settles text ratchets in about a minute. Do not trigger or wait for GitHub Actions merely to discover failures that local, Nix, or remote checks can reproduce.
- **Skill identity.** A declared skill name must resolve to one active `SKILL.md`. Jcode consumes APM content from `.agents/skills`, treats `.apm/skills` as authoring source, and uses `.claude/skills` only when `.agents/skills` has no loadable skills. It rejects duplicates across the selected plugin, global, `.jcode`, and projected source paths and reports every conflict; discovery order is not an override mechanism.
- **Governed APM migration gate.** APM 0.28 root-project integration deploys every `.apm/skills` bundle; `includes` and `--skill` do not filter root content. Do not refresh Jcode's tracked lock or claim frozen restoration while the governed local `plain-language` promotion candidate remains in place, because that would activate a duplicate of the shared canonical skill. Preserve the source until the approved cleanup gate.
- **Secrets.** `scripts/remote_config.sh` loads machine-local remote-builder configuration. Never print that file or its environment if it may contain secret-bearing values.
