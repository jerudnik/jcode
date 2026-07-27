---
description: Fork branch rails and placement reminders for this downstream Jcode fork.
applyTo: "**"
---

# Fork branch rails

When working in this forked upstream project, check the current branch before editing.

This is a **hard fork**: it does not track upstream, and there is no vendor
rail or automated sync. The divergence point is the immutable `fork-point` tag.
See `docs/BRANCHING.md`.

Durable rails:

- `distro/nix`: reusable Nix packaging only: flake outputs, packages, apps, overlays, Home Manager modules, cache, and **all** workflow files.
- `main`: stable custom fork. Put fork behavior, shims, compatibility fixes, and app features here.
- `stack/NN-topic`, `pr/topic`, or `exp/topic`: ordered review, upstream-PR, or disposable experiment work before folding into `main` or upstreaming.

Before changing files, run:

```sh
git branch --show-current
git remote -v
```

Placement rule:

- Reusable app packaging, wrappers, overlays, and Home Manager modules belong in the app fork.
- 4nix consumes app fork outputs. It should not duplicate app-owned packaging unless temporary, documented, and tracked for retirement.
- Use explicit remotes in durable docs and scripts: `upstream`, `github`, and `forgejo`. Avoid assuming `origin`.
- `upstream` is a read-only reference remote. Read it and cherry-pick from it; never rebase a rail onto it. Run `scripts/preflight.sh` after any harvest, and fix imported code rather than raising a budget to admit it.
