---
description: Fork branch rails and placement reminders for this downstream Jcode fork.
applyTo: "**"
---

# Fork branch rails

When working in this forked upstream project, check the current branch before editing.

Durable rails:

- `vendor/upstream`: clean upstream import. Do not make downstream edits here.
- `distro/nix`: reusable Nix packaging only: flake outputs, packages, apps, overlays, Home Manager modules, cache, and CI.
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
