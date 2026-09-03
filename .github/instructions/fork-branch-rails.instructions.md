---
description: Stable branch invariants for the Jcode hard fork.
applyTo: "**"
---

# Fork branch invariants

- This repository is an independent hard fork. `main` is the only long-lived branch and the product authority, and the immutable `fork-point` tag anchors fork-touched quality gates. Never move or delete it.
- There is no upstream-tracking, sync cadence, patch stack, patch ledger, or convergence obligation. If an `upstream` remote is configured, treat it only as optional read-only reference material; imported external code is an ordinary local change and must satisfy this repository's current gates.
- Topic branches start from `main`, flow through pull requests, and fold back into it. Do not recreate retired long-lived branches.
- Discover configured remotes with Git before acting; durable scripts and docs must not assume a remote named `origin`.
- Keep reusable app packaging in this repository and consumer-specific wiring in the consuming repository. See `docs/BRANCHING.md` and `docs/NIX.md` for the maintained contracts.
