//! Compile-time build/version metadata for jcode.
//!
//! The build script (`build.rs`) computes git- and version-derived values and
//! emits them via `cargo:rustc-env`. This module re-exposes them as `pub const`
//! so any workspace crate can read identical values through e.g.
//! `jcode_build_meta::VERSION` instead of `env!("JCODE_VERSION")`.

/// Human-readable version string, e.g. `v0.14.6-dev (abc1234)`.
pub const VERSION: &str = env!("JCODE_VERSION");
/// Short git hash of the build commit, e.g. `abc1234` (or `unknown`).
pub const GIT_HASH: &str = env!("JCODE_GIT_HASH");
/// Commit date/time of the build commit (or `unknown`).
pub const GIT_DATE: &str = env!("JCODE_GIT_DATE");
/// `git describe --tags --always` output (may be empty).
pub const GIT_TAG: &str = env!("JCODE_GIT_TAG");
/// Auto-incrementing build semver (dev) or explicit release semver.
pub const SEMVER: &str = env!("JCODE_SEMVER");
/// Base semver taken from the root `Cargo.toml` package version.
pub const BASE_SEMVER: &str = env!("JCODE_BASE_SEMVER");
/// Semver used for update comparisons.
pub const UPDATE_SEMVER: &str = env!("JCODE_UPDATE_SEMVER");
/// Encoded changelog (record/unit separated). See build.rs for the format.
pub const CHANGELOG: &str = env!("JCODE_CHANGELOG");
/// Root crate package version (mirrors the historical `CARGO_PKG_VERSION`).
pub const PKG_VERSION: &str = env!("JCODE_PKG_VERSION");
/// Filesystem path of the source checkout this binary was built from, e.g.
/// `/home/u/infrastructure/jcode` (or `unknown`). This answers the daemon
/// divergence question "which checkout produced the running binary" (G4 in
/// `docs/architecture/SELFDEV_NIX_DAEMON_DIVERGENCE.md`). For immutable Nix/store
/// builds it is the build-sandbox path and not meaningful as a live checkout;
/// `jcode doctor` only surfaces it for source/selfdev origins.
pub const BUILD_SOURCE_DIR: &str = env!("JCODE_BUILD_SOURCE_DIR");

/// Whether this binary was built as a release build (`JCODE_RELEASE_BUILD=1`).
pub const fn is_release_build() -> bool {
    option_env!("JCODE_RELEASE_BUILD").is_some()
}

/// Cargo's optimization level for this build (`"0"`, `"1"`, `"2"`, `"3"`,
/// `"s"`, `"z"`, or `"unknown"`).
///
/// Cargo exposes `OPT_LEVEL` to build scripts only, so it is forwarded here for
/// crate code that needs it.
pub const OPT_LEVEL: &str = env!("JCODE_OPT_LEVEL");

/// Whether the compiler actually optimized this build.
///
/// `cfg!(debug_assertions)` is commonly used as a proxy for this and is wrong
/// for at least one profile in this workspace: `selfdev` inherits `release`, so
/// assertions are compiled out, while pinning `opt-level = 0`. Wall-clock
/// performance assertions must key off this instead, or they impose optimized
/// timings on unoptimized binaries.
pub const fn is_optimized_build() -> bool {
    // `str` comparison is not const-stable, so compare the raw bytes.
    matches!(OPT_LEVEL.as_bytes(), b"1" | b"2" | b"3" | b"s" | b"z")
}

#[cfg(test)]
mod opt_level_tests {
    /// The forwarded opt-level must reflect the profile actually in use, or the
    /// performance assertions keyed off it silently pick the wrong budget.
    #[test]
    fn opt_level_is_forwarded_and_classified() {
        let lvl = super::OPT_LEVEL;
        assert_ne!(lvl, "unknown", "build script failed to forward OPT_LEVEL");
        assert_eq!(
            super::is_optimized_build(),
            lvl != "0",
            "opt-level {lvl} classified incorrectly"
        );
        println!("OPT_LEVEL={lvl} optimized={}", super::is_optimized_build());
    }
}
