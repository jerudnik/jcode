# Project State Snapshot

Last refreshed: 2026-05-28 (post TASK-90).

This document summarizes the current source, workflow, and tracking state for
agents and contributors. It is a snapshot, not the live issue tracker. For live
work, use Backlog.md from the repository root.

## Product focus

jcode is a Rust coding-agent harness focused on a fast TUI/CLI, multi-session
workflows, provider/model orchestration, tool execution, swarms, memory,
background work, and resource efficiency. Desktop and mobile support exist in
the tree, but the primary self-development target is still the `jcode` TUI/CLI
binary unless a task explicitly names another surface.

## Source layout

- Root package: `jcode` in `Cargo.toml`, currently version `0.14.3` in source.
- Main binary: `src/main.rs`; library entrypoint: `src/lib.rs`.
- Rust edition: 2024.
- Workspace crates live under `crates/` and split stable type contracts,
  provider integrations, tool contracts/core, storage, compaction, swarms,
  update/self-development support, TUI rendering/style/workspace/session picker,
  mobile, and desktop code.
- Major repository documentation lives under `docs/`; long-running planning
  and task metadata lives under `.backlog/`.

## Work tracking

The canonical task store for this checkout is `.backlog/`, not `backlog/`.
Use the Backlog.md CLI or MCP interface for all task changes and never edit task
Markdown files directly.

Useful commands:

```sh
backlog task list --plain
backlog task <id> --plain
backlog search "topic" --plain
backlog doc list
```

Current open-work themes include provider setup/auth/model management,
compaction quality, dependency-security upgrades, CI and quality guardrails,
ambient mode, the safety system, code-quality decomposition, Windows setup,
MCP/nix-config integration boundaries, and future build/offload work.

Context-hardening status (the TASK-79 through TASK-90 arc):

- TASK-86 / TASK-87: real-Anthropic eval and deterministic 10-technique
  evaluation across 6 scenario kinds completed.
- TASK-88: runtime provenance routing landed in
  `src/agent/context_pruning.rs`.
- TASK-89: runtime `IsolationKey` contract (`jcode-cache-isolation`
  crate) plus routing for `message_render`, `semantic_embed`, repo-map
  `GraphCache`, and openrouter disk memos, with `cache_invalidation.rs`
  hooks on session-resume and provider/model change.
- TASK-90: closed the `public_benchmark` protected-retention plateau
  fixture omission; `combined_p0` now passes the public_benchmark
  reliability gate.
- Remaining: TASK-81 (trimmed) tracks cache types not yet routed
  through `IsolationKey` (skeletons, token estimates, tool/result
  caches, non-openrouter external API caches) plus a reusable
  cache-metrics harness. TASK-80 plans the next context-management
  evaluation batch. The protected-span-aware lazy-restore variant is
  still un-IDed.

Historical execution logs and audit details are kept in docs such as
`docs/CODE_QUALITY_TODO.md`, `docs/CODE_QUALITY_AUDIT_2026-04-18.md`, and
`docs/CODE_QUALITY_10_10_PLAN.md`; those files are references, while `.backlog/`
owns live status.

## CI and quality gates

The current CI quality job enforces formatting, `cargo check --all-targets
--all-features`, clippy with warnings denied, portable agent-content validation,
warning and size budgets, panic/swallowed-error budgets, strict Backlog.md
tracking-divergence checks, and Backlog.md pointer-integrity checks.

The broader CI matrix also covers mobile simulator tests, Linux/macOS release
build and test paths, provider matrix tests, e2e tests, security preflight on
Linux, and Windows build/test smoke coverage.

## Runtime and generated state

Runtime state belongs outside the repository. Current documented locations are:

- Logs: `~/.jcode/logs/`.
- Active source/self-development build channel: `~/.jcode/builds/current/jcode`.
- Stable channel: `~/.jcode/builds/stable/jcode`.
- Immutable versions: `~/.jcode/builds/versions/<version>/jcode`.
- Windows equivalents under `%LOCALAPPDATA%\\jcode`.

The repository ignores build output and local development caches such as
`/target`, `/.direnv/`, `/.envrc.local`, `/.wrangler/`, `/tmp/`, and generated
image scratch space.

## Cross-repo boundary

This repository should export portable source data, portable agent content,
metadata, checks, and binaries. Host deployment policy belongs in consumer
repositories, especially nix-config. That includes installation locations,
activation behavior, launchd/home-manager/nix-darwin wiring, secrets, service
definitions, and host-specific runtime state.

## Refresh procedure

When the project state changes materially:

1. Refresh this file and the README/AGENTS pointers.
2. Update or close affected Backlog.md tasks using the CLI or MCP interface.
3. Update Serena project memories so future agents receive the same current
   guidance.
4. Run the lightweight documentation guardrails, at minimum the Backlog.md
   tracking and pointer checks.
