# Fork Sustainability Review Synthesis

Date: 2026-06-27
Status: review synthesis to fold into `FORK_SUSTAINABILITY_MODEL.md`

## Short-term vision

Use the simplest sustainable workflow first.

- `main` is John's living daily fork: upstream + Nix packaging + stable personal downstream behavior.
- 4nix installs and manages jcode as a flake input. It remains the stable/fallback world, not the patch-stack manager.
- `nix develop` is the reproducible jcode development environment.
- `nix run .` or the Home-Manager installed `jcode` is the packaged fallback.
- Selfdev is the dogfood edge: use `selfdev build-reload` while hacking jcode.
- Do not implement Nix feature stacks first. Keep them as an escape hatch for unusually invasive or mutually exclusive work.
- Make the daemon's identity impossible to miss: client binary, server binary, source checkout, commit, dirty state, and compatibility verdict.
- Keep up with upstream through the existing rails and `scripts/sync-local.sh`; treat repeated conflicts as requests for extension seams.

## Longer-term work

The long-term goal is explicit operating modes without cognitive overhead.

1. Add NS4 provenance stamping first: every build and daemon reports source path, commit, dirty state, and binary origin.
2. Add a protocol/capability compatibility gate so incompatible selfdev/stable client-server combinations fail loudly or reconnect cleanly.
3. Add `jcode doctor` as the main visibility command. It should explain PATH/client/server identity, branch drift, upstream sync state, selfdev channels, and fallback commands.
4. Shrink the conflict surface by converting repeated invasive edits into additive seams: config, registries, traits, hooks, prompt/tool/session extension points.
5. Later, add named daemon instances if needed:
   - stable instance: Nix-store binary, stable runtime dir/socket
   - selfdev instance: mutable selfdev binary, selfdev runtime dir/socket
   - clients attach intentionally to one instance
6. Only after these basics are working, consider Nix feature variants for the few features that need separate binary identity or explicit mutually exclusive stacks.

## Overall vision

The fork is allowed to live in bounded selfdev mode.

That means John can use a moving personal fork every day, even while using jcode to modify itself, without pretending that all downstream features are temporary or upstream-bound. The safety model is not "never run selfdev." The safety model is:

- selfdev is visible
- stable fallback is always available
- upstream sync is frequent and boring
- repeated conflicts become architecture seams
- downstream intent is recorded in the patch ledger
- cheap validation happens before expensive builds
- Nix provides reproducible environment, stable package, cache, and fallback
- selfdev provides the daily moving edge

The simplest durable slogan:

> Main is the living fork. Nix is the fallback and environment. Selfdev is the dogfood edge. Doctor explains which world is running. Repeated conflicts become seams.
