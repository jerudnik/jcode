# Using jcode with Nix

jcode ships a [Nix flake](https://nixos.wiki/wiki/Flakes) so it can be built,
run, and installed reproducibly, and reused as a flake input by downstream
configurations. The flake is intentionally unopinionated: it exposes the
package, an overlay, an app, a dev shell, CI checks, and a Home Manager module,
but bakes in no personal configuration.

## Supported platforms

`x86_64-linux`, `aarch64-linux`, and `aarch64-darwin`.

## Quick start

Run without installing:

```sh
nix run github:jerudnik/jcode
```

Build the binary:

```sh
nix build github:jerudnik/jcode
./result/bin/jcode --version
```

## Binary cache (skip building from source)

A public Cachix cache (`jerudnik-jcode`) serves prebuilt outputs. Add it once
and Nix will pull binaries instead of compiling the ~60-crate workspace:

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

The cache only ever stores `nix build` outputs (signed with a key whose private
half lives solely in CI secrets). It is safe to expose publicly and safe for
others to consume.

### Maintaining the cache

CI (`.github/workflows/nix.yml`) pushes successful build outputs automatically
once the `CACHIX_AUTH_TOKEN` repo secret is set. To set up or re-key:

1. The cache is `jerudnik-jcode` on Cachix.
2. Generate a push auth token in the Cachix UI and store it as the
   `CACHIX_AUTH_TOKEN` GitHub Actions secret.
3. If the signing key is ever rotated, update the public key in `flake.nix`
   (the `nixConfig` block) and in this doc.

## Use as a flake input

```nix
{
  inputs.jcode.url = "github:jerudnik/jcode";
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
    # home = "~/.jcode";

    # Optional: declare ~/.jcode/config.toml as Nix attrs.
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

Run the same gates CI runs:

```sh
nix flake check          # build + clippy + rustfmt + cargo-audit
nix fmt                  # format Nix files
```

## How the build stays reproducible

jcode's `build.rs` (in `jcode-build-meta`) normally shells out to `git` for
version metadata, which is unavailable in the Nix sandbox. The flake supplies
deterministic values via environment variables (`JCODE_BUILD_SEMVER`,
`JCODE_BUILD_GIT_HASH`, `JCODE_BUILD_GIT_DATE`, `JCODE_BUILD_GIT_DIRTY`) so
builds are pure and cacheable. The version is read from the root `Cargo.toml`.
