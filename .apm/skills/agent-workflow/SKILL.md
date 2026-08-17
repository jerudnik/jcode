---
name: agent-workflow
description: Use when building, testing, or validating code in the jcode repo. Prefer selfdev, remote builders, and Nix; avoid guessing flags or leaking config.
allowed-tools: bash, read, write, edit, agentgrep, batch, todo
---

# Agent workflow

- **Selfdev first.** Use `selfdev build` and `selfdev build-reload` for the self-development loop. Continue automatically after reload and confirm with `debug_socket` testers for TUI changes. Use direct local Cargo only when `selfdev` is unavailable or the documented fallback is required.
- **Cargo runs.** Route Cargo through `scripts/dev_cargo.sh` so the pinned environment, remote-build policy, and cache policy stay consistent. For resource-heavy work such as tests, Clippy, and full checks, use `scripts/remote_build.sh`. Inspect current options instead of memorizing flags.
- **Verify remote offload.** `JCODE_REMOTE_CARGO_FALLBACK` defaults to `local`, so a broken remote silently falls back to a local build. `scripts/dev_cargo.sh` prints a banner when this happens. Watch for the banner. Set `JCODE_REMOTE_CARGO=0` only when a specifically local Cargo run is required.
- **Nix and final gates.** Prefer `nix flake check --accept-flake-config` for reproducible final checks. Discover flake outputs before naming them with `nix flake show --all-systems --json`. Treat Cachix as read-only unless an authenticated publishing workflow is explicitly requested. Never reveal, copy, log, or commit Cachix credentials.
- **Quick feedback.** Reach for the cheapest gate that answers the question. `scripts/preflight.sh --ratchets-only` settles text ratchets in about a minute. Do not trigger or wait for GitHub Actions merely to discover failures that local, Nix, or remote checks can reproduce.
- **Secrets.** `scripts/remote_config.sh` loads machine-local remote-builder configuration. Never print that file or its environment if it may contain secret-bearing values.
