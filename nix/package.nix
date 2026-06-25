# Crane-based package definition for the `jcode` CLI/TUI binary.
#
# This is intentionally unopinionated: it builds the release `jcode` binary for
# the upstream-supported Nix platforms and exposes it as a plain derivation so
# downstream consumers can use it via the overlay, `packages.default`, an app,
# or a home-manager module without inheriting any personal configuration.
{
  lib,
  stdenv,
  craneLib,
  # Native/runtime inputs resolved by the caller (flake.nix) so this file stays
  # free of `pkgs` plumbing and is easy to reuse/override.
  pkg-config,
  cmake,
  perl,
  openssl,
  libiconv,
  # Build metadata. Defaults keep the sandboxed build reproducible because
  # jcode's build.rs shells out to `git` (unavailable in the Nix sandbox) and
  # otherwise auto-increments a dev patch counter.
  version,
  gitHash ? "nix",
  gitDate ? "1970-01-01T00:00:00+00:00",
  # When true, build.rs emits a clean `vX.Y.Z (hash)` version string instead of
  # the `-dev` developer suffix. Nix builds are reproducible and pinned, so they
  # behave like release builds by default.
  releaseBuild ? true,
}:
let
  # Only the workspace sources cargo actually needs. Keeping this tight avoids
  # rebuilds when docs/CI/nix files change.
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      (lib.fileset.maybeMissing ../rust-toolchain.toml)
      (lib.fileset.maybeMissing ../.cargo)
      ../src
      ../crates
      # Non-Rust build inputs referenced via include_bytes!/include_str! that
      # live at the workspace root (app icon, embedded fonts) and test fixtures
      # consumed by the `cargo test` check.
      ../assets
      (lib.fileset.maybeMissing ../tests)
    ];
  };

  commonArgs = {
    inherit src version;
    pname = "jcode";
    strictDeps = true;

    # Cargo git dependencies must be fetched as fixed-output Nix sources before
    # vendoring. Without these hashes, Crane can fall back to Cargo/git network
    # access while preparing dependencies, which is flaky on hosted macOS CI.
    cargoLock = "${src}/Cargo.lock";
    outputHashes = {
      "git+https://github.com/1jehuang/agentgrep.git?tag=v0.1.2#63e420bb4e035490d28cbca3f58e26baf297048e" =
        "sha256-Sf3EmWIZJ29KdaNbYRvM1tFXAPhOGhmpHOyqViEwkRI=";
      "git+https://github.com/1jehuang/agentgrep.git?tag=v0.1.3#1fc53630181548db7ad2c73ac8fb66969c04f854" =
        "sha256-vs8RK85sMa4WVupKU1V2oWxEVs1yHkEy7WNoTCNcMtE=";
      "git+https://github.com/1jehuang/mermaid-rs-renderer.git?tag=v0.2.1#01e8304ffc670f04dd4a047595cfb8ea9c854ae7" =
        "sha256-lQCloOhTqqEU8MNrkUmmJFdoOTEE3j5nvZJo21GJlMU=";
    };

    nativeBuildInputs = [
      pkg-config
      cmake
      perl # required by aws-lc-sys (rustls aws_lc_rs backend)
    ];

    # `openssl` (via openssl-sys) is required on Linux, where stdenv does not
    # provide it implicitly. Keeping it unconditional is harmless on Darwin and
    # avoids target-specific native dependency gaps. pkg-config (above) locates it.
    buildInputs = [ openssl ] ++ lib.optionals stdenv.hostPlatform.isDarwin [ libiconv ];

    # rustls uses aws-lc-rs; ensure the bundled C build is deterministic.
    CARGO_PROFILE = "release";
  };

  # Per-commit build metadata. This is deliberately kept OUT of `commonArgs` so
  # it does not flow into `buildDepsOnly`: the git hash/date change on every
  # commit, and including them in the dependency derivation would invalidate the
  # entire ~900-crate dependency cache on each commit (a full cold rebuild in CI
  # and locally). Only `jcode-build-meta` (a workspace crate compiled in the
  # final `buildPackage` step) reads these vars, so injecting them there alone is
  # correct and lets the expensive dependency layer stay cached across commits.
  buildMeta = {
    JCODE_BUILD_SEMVER = version;
    JCODE_BUILD_GIT_HASH = gitHash;
    JCODE_BUILD_GIT_DATE = gitDate;
    JCODE_BUILD_GIT_DIRTY = "false";
  }
  // lib.optionalAttrs releaseBuild {
    # Emit a clean release version string ("vX.Y.Z (hash)") rather than "-dev".
    JCODE_RELEASE_BUILD = "1";
  };

  # Build all workspace dependencies once; reused for the package and checks.
  # Note: `commonArgs` excludes the per-commit `buildMeta`, so this derivation's
  # hash is stable across commits and rebuilds only when actual dependencies,
  # toolchain, or native inputs change.
  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
in
craneLib.buildPackage (
  commonArgs
  // buildMeta
  // {
    inherit cargoArtifacts;

    # Only build the user-facing binary by default. Dev/bench bins stay behind
    # their feature gates and are not part of the public package.
    cargoExtraArgs = "--locked --bin jcode";

    # The full workspace test suite assumes a writable $HOME/.jcode, network,
    # and terminal; it is exercised in CI `checks`, not in the package build.
    doCheck = false;

    meta = {
      description = "Coding agent harness with a blazing-fast TUI, multi-model support, swarm coordination, and tool orchestration";
      homepage = "https://github.com/jerudnik/jcode";
      license = lib.licenses.mit;
      mainProgram = "jcode";
      platforms = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
    };
  }
)
