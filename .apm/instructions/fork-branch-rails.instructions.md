---
description: Fork branch model and placement reminders for this hard fork of Jcode.
applyTo: "**"
---

# Fork branch model

This is a **hard fork**: it does not track upstream, and there is no vendor
mirror, packaging rail, or automated sync. The divergence point is the
immutable `fork-point` tag, which the fork-touched clippy/rustfmt gates measure
against, so never move or delete it. See `docs/BRANCHING.md`.

There is one durable rail:

- `main`: everything. Fork behavior, shims, compatibility fixes, app features, packaging, and workflows.

Topic branches (`stack/NN-topic`, `pr/topic`, `exp/topic`) start from `main` and
fold back into it. Do not create durable remote topic branches. The pre-push
hook refuses to recreate the retired `vendor/upstream` and `distro/nix` rails.

Placement rule:

- Reusable app packaging, wrappers, overlays, and Home Manager modules belong in the app fork.
- 4nix consumes app fork outputs. It should not duplicate app-owned packaging unless temporary, documented, and tracked for retirement.
- Use explicit remotes in durable docs and scripts: `upstream`, `github`, and `forgejo`. Avoid assuming `origin`.
- `upstream` is a read-only reference remote. Read it and cherry-pick from it; never rebase a rail onto it. Run `scripts/preflight.sh` after any harvest, and fix imported code rather than raising a budget to admit it.
