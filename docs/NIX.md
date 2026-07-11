# Using jcode with Nix

The `distro/nix` and `main` branches ship a
[Nix flake](https://nixos.wiki/wiki/Flakes) so jcode can be built, run,
installed reproducibly, and reused as a flake input by downstream
configurations. The packaging layer exposes the package, an overlay, an app, a
dev shell, CI checks, and a Home Manager module.

## Branches

Use one of the branch-specific flake URLs:

```sh
github:jerudnik/jcode/distro/nix # packaging layer only
github:jerudnik/jcode/main       # stable custom fork
```

See [BRANCHING.md](BRANCHING.md) for the full downstream-only maintenance
procedure.

## Supported platforms

`x86_64-linux`, `aarch64-linux`, and `aarch64-darwin`.

## Quick start

Run without installing:

```sh
nix run github:jerudnik/jcode/main
```

Build the binary:

```sh
nix build github:jerudnik/jcode/main
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
from `distro/nix` and `main` once the `CACHIX_AUTH_TOKEN` repo secret is set.
Pull requests consume the cache read-only and do not push. To set up or re-key:

1. The cache is `jerudnik-jcode` on Cachix.
2. Generate a push auth token in the Cachix UI and store it as the
   `CACHIX_AUTH_TOKEN` GitHub Actions secret.
3. If the signing key is ever rotated, update the public key in `flake.nix` (the
   `nixConfig` block) and in this doc.

### Automated maintenance

Each maintenance concern has its own workflow (schedules run from `main`, the
default branch):

- **Upstream sync** (`sync.yml`) runs every six hours. It fast-forwards
  `vendor/upstream` to `upstream/master`, rebases `distro/nix`, rebases
  `main`, pushes with `--force-with-lease`, verifies rail health, and
  dispatches Fork CI + Nix on the result. Recurring conflicts self-heal via
  the shared rerere cache; a new conflict opens a `sync-blocked` issue.
- **Fork health** (`fork-health.yml`) runs daily and enforces the
  three-branch invariants via `scripts/fork-health.sh`.
- **flake.lock updates** (`nix-update.yml`) run weekly on Monday at 06:47
  UTC, after the 06:17 UTC sync window, and open a PR against `distro/nix`.

To trigger any of them manually:

```sh
gh workflow run "Upstream Sync" --repo jerudnik/jcode
gh workflow run "Fork Health" --repo jerudnik/jcode
gh workflow run "Update flake.lock" --repo jerudnik/jcode
```

## Use as a flake input

```nix
{
  inputs.jcode.url = "github:jerudnik/jcode/main";
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
    # module also manages config.nix.toml.
    # home = "~/.jcode";

    # Optional: declare $JCODE_HOME/config.nix.toml (or
    # ~/.jcode/config.nix.toml when `home` is unset) as pinned policy attrs.
    settings = {
      display.diff_mode = "inline";
      keybindings.scroll_up = "ctrl+k";
    };

    # ...or point at a pre-authored file instead of `settings`:
    # configFile = ./jcode-policy.toml;

    # Escape hatch: restore the old behavior and manage config.toml directly.
    # This can break jcode runtime saves because Home Manager files are normally
    # read-only Nix store symlinks, so prefer the default policy layer.
    # manageConfigToml = true;
  };
}
```

`settings` and `configFile` are mutually exclusive; omit both to let jcode use
its own defaults. By default, both options write `config.nix.toml`, a read-only
policy layer. jcode keeps the mutable durable `config.toml` for runtime choices
such as trust decisions and UI preferences. Keys declared in `config.nix.toml`
are pinned: runtime save attempts for those keys are ignored with a warning.

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

### Self-development loop

jcode's "self-dev" is a save-triggered incremental rebuild + re-exec — **plain
`cargo`, not Nix**. Crane is correct for the *installed* binary (hermetic,
Cachix-cached), but its two-phase derivation can't feed a running cargo daemon's
incremental state; every edit would re-evaluate the source derivation. So the
loop runs on `cargo` inside this devShell, which already inherits the crane
package's build inputs (openssl, apple-sdk) via `inherit (self') checks;` — you
get the *same* toolchain as the installed binary, just incremental.

```sh
nix develop        # this shell: pinned toolchain + build inputs + cargo-watch/nextest/rust-analyzer
just dev-check     # type-check + test on save (fast feedback)
just dev-build     # incremental `build --bin jcode` on save
just dev-selfdev   # same, with the [profile.selfdev] profile → target/selfdev/jcode
```

On Linux the shell links with `mold` (`RUSTFLAGS=-C link-arg=-fuse-ld=mold`),
scoped to the devShell so it never touches the hermetic crane build. Incremental
state lives in the worktree `target/` (survives shell exits). Under Nix
management the installed `jcode` is the Nix build; a self-dev build hot-reloads
the *running* process but does not replace the installed binary (see
`JCODE_NIX_MANAGED`), so `jcode` stays predictable.

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
