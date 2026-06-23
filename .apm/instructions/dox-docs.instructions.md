---
description: DOX contract for docs/ — fork maintenance, downstream patch ledger, and 4nix integration notes.
applyTo: "docs/**"
---

# docs/ — Fork and integration docs (DOX)

## Purpose

Code-adjacent reference for maintaining the downstream fork: branch topology, Nix packaging, GitHub workflow behavior, downstream patch ledger, and 4nix integration contracts.

## Ownership

- `BRANCHING.md` — branch roles, rebase flow, patch classes, and CI ownership.
- `NIX.md` — flake packaging, Home Manager module, Cachix, and install/use notes.
- `fork/patch-ledger.md` — durable ledger for temporary shims, upstreamable patches, and permanent downstream features.
- Other docs stay close to implementation and should not duplicate upstream docs unless the fork changes behavior.

## Local Contracts

- Keep branch topology docs aligned with actual maintained branches and GitHub workflow names.
- Every temporary shim, compatibility workaround, or planned upstream PR must have an entry in `docs/fork/patch-ledger.md` with a retire condition and validation command.
- Permanent downstream behavior can stay in the ledger, but label it clearly as `permanent-downstream`.
- 4nix-facing packaging contracts belong in docs when they affect the `github:jerudnik/jcode/main` flake input.
- Use direct, operational prose. Do not leave historical diary notes in durable docs.

## Work Guidance

- Update docs in the same commit as branch, packaging, workflow, or fork-policy changes.
- Prefer function-oriented validation language: say what behavior is proved and then list the command.
- Avoid generic remote names in durable docs. Use `github`, `upstream`, and `forgejo`.

## Verification

Prose has no build gate. When a doc states a command, ensure the command exists or explain its prerequisites.
