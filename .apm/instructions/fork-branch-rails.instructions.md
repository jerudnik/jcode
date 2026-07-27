---
description: Stable branch invariants for the Jcode hard fork.
applyTo: "**"
---

# Fork branch invariants

- This repository is a hard fork. `main` is the only durable rail, and the immutable `fork-point` tag anchors fork-touched quality gates. Never move or delete it.
- Treat `upstream` as read-only reference material. Cherry-pick selected fixes when useful, never rebase a maintained branch onto upstream, and run `scripts/preflight.sh` after a harvest.
- Topic branches start from `main` and fold back into it. Do not recreate retired durable rails.
- Discover configured remotes with Git before acting; durable scripts and docs must not assume a remote named `origin`.
- Keep reusable app packaging in this repository and consumer-specific wiring in the consuming repository. See `docs/BRANCHING.md` and `docs/NIX.md` for the maintained contracts.
