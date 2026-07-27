---
description: Repository-wide working contract for the independent Jcode hard fork.
applyTo: "**"
---

# Repository contract

This independent hard fork and its `main` branch are authoritative. There is no upstream-tracking, patch-stack, or convergence contract. Read `docs/agent-workflows.md` before broad build, CI, release, swarm, or instruction work.

## Work safely

- Default to TUI/CLI. Touch or build desktop code only for desktop-specific tasks.
- Before planning or changing Git, verify path, branch, HEAD, status, worktrees, ancestry, and authoritative remote; preserve other work.
- After integration, verify remote SHA and delete owned merged branches/worktrees. Push, publish, tag, release, or act destructively only when requested.

## Sources of truth

- Author instructions in `.apm/instructions/*.instructions.md`, never generated `AGENTS.md`, `CLAUDE.md`, or related surfaces. Then run `apm compile --validate` and `apm compile`; use `--dry-run` when placement may change.
- Discover commands and checks from current code, scripts, flake outputs, and workflows. Keep procedures in `docs/agent-workflows.md` and prompt files concise.

## Distribution authority

- Repository-owned end-user distribution is Nix-only: `flake.nix` defines packages and public Cachix is the sole binary publication channel.
- GitHub releases are metadata-only. Do not add executable assets, binary checksums, shell/PowerShell installers, Homebrew/AUR, signed app-store/TestFlight delivery, or Cargo publication.
- The native iOS product is retired. Do not restore `ios/`, `docs/IOS_APP.md`, iOS/TestFlight workflows, Apple signing, APNs fields, or `jcode://pair` native pairing deep links. Preserve the packaged `web/jcode-mobile` browser surface.
- Runtime update commands show Nix guidance only. Developer source rebuilds must not replace or mutate an end-user installation.

## Tools and delegation

- Follow `.jcode/preferred-tools.md`. Use swarm for separable work or review and a subagent for one isolated result; routing lives in `.jcode/swarm-prompt.md` and available routes come from `swarm list_models`.
- Give workers a task, label, and `subagent_type`; protect ownership, prefer artifact handoffs, and clean up owned workers.

## Validation and closeout

- Run narrow checks while iterating and the relevant final gate from `docs/agent-workflows.md`.
- Prefer local checks, Nix, remote builders, and public Cachix over routine GitHub CI. Never expose credentials.
- Use coordinated `selfdev` builds/reloads, continue automatically, and verify the running binary or UI.
- Update the nearest authoritative documentation when behavior, architecture, workflow, or ownership changes.
