---
id: doc-1
title: Agents flake transition roadmap
type: guide
created_date: '2026-05-27 17:54'
tags:
  - flake
  - nix-config
  - poly-repo
  - roadmap
---
# Agents flake transition planning scaffold

## Goal

Develop `infrastructure/agents` into a portable, flake-consumable source of agent data for `/Users/jrudnik/infrastructure/nix-config`, while supporting a gradual transition toward a dendritic-pattern-friendly poly-repo architecture with better context management and separation of concerns.

## Non-goals and boundaries

- Do not move deployment policy into this repository.
- Do not touch secrets, machine runtime state, or local daemon state.
- Keep this repo primarily vendor-neutral portable agent content: skills, permissions, prompts, specs, and metadata.
- Treat nix-config as the consumer/integrator responsible for home-manager, nix-darwin, launchd, and runtime wiring.
- Prefer additive, reversible scaffolding before migration.

## Staged roadmap

### 1. Foundation: repo hygiene and contracts

Inventory tracked content versus generated/runtime artifacts. Document what belongs in portable agent data, what belongs in local ignored state, and what belongs in nix-config deployment policy. Clean up ignores and repository guidance without deleting runtime state.

### 2. Validation: schemas and checks

Add a validation layer for existing content formats. Start from current format specs and reference examples. Checks should validate content shape, frontmatter, and naming conventions, but remain independent of host-specific deployment assumptions.

### 3. Flake surface: pure data outputs

Optionally add a minimal `flake.nix` that exposes pure data outputs and checks only. Examples: packages or paths for skills/prompts/permissions/specs, a generated catalog, and validation apps/checks. Avoid NixOS/nix-darwin/home-manager modules here unless they remain thin data adapters with no policy.

### 4. Portable MCP catalog metadata

Design portable metadata for MCP catalog entries so clients can consume descriptions, tool boundaries, and references without depending on nix-config internals. nix-config can then translate the catalog into deployment-specific MCP settings.

### 5. nix-config integration boundaries

Document the boundary between this repo and `/Users/jrudnik/infrastructure/nix-config`: this repo exports content and metadata; nix-config decides installation locations, activation behavior, secrets, and runtime services.

### 6. Flake input migration

After validation and pure flake outputs are stable, update nix-config in a separate session/PR to consume `infrastructure/agents` as a flake input. Do not bump or modify nix-config locks from this repo session.

## Risk controls

- Keep every stage independently reviewable.
- Prefer checks and documentation before implementation.
- Avoid broad rewrites of content primitives.
- Add migration shims before removing existing discovery paths.
- Preserve compatibility with clients that read plain Markdown files directly.
