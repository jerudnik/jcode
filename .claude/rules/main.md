---
paths:
  - "**"
---

# Repository contract

This independent hard fork and its `main` branch are authoritative. Read `docs/agent-workflows.md` before broad build, CI, release, swarm, or instruction work. Normal integration happens through pull requests into `main`.

## Work safely

- Before planning or changing Git, verify path, branch, HEAD, status, worktrees, ancestry, and configured remotes; preserve other work.
- After integration, verify remote SHA and delete owned merged branches/worktrees. Push, publish, tag, release, or act destructively only when requested.

## Sources of truth

- Author instructions in `.apm/instructions/*.instructions.md`, never a generated `AGENTS.md`, `CLAUDE.md`, or related surface.
- Discover commands and checks from current code, scripts, flake outputs, and workflows. Keep procedures in `docs/agent-workflows.md` and prompt files concise.

## Distribution authority

- End-user distribution is Nix-only: `flake.nix` defines packages and public Cachix is the sole binary publication channel. GitHub releases are metadata-only, and the native iOS product is retired.
- The `nix-distribution-policy` flake check enforces this, including retired iOS paths and banned publication tokens. Read it before changing release, packaging, or update surfaces; do not work around it.
- Runtime update commands show Nix guidance only. Developer source rebuilds must not replace or mutate an end-user installation.

## Tools and delegation

- Follow `.jcode/preferred-tools.md` for tool choice and `.jcode/swarm-prompt.md` for routing and worker structure; available routes come from `swarm list_models`.

## Validation and finishing

- Run narrow checks while iterating and `just pre-pr` before opening any pull request.
- Prefer local checks, Nix, remote builders, and public Cachix over routine GitHub CI.
- Update the nearest authoritative documentation when behavior, architecture, workflow, or ownership changes.
