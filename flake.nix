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
              ./scripts/check_reusable_workflow_calls.py
              ./scripts/check_workflow_permissions.py
              ./scripts/dev_cargo.sh
              ./scripts/docs_impact_advisory.py
              ./scripts/preflight.sh
              ./scripts/prune_incremental.sh
              ./scripts/remote_build.sh
              ./scripts/remote_config.sh
              ./scripts/test_docs_impact_advisory.py
              ./scripts/test_fast.sh
              ./scripts/test_incremental_policy.sh
              ./scripts/governance_compare.py
              ./tests/test_reusable_workflow_calls.py
              ./tests/test_workflow_permissions.py
              ./tests/fixtures/actionlint-dollar-local
              ./tests/fixtures/workflow_permissions
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
              ./.github/workflows/ci.yml
              ./.github/workflows/docs-impact.yml
              ./.github/workflows/fork-ci.yml
              ./.github/workflows/fork-health.yml
              ./.github/workflows/main.yml
              ./.github/workflows/security.yml
              ./.github/workflows/nix.yml
              ./.github/workflows/nix-update.yml
              ./.github/workflows/pr.yml
              ./.github/workflows/release.yml
              ./.github/workflows/scheduled.yml
              ./.github/workflows/governance-root.yml
              # Not linted (sole upstream exemption), but ci.yml calls it, so
              # the reusable-call policy check needs it present.
              ./.github/workflows/freebsd-smoke.yml
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
              # The policy scan is opt-out (F30-FIX-1), so the derivation must
              # carry every doc it governs. Naming individual files here
              # reintroduced the same allowlist hole in the hermetic sandbox:
              # the check saw 13 documents and passed over the rest.
              ./docs
              ./.apm/instructions
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

          # Keep actionlint 1.7.12 aligned with GitHub.com's documented
          # permissions and same-repository reusable-workflow syntax. Patch its
          # parser and tables rather than filtering diagnostics so local
          # metadata checks, unknown scopes, and invalid access levels remain
          # fail closed.
          actionlint = pkgs.actionlint.overrideAttrs (old: {
            patches = (old.patches or [ ]) ++ [ ./nix/actionlint-dollar-local-workflows.patch ];
            postPatch = (old.postPatch or "") + ''
              substituteInPlace rule_permissions.go \
                --replace-fail \
                  $'\t"checks":              {"read", "write", "none"},' \
                  $'\t"checks":              {"read", "write", "none"},\n\t"code-quality":        {"read", "write", "none"},'
              substituteInPlace rule_permissions.go \
                --replace-fail \
                  $'\t"statuses":            {"read", "write", "none"},' \
                  $'\t"statuses":            {"read", "write", "none"},\n\t"vulnerability-alerts": {"read", "none"},'
            '';
          });
        in
        {
          _module.args.pkgs = pkgs;

          packages = {
            default = jcode;
            inherit actionlint jcode;

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
                  nativeBuildInputs = [ actionlint ];
                }
                ''
                  cd "$src"
                  actionlint \
                    .github/workflows/ci.yml \
                    .github/workflows/docs-impact.yml \
                    .github/workflows/fork-ci.yml \
                    .github/workflows/fork-health.yml \
                    .github/workflows/main.yml \
                    .github/workflows/security.yml \
                    .github/workflows/nix.yml \
                    .github/workflows/nix-update.yml \
                    .github/workflows/pr.yml \
                    .github/workflows/release.yml \
                    .github/workflows/scheduled.yml \
                    .github/workflows/governance-root.yml

                  ${pkgs.python3}/bin/python3 scripts/check_reusable_workflow_calls.py .
                  ${pkgs.python3}/bin/python3 tests/test_reusable_workflow_calls.py
                  ${pkgs.python3}/bin/python3 scripts/check_workflow_permissions.py .
                  ${pkgs.python3}/bin/python3 -m unittest tests.test_workflow_permissions

                  dollar_fixture="$TMPDIR/actionlint-dollar-local-valid"
                  cp -R "$src/tests/fixtures/actionlint-dollar-local" "$dollar_fixture"
                  chmod -R u+w "$dollar_fixture"
                  mkdir "$dollar_fixture/.git"
                  (
                    cd "$dollar_fixture"
                    actionlint .github/workflows/caller.yml .github/workflows/called.yaml
                  )
                  ${pkgs.python3}/bin/python3 scripts/check_reusable_workflow_calls.py "$dollar_fixture"
                  ${pkgs.python3}/bin/python3 scripts/check_workflow_permissions.py "$dollar_fixture"

                  fixture_dir="$TMPDIR/actionlint-permissions"
                  mkdir -p "$fixture_dir"
                  cat > "$fixture_dir/supported.yml" <<'EOF'
                  name: Supported permissions
                  on: push
                  permissions:
                    code-quality: write
                    vulnerability-alerts: read
                    models: read
                    repository-projects: write
                  jobs:
                    valid:
                      runs-on: ubuntu-latest
                      steps:
                        - run: 'true'
                  EOF
                  actionlint "$fixture_dir/supported.yml"

                  assert_rejected() {
                    fixture="$1"
                    expected="$2"
                    output="$fixture_dir/actionlint.out"
                    if actionlint "$fixture" > "$output" 2>&1; then
                      echo "actionlint unexpectedly accepted $fixture" >&2
                      exit 1
                    fi
                    grep -F "$expected" "$output"
                  }

                  dollar_negative="$TMPDIR/actionlint-dollar-local"
                  cp -R "$dollar_fixture" "$dollar_negative"
                  chmod -R u+w "$dollar_negative"

                  sed '/    with:/,+1d' \
                    "$dollar_fixture/.github/workflows/caller.yml" \
                    > "$dollar_negative/.github/workflows/missing-input.yml"
                  (
                    cd "$dollar_negative"
                    assert_rejected .github/workflows/missing-input.yml \
                      'input "required-input" is required by "$/.github/workflows/called.yaml" reusable workflow'
                  )

                  sed '/    secrets:/,+1d' \
                    "$dollar_fixture/.github/workflows/caller.yml" \
                    > "$dollar_negative/.github/workflows/missing-secret.yml"
                  (
                    cd "$dollar_negative"
                    assert_rejected .github/workflows/missing-secret.yml \
                      'secret "required-secret" is required by "$/.github/workflows/called.yaml" reusable workflow'
                  )

                  sed 's/needs.call.outputs.result/needs.call.outputs.unknown/' \
                    "$dollar_fixture/.github/workflows/caller.yml" \
                    > "$dollar_negative/.github/workflows/unknown-output.yml"
                  (
                    cd "$dollar_negative"
                    assert_rejected .github/workflows/unknown-output.yml \
                      'property "unknown" is not defined in object type'
                  )

                  sed 's|called.yaml|called.yaml@main|' \
                    "$dollar_fixture/.github/workflows/caller.yml" \
                    > "$dollar_negative/.github/workflows/local-ref.yml"
                  (
                    cd "$dollar_negative"
                    assert_rejected .github/workflows/local-ref.yml \
                      'reusable workflow call "$/.github/workflows/called.yaml@main"'
                  )

                  sed 's/code-quality: write/code-quality: admin/' \
                    "$fixture_dir/supported.yml" > "$fixture_dir/invalid-access.yml"
                  assert_rejected "$fixture_dir/invalid-access.yml" \
                    '"admin" is invalid as permission of scope "code-quality"'

                  sed 's/vulnerability-alerts: read/vulnerability-alerts: write/' \
                    "$fixture_dir/supported.yml" > "$fixture_dir/invalid-read-only-access.yml"
                  assert_rejected "$fixture_dir/invalid-read-only-access.yml" \
                    '"write" is invalid as permission of scope "vulnerability-alerts"'

                  sed 's/code-quality: write/future-scope: read/' \
                    "$fixture_dir/supported.yml" > "$fixture_dir/unknown-scope.yml"
                  assert_rejected "$fixture_dir/unknown-scope.yml" \
                    'unknown permission scope "future-scope"'
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
            provenance-sbom-fixtures =
              pkgs.runCommand "jcode-provenance-sbom-fixtures-check"
                {
                  sbom = jcode-sbom;
                  src = checkSrc;
                  nativeBuildInputs = [ pkgs.python3 ];
                  # The workspace version, so fixture facts track Cargo.toml
                  # instead of hard-coding a version that breaks on every bump.
                  jcodeVersion = version;
                }
                ''
                  cd "$src"
                  tmp=$(mktemp -d)
                  cp "$sbom/share/jcode/sbom.cdx.json" "$tmp/sbom.json"
                  python3 - <<'PY' "$tmp" "$jcodeVersion"
                  import hashlib
                  import json
                  import sys
                  from pathlib import Path

                  tmp = Path(sys.argv[1])
                  cargo_version = sys.argv[2]
                  sbom_bytes = (tmp / "sbom.json").read_bytes()
                  expected = {
                      "source_full_revision": "full",
                      "source_display_revision": "short",
                      "flake_lock_sha256": "lock",
                      "cargo_version": cargo_version,
                      "nix_system": "x86_64-linux",
                      "drv_path": "/nix/store/example.drv",
                      "output_path": "/nix/store/out-jcode",
                      "output_nar_hash": "sha256-abc",
                      "output_nar_size": 123,
                      "sbom_sha256": hashlib.sha256(sbom_bytes).hexdigest(),
                  }
                  provenance = {
                      "schema": "https://jerudnik.github.io/jcode/schemas/nix-provenance/v1",
                      "source": {"full_revision": "full", "display_revision": "short", "flake_lock_sha256": "lock"},
                      "version": {"cargo": cargo_version},
                      "nix": {"system": "x86_64-linux"},
                      "derivation": {"drv_path": "/nix/store/example.drv"},
                      "output": {"store_path": "/nix/store/out-jcode", "nar_hash": "sha256-abc", "nar_size": 123},
                      "sbom": {"sha256": expected["sbom_sha256"]},
                      "scope": {
                          "artifact": "packages.x86_64-linux.jcode",
                          "nix_system": "x86_64-linux",
                          "exclusions": [
                              "scripts/build_linux_compat.sh",
                              "compatibility bundle and Linux archive assets are excluded unless made reproducible separately",
                              "release assets",
                          ],
                      },
                      "release_assets": {"included": False},
                  }
                  (tmp / "expected.json").write_text(json.dumps(expected, sort_keys=True) + "\n")
                  (tmp / "provenance.json").write_text(json.dumps(provenance, sort_keys=True) + "\n")
                  bad_provenance = json.loads(json.dumps(provenance))
                  bad_provenance["source"]["full_revision"] = "wrong"
                  (tmp / "bad-provenance.json").write_text(json.dumps(bad_provenance, sort_keys=True) + "\n")
                  sbom = json.loads(sbom_bytes)
                  bad_sbom = json.loads(json.dumps(sbom))
                  git_index = next(
                      index
                      for index, component in enumerate(bad_sbom["components"])
                      if any(ref.get("type") == "vcs" for ref in component.get("externalReferences", []))
                  )
                  del bad_sbom["components"][git_index]
                  (tmp / "bad-sbom.json").write_text(json.dumps(bad_sbom, sort_keys=True) + "\n")
                  duplicate_sbom = json.loads(json.dumps(sbom))
                  duplicate_sbom["components"].append(json.loads(json.dumps(duplicate_sbom["components"][0])))
                  (tmp / "duplicate-sbom.json").write_text(json.dumps(duplicate_sbom, sort_keys=True) + "\n")
                  PY

                  python3 nix/verify-provenance-sbom.py --provenance "$tmp/provenance.json" --sbom "$tmp/sbom.json" --expected "$tmp/expected.json" --cargo-lock Cargo.lock
                  if python3 nix/verify-provenance-sbom.py --provenance "$tmp/bad-provenance.json" --sbom "$tmp/sbom.json" --expected "$tmp/expected.json" --cargo-lock Cargo.lock; then
                    echo "planted provenance mismatch passed unexpectedly" >&2
                    exit 1
                  fi
                  if python3 nix/verify-provenance-sbom.py --provenance "$tmp/provenance.json" --sbom "$tmp/bad-sbom.json" --expected "$tmp/expected.json" --cargo-lock Cargo.lock; then
                    echo "planted SBOM omission passed unexpectedly" >&2
                    exit 1
                  fi
                  if python3 nix/verify-provenance-sbom.py --provenance "$tmp/provenance.json" --sbom "$tmp/duplicate-sbom.json" --expected "$tmp/expected.json" --cargo-lock Cargo.lock; then
                    echo "planted duplicate bom-ref passed unexpectedly" >&2
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
              pkgs.gitleaks
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
