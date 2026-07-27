# Agent workflows

This document owns operational guidance for coding agents in this repository. Prompt-loaded files should state policy and link here rather than duplicate command blocks. The live scripts, flake, and workflow files remain authoritative.

## Start every task

1. Read the applicable generated `AGENTS.md` chain.
2. Inspect `git status --short` and preserve unrelated changes.
3. Identify the narrowest validation that can prove the requested outcome.
4. For broad or parallel work, establish file ownership before delegating.

Use `git remote -v`, script help, flake inspection, and workflow inspection to discover current machine or repository state. Do not encode hostnames, credentials, model inventories, toolchain versions, or transient check names in prompt files.

## Search and edit

- Start with `agentgrep` for text, paths, outlines, and relationships.
- Use Serena for language-aware symbol lookup, references, renames, and symbol-level edits.
- Batch independent reads and checks.
- Prefer structured edit tools over shell rewrites. Never replace a whole file when a narrow edit will preserve concurrent work.
- Keep shell commands non-interactive and bounded.

The repository-specific routing summary lives in `.jcode/preferred-tools.md`.

## Rust iteration and tests

Run Rust commands through the repository wrapper so the pinned environment, remote-build policy, and cache policy remain consistent:

```bash
scripts/dev_cargo.sh check -p <crate>
scripts/dev_cargo.sh test -p <crate> <test-filter>
scripts/dev_cargo.sh clippy --all-targets --all-features
```

Use the fast suite when the change crosses several crates:

```bash
scripts/test_fast.sh
```

Run the fork guardrails before closeout for workspace-impacting changes or harvested upstream code:

```bash
scripts/preflight.sh --ratchets-only
scripts/preflight.sh
```

Use `scripts/preflight.sh --help` and `scripts/dev_cargo.sh` source/help output for current options. Set `JCODE_REMOTE_CARGO=0` only when a specifically local Cargo run is required.

## Self-development

This repository defaults to the TUI target. In a self-development session:

1. Use `selfdev build` for a coordinated build.
2. Use `selfdev build-reload` when the new binary should replace the running binary immediately.
3. Continue automatically after reload.
4. Confirm the running revision or behavior. Use `debug_socket` testers and frames for TUI changes.

Use direct local Cargo builds only when `selfdev` is unavailable or the documented fallback is required. Desktop builds and desktop UI debugging are reserved for desktop-specific tasks.

## Nix, remote builders, and caches

Prefer Nix for reproducible final checks and remote builders for resource-heavy Cargo work.

Discover current flake outputs before naming a check:

```bash
nix flake show --all-systems --json
nix eval --json '.#checks'
```

Run the complete local Nix gate when warranted:

```bash
nix flake check --accept-flake-config
```

Inspect remote-build behavior instead of assuming a host or directory:

```bash
scripts/remote_build.sh --help
```

The remote configuration loader is `scripts/remote_config.sh`; configuration is machine-local. Never print the file or its environment if it may contain secret-bearing values.

The flake declares the public Cachix substituter. Treat it as read-only unless an explicitly requested publishing workflow has authenticated upload access. Never reveal, copy, log, or commit Cachix credentials. See [the Nix guide](NIX.md) for the maintained cache and platform contract.

## Incremental-cache safety

Do not delete `target/` or incremental directories ad hoc while builds may be active. Use the repository policies and inspect their current options:

```bash
scripts/clean_target.sh --help
scripts/prune_incremental.sh --help
scripts/test_incremental_policy.sh
```

Automatic pruning is limited to safe repository-owned targets unless an operator explicitly opts into an external target. Do not weaken path, symlink, lock, or active-build guards to make cleanup pass.

## Swarm and subagents

Use `subagent` for one isolated answer. Use `swarm` when work benefits from parallel ownership, dependency-aware execution, or independent review.

Before selecting a non-default model route, run `swarm list_models`. `.jcode/swarm-prompt.md` is the only repository authority for routing guidance.

For spawned workers:

