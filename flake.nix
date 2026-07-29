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
              ./flake.lock
              ./rust-toolchain.toml
              ./apm.yml
              ./apm.lock.yaml
              ./nix
              ./Cargo.toml
              ./Cargo.lock
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

          # Minimal source for the Cargo git outputHashes coherence check: the
          # lockfile that declares the git dependencies, the expression that
          # pins their fixed-output hashes, and the check itself.
          cargoGitHashSrc = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              ./Cargo.lock
              ./nix/package.nix
              ./tests/test_cargo_git_output_hashes.py
            ];
          };

          distributionPolicySrc = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              ./flake.nix
              ./Cargo.toml
              ./crates
              ./nix/package.nix
              ./scripts
              ./src
              ./tests
              ./web/jcode-mobile
              ./README.md
              ./RELEASING.md
              ./docs/BRANCHING.md
              ./docs/NIX.md
              ./docs/ONBOARDING_SANDBOX.md
              ./docs/WINDOWS.md
              ./docs/WRAPPERS.md
              ./.github/workflows
            ];
          };

          # The policy derivation uses a deliberately narrow source tree, so
          # check retired paths against the original flake checkout before the
          # sandbox is assembled. Otherwise a restored path omitted from
          # distributionPolicySrc would appear absent inside the derivation.
          retiredPathViolations = lib.concatStringsSep "\n" (
            lib.optional (builtins.pathExists ./ios) "ios"
            ++ lib.optional (builtins.pathExists ./docs/IOS_APP.md) "docs/IOS_APP.md"
            ++ lib.optional (builtins.pathExists ./.github/workflows/ios.yml) ".github/workflows/ios.yml"
            ++ lib.optional (builtins.pathExists ./.github/workflows/ios-testflight.yml) ".github/workflows/ios-testflight.yml"
            ++ lib.optional (builtins.pathExists ./scripts/phone-server/testflight-setup.py) "scripts/phone-server/testflight-setup.py"
          );

          jcode = pkgs.callPackage ./nix/package.nix {
            inherit craneLib version;
            # Stamp the binary with the flake's source revision when available
            # (a clean checkout). Dirty/path trees fall back to the package default.
            gitHash = inputs.self.shortRev or inputs.self.dirtyShortRev or "nix";
          };

          sourceFullRevision = inputs.self.rev or inputs.self.dirtyRev or "unknown";
          sourceDisplayRevision =
            inputs.self.shortRev or inputs.self.dirtyShortRev or (builtins.substring 0 12 sourceFullRevision);
          flakeLockSha256 = builtins.hashFile "sha256" ./flake.lock;

          jcode-sbom = pkgs.callPackage ./nix/sbom.nix {
            cargoLock = ./Cargo.lock;
            inherit version;
          };

          jcode-provenance = pkgs.callPackage ./nix/provenance.nix {
            inherit
              jcode
              version
              sourceFullRevision
              sourceDisplayRevision
              flakeLockSha256
              system
              ;
            sbom = jcode-sbom;
          };
        in
        {
          _module.args.pkgs = pkgs;

          packages = {
            default = jcode;
            inherit jcode;

            # The ~900-crate dependency layer, exposed so CI can publish it to
            # Cachix. `nix/package.nix` deliberately keeps per-commit build
            # metadata out of this derivation, so its hash is stable across
            # commits: publishing it means a normal commit only recompiles the
            # workspace crates instead of the whole dependency tree. Pushing
            # only the final `./result` leaves this layer unpublished and every
            # runner (and every fresh clone) rebuilds it from source.
            jcode-deps = jcode.cargoArtifacts;
          }
          // lib.optionalAttrs (system == "x86_64-linux") {
            inherit jcode-provenance jcode-sbom;
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

            nix-distribution-policy =
              pkgs.runCommand "jcode-nix-distribution-policy-check"
                {
                  src = distributionPolicySrc;
                  inherit retiredPathViolations;
                  nativeBuildInputs = [ pkgs.python3 ];
                }
                ''
                  if [ -n "$retiredPathViolations" ]; then
                    printf 'retired native-iOS path restored:\n%s\n' "$retiredPathViolations" >&2
                    exit 1
                  fi
                  cd "$src"
                  python3 tests/test_nix_distribution_policy.py
                  touch "$out"
                '';

            cargo-git-output-hashes =
              pkgs.runCommand "jcode-cargo-git-output-hashes-check"
                {
                  src = cargoGitHashSrc;
                  nativeBuildInputs = [ pkgs.python3 ];
                }
                ''
                  cd "$src"
                  python3 tests/test_cargo_git_output_hashes.py
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

          }
          // lib.optionalAttrs (system == "x86_64-linux") {
            provenance-sbom =
              pkgs.runCommand "jcode-provenance-sbom-check"
                {
                  src = checkSrc;
                  nativeBuildInputs = [
                    pkgs.nix
                    pkgs.python3
                  ];
                  inherit
                    version
                    sourceFullRevision
                    sourceDisplayRevision
                    flakeLockSha256
                    system
                    ;
                  jcodeDrvPath = builtins.unsafeDiscardStringContext jcode.drvPath;
                  jcodeOutPath = builtins.unsafeDiscardStringContext jcode.outPath;
                }
                ''
                  cd "$src"
                  nar_hash=$(nix --extra-experimental-features nix-command hash path --sri ${jcode})
                  nar_size=$(nix-store --dump ${jcode} | wc -c | tr -d ' ')
                  sbom_sha256=$(sha256sum ${jcode-sbom}/share/jcode/sbom.cdx.json | cut -d' ' -f1)

                  cat > expected.json <<EOF
                  {
                    "source_full_revision": "$sourceFullRevision",
                    "source_display_revision": "$sourceDisplayRevision",
                    "flake_lock_sha256": "$flakeLockSha256",
                    "cargo_version": "$version",
                    "nix_system": "$system",
                    "drv_path": "$jcodeDrvPath",
                    "output_path": "$jcodeOutPath",
                    "output_nar_hash": "$nar_hash",
                    "output_nar_size": $nar_size,
                    "sbom_sha256": "$sbom_sha256"
                  }
                  EOF

                  python3 nix/verify-provenance-sbom.py \
                    --provenance ${jcode-provenance}/share/jcode/provenance.json \
                    --sbom ${jcode-sbom}/share/jcode/sbom.cdx.json \
                    --expected expected.json \
                    --cargo-lock Cargo.lock

                  cp ${jcode-provenance}/share/jcode/provenance.json planted-provenance.json
                  python3 - <<'PY'
                  import json
                  from pathlib import Path
                  path = Path("planted-provenance.json")
                  data = json.loads(path.read_text())
                  data["source"]["full_revision"] = "planted-wrong-revision"
                  path.write_text(json.dumps(data, sort_keys=True) + "\n")
                  PY
                  if python3 nix/verify-provenance-sbom.py \
                    --provenance planted-provenance.json \
                    --sbom ${jcode-sbom}/share/jcode/sbom.cdx.json \
                    --expected expected.json \
                    --cargo-lock Cargo.lock; then
                    echo "planted provenance mismatch passed" >&2
                    exit 1
                  fi

                  cp ${jcode-sbom}/share/jcode/sbom.cdx.json planted-sbom.cdx.json
                  python3 - <<'PY'
                  import json
                  from pathlib import Path
                  path = Path("planted-sbom.cdx.json")
                  data = json.loads(path.read_text())
                  for component in data["components"]:
                      if component.get("name") == "agentgrep":
                          component.pop("externalReferences", None)
                          break
                  path.write_text(json.dumps(data, sort_keys=True) + "\n")
                  PY
                  if python3 nix/verify-provenance-sbom.py \
                    --provenance ${jcode-provenance}/share/jcode/provenance.json \
                    --sbom planted-sbom.cdx.json \
                    --expected expected.json \
                    --cargo-lock Cargo.lock; then
                    echo "planted SBOM mismatch passed" >&2
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
