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

          # CI gates run by `nix flake check`. We intentionally do NOT duplicate
          # clippy/rustfmt/test here: those are owned by the upstream `ci.yml`
          # against its pinned `stable` toolchain. Re-running them through the
          # flake's rust-overlay toolchain only produces spurious version-skew
          # failures. Dependency security auditing runs as a separate,
          # non-blocking CI job (advisories in transitive deps should surface
          # without permanently reddening every build). The flake's job here is
          # to prove the package *builds* reproducibly under Nix.
          checks = {
            # Reuses the package derivation: `nix flake check` fails if the
            # workspace does not build reproducibly under Nix.
            inherit jcode;
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
