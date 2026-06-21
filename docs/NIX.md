# Using jcode with Nix

This branch ships a [Nix flake](https://nixos.wiki/wiki/Flakes) so jcode can be
built, run, installed reproducibly, and reused as a flake input by downstream
configurations. The flake is intentionally unopinionated: it exposes the package,
an overlay, an app, a dev shell, CI checks, and a Home Manager module, but bakes
in no personal configuration.

## Branches

The upstream-mirror branch is `main` and is kept free of fork packaging commits.
Use the Nix packaging branch explicitly in flake URLs:

```sh
github:jerudnik/jcode/nix-flake
```

See [BRANCHING.md](BRANCHING.md) for the full downstream-only maintenance
procedure.

## Supported platforms

`x86_64-linux`, `aarch64-linux`, and `aarch64-darwin`.

## Quick start

Run without installing:

```sh
nix run github:jerudnik/jcode/nix-flake
```

Build the binary:

```sh
nix build github:jerudnik/jcode/nix-flake
./result/bin/jcode --version
```

## Binary cache (skip building from source)

A public Cachix cache (`jerudnik-jcode`) serves prebuilt outputs. Add it once and
Nix will pull binaries instead of compiling the workspace:

```nix
nix.settings = {
  substituters = [ "https://jerudnik-jcode.cachix.org" ];
  trusted-public-keys = [
    "jerudnik-jcode.cachix.org-1:WL5DX0TS/0N/BIW6RDnFGKpkZX9eT2DwFJK+05cpIZQ="
  ];
};
```

The flake also declares this cache in `nixConfig`, so `nix run`/`nix build`
against the flake URL will offer to use it automatically (consumers opt in with
`--accept-flake-config`).

The cache only ever stores successful `nix build` outputs from trusted branch
builds, signed with a key whose private half lives solely in CI secrets. It is
safe to expose publicly and safe for others to consume.

### Maintaining the cache

CI (`.github/workflows/nix.yml`) pushes successful build outputs automatically
from `nix-flake` once the `CACHIX_AUTH_TOKEN` repo secret is set. Pull requests
consume the cache read-only and do not push. To set up or re-key:

1. The cache is `jerudnik-jcode` on Cachix.
2. Generate a push auth token in the Cachix UI and store it as the
   `CACHIX_AUTH_TOKEN` GitHub Actions secret.
3. If the signing key is ever rotated, update the public key in `flake.nix` (the
   `nixConfig` block) and in this doc.

### Automated maintenance

The default branch carries one fork-only exception:
`.github/workflows/fork-maintenance.yml`. It is a tiny scheduler that dispatches
the real branch-local maintenance work on `nix-flake`.

Upstream sync runs every six hours. It rebuilds `main` from `upstream/master`
plus the scheduler workflow, rebases `nix-flake`, pushes it, and lets the normal
Nix CI surface breakage early.

`flake.lock` updates run weekly on Monday at 06:47 UTC, after the 06:17 UTC
sync window. To trigger either task manually:

```sh
gh workflow run "Fork maintenance" --repo jerudnik/jcode -f task=sync-upstream
gh workflow run "Fork maintenance" --repo jerudnik/jcode -f task=update-flake-lock

# Or dispatch the branch-local workflow directly:
gh workflow run Nix --repo jerudnik/jcode --ref nix-flake -f task=sync-upstream
gh workflow run Nix --repo jerudnik/jcode --ref nix-flake -f task=update-flake-lock
```

The lock update task invokes `.github/workflows/nix-update.yml` as a reusable
workflow, updates `flake.lock` on a temporary branch, validates the lock bump,
and opens or updates a PR against `nix-flake`.

## Use as a flake input

```nix
{
  inputs.jcode.url = "github:jerudnik/jcode/nix-flake";
  # Optionally pin nixpkgs to your own:
  inputs.jcode.inputs.nixpkgs.follows = "nixpkgs";
}
```

### Via the overlay (recommended)

```nix
nixpkgs.overlays = [ inputs.jcode.overlays.default ];
# ... then `pkgs.jcode` is available everywhere.
environment.systemPackages = [ pkgs.jcode ];
```

### Via the package directly

```nix
environment.systemPackages = [ inputs.jcode.packages.${pkgs.system}.default ];
```

## Home Manager module

```nix
{
  imports = [ inputs.jcode.homeManagerModules.default ];

  programs.jcode = {
    enable = true;

    # Optional: relocate jcode's state dir (sets JCODE_HOME).
    # Home-relative values such as ~/.local/state/jcode are supported when the
    # module also manages config.toml.
    # home = "~/.jcode";

    # Optional: declare $JCODE_HOME/config.toml (or ~/.jcode/config.toml when
    # `home` is unset) as Nix attrs.
    settings = {
      display.diff_mode = "inline";
      keybindings.scroll_up = "ctrl+k";
    };

    # ...or point at a pre-authored file instead of `settings`:
    # configFile = ./jcode-config.toml;
  };
}
```

`settings` and `configFile` are mutually exclusive; omit both to let jcode use
its own defaults.

## Contributing (dev shell)

```sh
nix develop
# provides the pinned Rust toolchain, cargo-nextest, cargo-audit, cargo-watch,
# nixfmt, and all native build inputs.
```

Run the same Nix gates CI runs:

```sh
nix build .#packages.x86_64-linux.jcode --dry-run --print-build-logs
nix flake check --accept-flake-config --no-build --all-systems --option eval-cores 1
nix fmt
```

Rust quality gates such as `cargo fmt`, clippy, tests, and simulator checks are
owned by the upstream `CI` workflow and its Rust toolchain. `cargo audit` runs in
Nix CI as a report-only advisory job so dependency advisories are visible without
blocking unrelated packaging changes.

## Cargo git dependencies

Every Cargo git source locked in `Cargo.lock` needs a fixed-output hash in
`nix/package.nix` under `outputHashes`. This version of Crane keys
`outputHashes` by the exact `Cargo.lock` source string, including the
`git+https://...?...#rev` form.

When adding or updating a Cargo git dependency, prefetch the locked revision and
copy the resulting hash into `nix/package.nix`:

```sh
nix run nixpkgs#nix-prefetch-git -- --url <repo.git> --rev <locked-rev>
```

Use the `sha256` value reported by the prefetch command. If the key does not
match the exact source string in `Cargo.lock`, Crane will not use the hash during
dependency vendoring.

## How the build stays reproducible

jcode's `build.rs` (in `jcode-build-meta`) normally shells out to `git` for
version metadata, which is unavailable in the Nix sandbox. The flake supplies
deterministic values via environment variables (`JCODE_BUILD_SEMVER`,
`JCODE_BUILD_GIT_HASH`, `JCODE_BUILD_GIT_DATE`, `JCODE_BUILD_GIT_DIRTY`) so
builds are pure and cacheable. The version is read from the root `Cargo.toml`.
