---
description: Repository-wide working contract for the independent Jcode hard fork.
applyTo: "**"
---

# Repository contract

This is the independent Jcode hard fork. The live repository and its `main` branch are authoritative; there is no upstream-tracking, patch-stack, or convergence contract. Read `docs/agent-workflows.md` before broad build, CI, release, swarm, or instruction work.

## Work safely

- Default to the TUI/CLI target. Touch desktop code or run desktop builds only when the task is desktop-specific.
- Inspect the working tree before editing. Preserve unrelated and concurrent work, and commit only the files you own.
- Make focused commits as work becomes valid. Do not push, publish, tag, or release unless the user requested it.
- Prefer fixing and validating problems over merely reporting them. Avoid destructive or irreversible actions.

## Sources of truth

- Agent instructions are authored in `.apm/instructions/*.instructions.md`; generated `AGENTS.md`, `CLAUDE.md`, and related surfaces are never hand-edited.
- After instruction-source changes, run `apm compile --validate` and `apm compile`. Use `apm compile --dry-run` when placement may change.
- Use current code, scripts, flake outputs, and workflow files as the authority for commands and check names. Discover volatile details live instead of copying them into prompt files.
- Detailed procedures belong in `docs/agent-workflows.md`. Keep prompt-loaded files concise and free of duplicated command blocks.

## Tools and delegation

- Follow `.jcode/preferred-tools.md` for repository tool routing.
- Use swarm for genuinely separable investigation, implementation, or independent review. Use a subagent for one isolated result. Keep model routing in `.jcode/swarm-prompt.md` and discover available routes with `swarm list_models`.
- Give spawned workers a concrete task, label, and `subagent_type`. Protect ownership boundaries, prefer artifact-based handoffs, and clean up owned workers after completion.

## Validation and closeout

- Run the narrowest useful check while iterating, then the relevant final gate from `docs/agent-workflows.md`.
- Prefer local checks, Nix, remote builders, and the public Cachix cache over routine GitHub CI. Never print, copy, or commit credentials.
- For self-development, use coordinated `selfdev` builds and reloads, then continue automatically and verify the running binary or UI.
- If behavior, architecture, workflow, or ownership changed, update the nearest authoritative documentation in the same change.