- Provide a concrete prompt, a short label, and a useful `subagent_type` such as `explore`, `implement`, `verify`, or `synthesize`.
- Assign disjoint files or read-only scopes when agents share a worktree.
- Prefer a light task graph for a small flat fan-out. Use deep mode only when explicit critique or verification gates and typed handoff artifacts are valuable.
- In deep mode, complete nodes with findings, evidence, validation, confidence, edge cases, open questions, and what was not checked.
- Prefer shared artifacts, commits, and `await_members` over polling or frequent interruption messages.
- The spawner owns its descendants. Stop or clean up owned workers after their reports land unless they are intentionally retained.

See [swarm architecture](SWARM_ARCHITECTURE.md) and [task-graph semantics](SWARM_TASK_GRAPH.md) for implementation details.

## CI and workflow discovery

Local checks are the normal feedback loop. Do not trigger or wait for GitHub Actions merely to discover failures that local, Nix, or remote checks can reproduce.

Current workflow authority:

- `.github/workflows/docs-impact.yml` produces a non-blocking DOX review packet for the complete pull-request diff.
- `.github/workflows/fork-ci.yml` is the fork's blocking Rust and quality gate.
- `.github/workflows/nix.yml` validates and builds the supported Nix surfaces.
- `.github/workflows/security.yml` owns advisory security checks.
- `.github/workflows/release.yml` owns release artifacts.
- Inherited or dispatch-only workflows are not substitutes for the fork gates.

Discover triggers and job names from the workflow files themselves:

```bash
ls .github/workflows
rg -n '^(name:|on:|jobs:|  [A-Za-z0-9_-]+:)' .github/workflows/*.yml
```

Run `actionlint` for changed fork-owned workflows. Use GitHub CI for pull-request and release confirmation, not routine iteration.

The docs-impact workflow runs when a pull request is opened, updated, reopened, or marked ready for review. It derives affected scopes from the tracked APM `applyTo` declarations and writes the changed paths, applicable instruction sources, and durable-contract review questions to the job summary. It has read-only repository permissions and the job is explicitly non-blocking. Packet-generation errors remain visible in the step logs but do not gate the pull request. Preview the same packet locally with:

```bash
python3 scripts/docs_impact_advisory.py --base <base-revision> --head <head-revision>
```

## Releases

Never bump versions, tag, publish, or push release artifacts unless the user requested a release. When requested, follow [RELEASING.md](../RELEASING.md) and the current release workflow. Use the `/release` skill when available. Discover the last tag and changes live rather than copying a version plan into repository instructions.

## Agent instruction maintenance

APM is the source of truth for generated agent surfaces:

- Edit `.apm/instructions/*.instructions.md` and related APM sources.
- Do not hand-edit generated `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, or client rule directories.
- Run validation and preview before regeneration when scope or placement changes.

APM is not part of the pinned development shell. If `apm` is absent, prefix the commands below with `nix shell nixpkgs#apm-cli --command`.

```bash
apm compile --validate
apm compile --dry-run
apm compile
```

Generated files are intentionally ignored and local. After compilation, use Jcode prompt diagnostics such as `/info` to confirm that project `AGENTS.md`, `.jcode/preferred-tools.md`, and `.jcode/prompt-overlay.md` were loaded.

Run the repository drift check after any instruction change:

```bash
python3 scripts/check_agent_instructions.py
```

The check enforces the prompt budget, required source paths, link integrity, and the rule that operational command blocks live here rather than in prompt-loaded files. When ignored generated surfaces are present locally, it also verifies that their compiled bodies match the APM sources. Hermetic and CI checkouts validate the source projection because those generated files are intentionally not tracked.

## Closeout

- Run the checks that directly prove the requested result.
- Update durable docs when contracts changed.
- Review the diff and working tree for collateral edits.
- Commit only owned files in focused commits.
- Report validation performed and any material limitation that remains.

See [CONTRIBUTING.md](../CONTRIBUTING.md), [branch policy](BRANCHING.md), and [the Nix guide](NIX.md) for adjacent maintained contracts.
