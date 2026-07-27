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

          checkSrc = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              ./flake.nix
              ./rust-toolchain.toml
              ./apm.yml
              ./apm.lock.yaml
              ./nix
              ./Cargo.toml
              ./scripts/clean_target.sh
              ./scripts/check_agent_instructions.py
              ./scripts/dev_cargo.sh
              ./scripts/docs_impact_advisory.py
              ./scripts/preflight.sh
              ./scripts/prune_incremental.sh
              ./scripts/remote_build.sh
              ./scripts/remote_config.sh
              ./scripts/test_docs_impact_advisory.py
              ./scripts/test_fast.sh
              ./scripts/test_incremental_policy.sh
              ./.apm/instructions
              ./.jcode/preferred-tools.md
              ./.jcode/prompt-overlay.md
              ./.jcode/swarm-prompt.md
              ./docs/agent-workflows.md
              ./docs/BRANCHING.md
              ./docs/NIX.md
              ./docs/SWARM_ARCHITECTURE.md
              ./docs/SWARM_TASK_GRAPH.md
              ./CONTRIBUTING.md
              ./RELEASING.md
              ./.github/workflows/docs-impact.yml
              ./.github/workflows/fork-ci.yml
              ./.github/workflows/fork-health.yml
              ./.github/workflows/security.yml
              ./.github/workflows/nix.yml
              ./.github/workflows/nix-update.yml
              ./.github/workflows/release.yml
            ];
          };

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

          # CI gates run by `nix flake check`. Keep these cheap, local, and valid
          # on every flake system: Rust clippy/fmt/tests and the package build are
          # already covered by fork-ci.yml / nix.yml, while security auditing is a
          # separate non-blocking workflow. These checks instead validate the Nix
          # surface, local preflight entry point, fork-owned workflows, and pinned
          # Rust-toolchain contract without network access or another full build.
          checks = {
            nix-format =
              pkgs.runCommand "jcode-nix-format-check"
                {
                  src = checkSrc;
                  nativeBuildInputs = [ pkgs.nixfmt ];
                }
                ''
                  cd "$src"
                  nixfmt --check flake.nix nix/*.nix nix/modules/*.nix
                  touch "$out"
                '';

            preflight-shell =
              pkgs.runCommand "jcode-preflight-shell-check"
                {
                  src = checkSrc;
                  nativeBuildInputs = [ pkgs.shellcheck ];
                }
                ''
                  cd "$src"
                  bash -n \
                    scripts/clean_target.sh \
                    scripts/dev_cargo.sh \
                    scripts/preflight.sh \
                    scripts/prune_incremental.sh \
                    scripts/remote_build.sh \
                    scripts/remote_config.sh \
                    scripts/test_incremental_policy.sh
                  shellcheck -x -e SC2016 \
                    scripts/clean_target.sh \
                    scripts/dev_cargo.sh \
                    scripts/preflight.sh \
                    scripts/prune_incremental.sh \
                    scripts/remote_build.sh \
                    scripts/remote_config.sh \
                    scripts/test_incremental_policy.sh
                  touch "$out"
                '';

            agent-instructions =
              pkgs.runCommand "jcode-agent-instructions-check"
                {
                  src = checkSrc;
                  nativeBuildInputs = [ pkgs.python3 ];
                }
                ''
                  cd "$src"
                  python3 scripts/check_agent_instructions.py
                  touch "$out"
                '';

            docs-impact-advisory =
              pkgs.runCommand "jcode-docs-impact-advisory-check"
                {
                  src = checkSrc;
                  nativeBuildInputs = [
                    pkgs.git
                    pkgs.python3
                  ];
                }
                ''
                  cd "$src"
                  python3 scripts/test_docs_impact_advisory.py
                  touch "$out"
                '';

            incremental-policy =
              pkgs.runCommand "jcode-incremental-policy-check"
                {
                  src = checkSrc;
                  nativeBuildInputs = [
                    pkgs.git
                    pkgs.lsof
                    pkgs.procps
                    pkgs.stdenv.cc
                  ];
                }
                ''
                  cd "$src"
                  bash scripts/test_incremental_policy.sh
                  touch "$out"
                '';

            workflow-syntax =
              pkgs.runCommand "jcode-workflow-syntax-check"
                {
                  src = checkSrc;
                  nativeBuildInputs = [ pkgs.actionlint ];
                }
                ''
                  cd "$src"
                  actionlint \
                    .github/workflows/docs-impact.yml \
                    .github/workflows/fork-ci.yml \
                    .github/workflows/fork-health.yml \
                    .github/workflows/security.yml \
                    .github/workflows/nix.yml \
                    .github/workflows/nix-update.yml \
                    .github/workflows/release.yml
                  touch "$out"
                '';

            rust-toolchain-coherence =
              pkgs.runCommand "jcode-rust-toolchain-coherence-check"
                {
                  src = checkSrc;
                }
                ''
                  cd "$src"
                  expected="${rustVersion}"
                  toolchain_version=$(sed -n 's/^channel = "\([^"]*\)"$/\1/p' rust-toolchain.toml)
                  flake_version=$(sed -n 's/^[[:space:]]*rustVersion = "\([^"]*\)";$/\1/p' flake.nix)

                  if [ "$toolchain_version" != "$expected" ]; then
                    echo "rust-toolchain.toml pins '$toolchain_version'; expected '$expected'" >&2
                    exit 1
                  fi

                  if [ "$flake_version" != "$expected" ]; then
                    echo "flake.nix pins '$flake_version'; expected '$expected'" >&2
                    exit 1
                  fi

                  touch "$out"
                '';
          };

          devShells.default = craneLib.devShell {
            inherit (self') checks;
            packages = [
              pkgs.cargo-nextest
              pkgs.cargo-audit
              pkgs.cargo-watch
              pkgs.rust-analyzer
              pkgs.nixfmt
              pkgs.python3
              pkgs.ripgrep
              pkgs.actionlint
              pkgs.git
              pkgs.just
              pkgs.jq
              pkgs.shellcheck
              pkgs.curl
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
              # Install a local pre-push guard that refuses to recreate the
              # rails this hard fork retired. It is idempotent and leaves
              # user-owned hooks untouched.
              if [ -x scripts/install-git-hooks.sh ]; then
                scripts/install-git-hooks.sh || true
              fi
            '';
          };

          formatter = pkgs.nixfmt;
        };
    };
}
