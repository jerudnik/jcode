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

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  # Public, safe-to-share binary cache for prebuilt outputs.
  #
  # This is intentionally commented out until the cache actually exists, so a
  # fresh clone never points consumers at a dead substituter or an unverifiable
  # key. To enable: create the `jerudnik-jcode` Cachix cache, then uncomment
  # and paste the real public key (see docs/NIX.md). Consumers opt in with
  # `--accept-flake-config` or by adding the substituter to their own config.
  #
  # nixConfig = {
  #   extra-substituters = [ "https://jerudnik-jcode.cachix.org" ];
  #   extra-trusted-public-keys = [
  #     "jerudnik-jcode.cachix.org-1:<PUBLIC_KEY>="
  #   ];
  # };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
      ];

      flake = {
        # Overlay: the single most useful thing for downstream reuse. Consumers
        # add `inputs.jcode.overlays.default` and get `pkgs.jcode`.
        overlays.default = final: prev: {
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

          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            extensions = [
              "rust-src"
              "clippy"
              "rustfmt"
            ];
          };

          craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rustToolchain;

          # Version is the single source of truth from the root Cargo.toml.
          version = (craneLib.crateNameFromCargoToml { src = ./.; }).version;

          jcode = pkgs.callPackage ./nix/package.nix {
            inherit craneLib version;
            # Stamp the binary with the flake's source revision when available
            # (a clean checkout). Dirty/path trees fall back to the package default.
            gitHash = inputs.self.shortRev or inputs.self.dirtyShortRev or "nix";
          };

          # Shared crane args for the `checks` so they reuse the dep build.
          checkSrc = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              ./src
              ./crates
            ];
          };
          commonCheckArgs = {
            src = checkSrc;
            pname = "jcode";
            inherit version;
            strictDeps = true;
            nativeBuildInputs = [
              pkgs.pkg-config
              pkgs.cmake
              pkgs.perl
            ];
            buildInputs = lib.optionals pkgs.stdenv.hostPlatform.isDarwin [ pkgs.libiconv ];
            JCODE_BUILD_SEMVER = version;
            JCODE_BUILD_GIT_HASH = "nix";
            JCODE_BUILD_GIT_DATE = "1970-01-01T00:00:00+00:00";
            JCODE_BUILD_GIT_DIRTY = "false";
          };
          cargoArtifacts = craneLib.buildDepsOnly commonCheckArgs;
        in
        {
          _module.args.pkgs = pkgs;

          packages = {
            default = jcode;
            inherit jcode;
          };

          apps.default = {
            type = "app";
            program = lib.getExe jcode;
          };

          # CI gates. Built with `nix flake check`; reuse cached dep artifacts.
          checks = {
            inherit jcode;

            jcode-clippy = craneLib.cargoClippy (
              commonCheckArgs
              // {
                inherit cargoArtifacts;
                cargoClippyExtraArgs = "--all-targets -- --deny warnings";
              }
            );

            jcode-fmt = craneLib.cargoFmt {
              inherit (commonCheckArgs) src pname;
            };

            jcode-audit = craneLib.cargoAudit {
              inherit (commonCheckArgs) src;
              inherit (inputs) advisory-db;
            };
          };

          devShells.default = craneLib.devShell {
            checks = self'.checks;
            packages = [
              pkgs.cargo-nextest
              pkgs.cargo-audit
              pkgs.cargo-watch
              pkgs.nixfmt-rfc-style
              pkgs.pkg-config
              pkgs.cmake
              pkgs.perl
            ]
            ++ lib.optionals pkgs.stdenv.hostPlatform.isDarwin [ pkgs.libiconv ];

            JCODE_BUILD_SEMVER = version;
            shellHook = ''
              echo "jcode dev shell — rust $(rustc --version 2>/dev/null || echo '?')"
            '';
          };

          formatter = pkgs.nixfmt-rfc-style;
        };
    };
}
