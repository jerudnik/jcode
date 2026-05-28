---
id: doc-3
title: Current project state snapshot
type: guide
created_date: '2026-05-28 07:28'
tags:
  - current-state
  - planning
  - process
  - boundaries
---
# Current project state snapshot

Last refreshed: 2026-05-28.

This Backlog.md document mirrors the repository-facing snapshot in `docs/PROJECT_STATE.md` and records the planning state used by agents.

## Current source state

- jcode is a Rust 2024 Cargo workspace.
- The root source package is `jcode` at version `0.14.3`.
- The primary self-development surface is the TUI/CLI binary in `src/main.rs` unless another surface is explicitly requested.
- Workspace crates under `crates/` split provider, tool, storage, compaction, swarm, TUI, update/self-development, mobile, and desktop domains.

## Tracking state

- The canonical Backlog.md store for this checkout is `.backlog/`.
- Agents must mutate tasks and Backlog-managed docs only through the Backlog.md CLI or MCP interface.
- Historical code-quality docs now act as references; live status belongs to Backlog.md tasks.

## Current active themes

Open work is centered on provider setup/auth/model UX, compaction quality, dependency security, CI and quality guardrails, ambient mode, safety-system phases, code-quality decomposition, Windows setup, MCP/nix-config boundaries, and future build/offload work.

## Boundary statement

This repository owns portable source data, portable agent content, metadata, checks, and binaries. Consumer repositories, especially nix-config, own deployment policy, activation behavior, launchd/home-manager/nix-darwin wiring, secrets, services, and host-specific runtime state.

## Validation state

CI quality guardrails currently cover formatting, all-target/all-feature check and clippy, portable agent-content validation, warning/size/panic/swallowed-error budgets, strict Backlog.md tracking-divergence checks, and Backlog.md pointer-integrity checks.

## Refresh guidance

When this state changes materially, update `docs/PROJECT_STATE.md`, `AGENTS.md`, this Backlog.md document, Serena memories, and affected Backlog.md task statuses in the same change set.
