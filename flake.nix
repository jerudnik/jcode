{
  description = "jcode - a blazing-fast TUI/CLI coding agent harness (multi-model, swarm coordination, tool orchestration)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    flake-parts.url = "github:hercules-ci/flake-parts";

    crane.url = "github:ipetkov/crane";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  # Public, safe-to-share binary cache for prebuilt outputs.
  #
  # Only `nix build` outputs land here, signed with a key whose private half
  # lives solely in CI secrets / Cachix. It is safe to expose publicly and safe
  # for others to consume. Consumers opt in with `--accept-flake-config` or by
  # adding the substituter to their own nix config (see docs/NIX.md).
  nixConfig = {
    extra-substituters = [ "https://jerudnik-jcode.cachix.org" ];
    extra-trusted-public-keys = [
      "jerudnik-jcode.cachix.org-1:WL5DX0TS/0N/BIW6RDnFGKpkZX9eT2DwFJK+05cpIZQ="
    ];
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];

      flake = {
        # Overlay: the single most useful thing for downstream reuse. Consumers
        # add `inputs.jcode.overlays.default` and get `pkgs.jcode`.
        overlays.default = final: _prev: {
          jcode = inputs.self.packages.${final.stdenv.hostPlatform.system}.jcode;
        };

        # Home Manager module. Use as
        #   imports = [ inputs.jcode.homeManagerModules.default ];
        #   programs.jcode.enable = true;
        homeManagerModules.default = import ./nix/modules/home-manager.nix;
        homeModules.default = import ./nix/modules/home-manager.nix; # HM >= 24.11 alias
      };

      perSystem =
        {
          self',
          system,
          ...
        }:
        let
          pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ (import inputs.rust-overlay) ];
          };
          inherit (pkgs) lib;

          # Keep the Nix package/devShell aligned with rust-toolchain.toml and
          # the blocking GitHub Actions gates. Fork CI validates these pins.
          rustVersion = "1.96.0";
          rustToolchain = pkgs.rust-bin.stable.${rustVersion}.default.override {
            extensions = [
              "rust-src"
              "clippy"
              "rustfmt"
            ];
          };

          craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rustToolchain;

          # Version is the single source of truth from the root Cargo.toml.
          inherit ((craneLib.crateNameFromCargoToml { src = ./.; })) version;

          jcode = pkgs.callPackage ./nix/package.nix {
            inherit craneLib version;
            # Stamp the binary with the flake's source revision when available
            # (a clean checkout). Dirty/path trees fall back to the package default.
            gitHash = inputs.self.shortRev or inputs.self.dirtyShortRev or "nix";
          };
        in
        {
          _module.args.pkgs = pkgs;

          packages = {
            default = jcode;
            inherit jcode;
          };

          # CI gates run by `nix flake check`. We intentionally do NOT duplicate
          # clippy/rustfmt/test here: those are owned by the upstream `ci.yml`
          # against its pinned `stable` toolchain. Re-running them through the
          # flake's rust-overlay toolchain only produces spurious version-skew
          # failures. Dependency security auditing runs as a separate,
          # non-blocking CI job. Package builds are covered by the workflow's
          # trusted push/dispatch matrix; PR validation uses
          # `nix flake check --no-build --all-systems` to evaluate every public
          # flake surface without duplicating full package builds.
          checks = { };

          devShells.default = craneLib.devShell {
            inherit (self') checks;
            packages = [
              pkgs.cargo-nextest
              pkgs.cargo-audit
              pkgs.cargo-watch
              pkgs.rust-analyzer
              pkgs.nixfmt
              pkgs.pkg-config
              pkgs.cmake
              pkgs.perl
            ]
            ++ lib.optionals pkgs.stdenv.hostPlatform.isDarwin [ pkgs.libiconv ]
            # mold cuts link time 5-10x on the ~720-dep workspace (Linux only;
            # no-op on Darwin, whose linker mold does not target).
            ++ lib.optionals pkgs.stdenv.hostPlatform.isLinux [ pkgs.mold ];

            JCODE_BUILD_SEMVER = version;
            # Use mold as the linker for the self-dev loop on Linux. Scoped to
            # the devShell so it never affects the hermetic crane build.
            RUSTFLAGS = lib.optionalString pkgs.stdenv.hostPlatform.isLinux "-C link-arg=-fuse-ld=mold";
            shellHook = ''
              echo "jcode dev shell — rust $(rustc --version 2>/dev/null || echo '?')"
              # Enable git rerere for this clone and import shared recorded
              # conflict resolutions so local rebases self-heal like CI does.
              if [ -x scripts/rerere-cache.sh ]; then
                scripts/rerere-cache.sh setup || true
              fi
              # Install a local pre-push guard that blocks accidental writes to
              # distro/nix/vendor rails. It is idempotent and leaves user-owned
              # hooks untouched.
              if [ -x scripts/install-git-hooks.sh ]; then
                scripts/install-git-hooks.sh || true
              fi
              # Non-blocking fork-drift nudge: reads cached refs, never blocks on
              # the network, auto fast-forwards only the unambiguously safe case.
              # Disable with FORK_NUDGE_DISABLE=1.
              if [ "''${FORK_NUDGE_DISABLE:-0}" != "1" ] && [ -x scripts/fork-nudge.sh ]; then
                scripts/fork-nudge.sh || true
              fi
            '';
          };

          formatter = pkgs.nixfmt;
        };
    };
}
