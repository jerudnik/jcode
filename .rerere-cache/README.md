# `.rerere-cache/` -- shared recorded conflict resolutions

This directory is the **shared transport** for `git rerere` ("reuse recorded
resolution"). It is committed to the repo on purpose.

## Why it exists

CI rebases `distro/nix` and `main` onto a fast-moving upstream every six hours
(`.github/workflows/nix.yml`, `sync-upstream`). The few files this fork rewrites
conflict against upstream the same way every cycle. `git rerere` records how a
conflict hunk was resolved and replays it automatically next time -- but it stores
recordings in `$GIT_DIR/rr-cache`, which is per-clone and never pushed. A fresh
CI checkout therefore starts empty and re-fails the same conflict forever.

Committing the recordings here lets CI (and any clone) **import** them and replay
your one-time resolution headlessly.

## Workflow

- A clone enables rerere and imports these recordings automatically on dev-shell
  entry (`scripts/rerere-cache.sh setup`, called from the flake `shellHook`).
- When you resolve a *new* recurring conflict during a local `sync-local.sh`
  rebase, capture and commit it:

  ```sh
  scripts/rerere-cache.sh export
  git add .rerere-cache && git commit -m "fork: record rerere resolution"
  ```

- CI imports these before its rebase and auto-continues on recognised conflicts;
  a genuinely new conflict fails the job loudly so a human resolves it once.

## What's stored

One subdirectory per conflict signature, each holding a `preimage` (the conflict
shape) and `postimage` (your resolution). Only *resolved* entries are tracked;
in-progress `preimage`-only recordings are not exported.

This directory is outside the Nix build `src` fileset and the CI build-path
filters, so updating it never triggers a rebuild.
