# F18 evidence — Real Nix package build + launch on pull requests

**Node:** F18 (implement / deterministic / W3) · audit A12 "Real Nix package build and launch".

## The gap F18 closed

Before this change, `.github/workflows/nix.yml` gated the `build` job with
`if: github.event_name != 'pull_request'`. On PRs the real package was **never
built or launched** — PRs only got `nix build --dry-run` plus
`nix flake check --no-build`. That violates the F18 acceptance gate: *relevant
path changes must run a real `nix build` and prove `result/bin/jcode --version`
on the pull_request event.*

## The change

`.github/workflows/nix.yml`:

1. Removed the `if: github.event_name != 'pull_request'` gate from `build`, so
   PRs now build and smoke-launch the actual package.
2. Added a tiny `matrix` job that selects the build matrix by event:
   - **pull_request → `x86_64-linux` only.** It is the fork's CI validation
     platform, runs on fast hosted ubuntu, and reads the public Cachix cache
     (`skipPush: true`) so it does not rebuild ~900 crates from source.
   - **push / workflow_dispatch → full matrix** (`x86_64-linux` +
     `aarch64-darwin`). Darwin cache misses are expensive on hosted macOS and a
     per-PR rebuild there is not worth the wall-clock cost (90-min timeout).
   The `build` job consumes it via `strategy.matrix.include:
   ${{ fromJSON(needs.matrix.outputs.include) }}`.
3. Relabeled the smoke step **"Smoke test binary (launch gate)"** and documented
   that it is the F18 acceptance proof on every event, PRs included.

### What was deliberately NOT changed

- **`doCheck = false` stays** in `nix/package.nix`. F18's wording — "without
  enabling non-hermetic package `doCheck` blindly" — means we prove *packaging +
  launch*, not test execution. The workspace suite assumes a writable
  `$HOME/.jcode`, network, and a terminal (F15-audited non-hermetic); it is
  owned by `ci.yml` / `fork-ci.yml`, not the sandbox.
- **`flake.nix` `checks = { }` stays empty** — no re-running of the suite via the
  flake's rust-overlay toolchain.
- **Cache push stays off for PRs.** `CACHIX_CAN_PUSH` already excludes
  `pull_request` (and forks without secrets); PRs use the read-only
  `skipPush: true` Cachix step and never push.
- **No double-build.** The `validate` job keeps its `--dry-run` +
  `flake check --no-build` (cheap evaluation only); the real build happens once
  per selected system in `build`.

## Evidence artifacts

- `local-darwin-build.log.txt` — truncated log of a real, from-source
  `nix build .#packages.aarch64-darwin.jcode` on the maintainer Mac (proves the
  package builds cold, ~900 crates, without the binary cache).
- `local-darwin-version.txt` — `result/bin/jcode --version` output:
  `jcode v0.46.0 (c481344)`. Clean release string (no `-dev`), confirming the
  package launches.
- `ci-pr-run.md` — the pull_request Nix workflow run showing a real `nix build`
  + `result/bin/jcode --version` on `x86_64-linux` for the PR event (added once
  CI is green).

## Local reproduction (Mac, aarch64-darwin)

```
export PATH="/etc/profiles/per-user/jrudnik/bin:$PATH"
nix build .#packages.aarch64-darwin.jcode --print-build-logs --accept-flake-config
./result/bin/jcode --version   # -> jcode v0.46.0 (<shortRev>)
```
