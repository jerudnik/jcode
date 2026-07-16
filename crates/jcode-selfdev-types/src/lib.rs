use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Environment variable that marks a child process as running in self-dev client
/// mode. Defined here (a low-level crate) so cross-cutting consumers (telemetry,
/// process title, server tester spawning) can reference it without depending on
/// the `cli` subsystem.
pub const CLIENT_SELFDEV_ENV: &str = "JCODE_CLIENT_SELFDEV_MODE";

/// Returns true if the current process was launched in self-dev client mode
/// (i.e. `CLIENT_SELFDEV_ENV` is set). Defined in this low-level crate so any
/// consumer can check self-dev mode without depending on the `cli` subsystem.
pub fn client_selfdev_requested() -> bool {
    std::env::var(CLIENT_SELFDEV_ENV).is_ok()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReloadRecoveryDirective {
    pub reconnect_notice: Option<String>,
    pub continuation_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelfDevBuildCommand {
    pub program: String,
    pub args: Vec<String>,
    pub display: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SelfDevBuildTarget {
    Auto,
    Tui,
    Desktop,
    All,
}

impl SelfDevBuildTarget {
    pub fn parse(value: Option<&str>) -> Result<Self> {
        match value.unwrap_or("auto").trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Self::Auto),
            "tui" | "jcode" => Ok(Self::Tui),
            "desktop" | "jcode-desktop" => Ok(Self::Desktop),
            "all" | "both" => Ok(Self::All),
            other => anyhow::bail!(
                "invalid selfdev build target `{}`; expected auto, tui, desktop, or all",
                other
            ),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BinaryVersionReport {
    pub version: Option<String>,
    pub git_hash: Option<String>,
}

/// Which binary to use.
#[derive(Debug, Clone)]
pub enum BinaryChoice {
    /// Use the stable version.
    Stable(String),
    /// Use the canary version for testing.
    Canary(String),
    /// Use current running binary because no versioned builds exist yet.
    Current,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceState {
    pub repo_scope: String,
    pub worktree_scope: String,
    pub short_hash: String,
    pub full_hash: String,
    pub dirty: bool,
    pub fingerprint: String,
    pub version_label: String,
    pub changed_paths: usize,
}

/// R01-owned canonical projection of the runtime identity that produced or is
/// executing a jcode binary.
///
/// This is distinct from the R03A `build_hash` compatibility token. Dirty builds
/// from the same commit can share a short hash while representing different
/// source states; the source fingerprint and version label are therefore part of
/// the canonical identity that reload/restart evidence must preserve.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeIdentityProjection {
    /// Immutable label for this source state, e.g. `<hash>` or
    /// `<hash>-dirty-<fingerprint-prefix>`.
    pub version_label: String,
    /// Stable fingerprint of the source state when known. Optional for older or
    /// release binaries that cannot reconstruct the build-time source tree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_fingerprint: Option<String>,
    /// Whether the source state was dirty when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_dirty: Option<bool>,
    /// Short source/build hash used for human correlation. This is not the R03A
    /// compatibility verdict by itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_hash: Option<String>,
    /// Full source/build hash when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_full_hash: Option<String>,
    /// Channel or mechanism that selected the executable, e.g. `selfdev`,
    /// `shared-server`, `generic-client`, or `tui-client`.
    pub activation_channel: String,
    /// Payload executable after wrapper/symlink resolution where possible.
    pub resolved_executable_payload: std::path::PathBuf,
}

impl SourceState {
    pub fn runtime_identity_projection(
        &self,
        activation_channel: impl Into<String>,
        resolved_executable_payload: impl Into<std::path::PathBuf>,
    ) -> RuntimeIdentityProjection {
        RuntimeIdentityProjection {
            version_label: self.version_label.clone(),
            source_fingerprint: Some(self.fingerprint.clone()),
            source_dirty: Some(self.dirty),
            source_hash: Some(self.short_hash.clone()),
            source_full_hash: Some(self.full_hash.clone()),
            activation_channel: activation_channel.into(),
            resolved_executable_payload: resolved_executable_payload.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublishedBuild {
    pub version: String,
    pub source_fingerprint: String,
    pub versioned_path: PathBuf,
    pub current_link: PathBuf,
    pub launcher_link: PathBuf,
    pub previous_current_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PendingActivation {
    pub session_id: String,
    pub new_version: String,
    pub previous_current_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_shared_server_version: Option<String>,
    pub source_fingerprint: Option<String>,
    pub requested_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DevBinarySourceMetadata {
    pub version_label: String,
    pub source_fingerprint: String,
    pub short_hash: String,
    pub full_hash: String,
    pub dirty: bool,
    pub changed_paths: usize,
}

impl From<&SourceState> for DevBinarySourceMetadata {
    fn from(source: &SourceState) -> Self {
        Self {
            version_label: source.version_label.clone(),
            source_fingerprint: source.fingerprint.clone(),
            short_hash: source.short_hash.clone(),
            full_hash: source.full_hash.clone(),
            dirty: source.dirty,
            changed_paths: source.changed_paths,
        }
    }
}

/// Status of a canary build being tested
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CanaryStatus {
    /// Build is currently being tested
    #[serde(alias = "Testing")]
    Testing,
    /// Build passed all tests and is ready for promotion
    #[serde(alias = "Passed")]
    Passed,
    /// Build failed testing
    #[serde(alias = "Failed")]
    Failed,
}

/// Information about a specific build version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildInfo {
    /// Git commit hash (short)
    pub hash: String,
    /// Git commit hash (full)
    pub full_hash: String,
    /// Build timestamp
    pub built_at: DateTime<Utc>,
    /// Git commit message (first line)
    pub commit_message: Option<String>,
    /// Whether build is from dirty working tree
    pub dirty: bool,
    /// Stable fingerprint of the source state used to produce the build.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_fingerprint: Option<String>,
    /// Immutable published version label, if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_label: Option<String>,
}

/// Information about a crash during canary testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashInfo {
    /// Build hash that crashed
    pub build_hash: String,
    /// Exit code
    pub exit_code: i32,
    /// Stderr output (truncated)
    pub stderr: String,
    /// Timestamp of crash
    pub crashed_at: DateTime<Utc>,
    /// Git diff that was being tested
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}

/// Context saved before migrating to a canary build
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationContext {
    pub session_id: String,
    pub from_version: String,
    pub to_version: String,
    pub change_summary: Option<String>,
    pub diff: Option<String>,
    pub timestamp: DateTime<Utc>,
}
