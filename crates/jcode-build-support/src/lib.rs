mod paths;
mod platform_support;
mod source_state;
mod storage_helpers;

pub use paths::{
    RetiredLayoutResidue, SELFDEV_CARGO_PROFILE, binary_name, binary_stem, client_update_candidate,
    current_binary_build_time_string, current_binary_built_at, current_fixed_binary_path,
    current_fixed_dir, find_dev_binary, find_repo_in_ancestors, get_repo_dir,
    is_externally_managed, is_jcode_repo, launcher_binary_path, launcher_dir,
    nix_managed_fallback_binary, preferred_reload_candidate, release_binary_path,
    resolve_binary_payload, retired_layout_dir, retired_layout_residue, run_selfdev_build,
    selfdev_binary_path, selfdev_build_command, selfdev_build_command_for_target,
    shared_server_update_candidate, update_launcher_symlink_to_current,
};
pub use source_state::{
    current_build_info, current_git_diff, current_git_hash, current_git_hash_full,
    current_source_state, ensure_source_state_matches, get_commit_message, is_working_tree_dirty,
    repo_build_version, repo_scope_key, worktree_scope_key,
};
pub use storage_helpers::{
    build_log_path, build_progress_path, clear_build_progress, manifest_path, read_build_progress,
    write_build_progress,
};

use anyhow::{Context, Result};
use jcode_storage as storage;
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(unix)]
use std::time::{Duration, Instant};

pub use jcode_selfdev_types::{
    BinaryVersionReport, BuildInfo, DevBinarySourceMetadata, PublishedBuild,
    RuntimeIdentityProjection, SelfDevBuildCommand, SelfDevBuildTarget, SourceState,
};

fn metadata_version_label(metadata: &DevBinarySourceMetadata) -> String {
    if metadata.dirty {
        let prefix: String = metadata.source_fingerprint.chars().take(12).collect();
        format!("{}-dirty-{}", metadata.short_hash, prefix)
    } else {
        metadata.short_hash.clone()
    }
}

fn runtime_identity_projection_from_metadata(
    metadata: DevBinarySourceMetadata,
    activation_channel: String,
    resolved_executable_payload: PathBuf,
) -> RuntimeIdentityProjection {
    RuntimeIdentityProjection {
        version_label: metadata_version_label(&metadata),
        source_fingerprint: Some(metadata.source_fingerprint),
        source_dirty: Some(metadata.dirty),
        source_hash: Some(metadata.short_hash),
        source_full_hash: Some(metadata.full_hash),
        activation_channel,
        resolved_executable_payload,
    }
}

/// Read the source sidecar written next to a published self-dev binary.
pub fn read_dev_binary_source_metadata(binary: &Path) -> Option<DevBinarySourceMetadata> {
    storage::read_json(&binary_source_metadata_path(binary)).ok()
}

pub fn runtime_identity_projection_for_binary(
    binary: &Path,
    activation_channel: impl Into<String>,
) -> RuntimeIdentityProjection {
    let activation_channel = activation_channel.into();
    let resolved_payload = resolve_binary_payload(binary);
    if let Some(metadata) = read_dev_binary_source_metadata(&resolved_payload)
        .or_else(|| read_dev_binary_source_metadata(binary))
    {
        return runtime_identity_projection_from_metadata(
            metadata,
            activation_channel,
            resolved_payload,
        );
    }

    RuntimeIdentityProjection {
        version_label: jcode_build_meta::VERSION.to_string(),
        source_fingerprint: None,
        source_dirty: None,
        source_hash: Some(jcode_build_meta::GIT_HASH.to_string()),
        source_full_hash: None,
        activation_channel,
        resolved_executable_payload: resolved_payload,
    }
}

/// Best-effort R01 canonical projection for the currently running process.
///
/// Release/ambient binaries cannot always reconstruct the build-time dirty source
/// fingerprint, so those fields are optional. Dirty selfdev publication paths
/// should prefer [`SourceState::runtime_identity_projection`] with the exact
/// requested source state.
pub fn current_runtime_identity_projection(
    activation_channel: impl Into<String>,
) -> RuntimeIdentityProjection {
    let current_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("unknown"));
    runtime_identity_projection_for_binary(&current_exe, activation_channel)
}

/// Manifest of recent self-dev builds.
///
/// F20c retired the canary/stable/pending-activation state machine along with
/// the multi-channel version store it coordinated: with one atomic fixed
/// publish target there is no second channel to roll back to, and a failed
/// publish simply leaves the previous binary in place. What remains is the
/// build history the `self_dev status` view reports.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BuildManifest {
    /// History of recent builds, newest first.
    #[serde(default)]
    pub history: Vec<BuildInfo>,
}

impl BuildManifest {
    /// Load manifest from disk.
    ///
    /// F20c moved the manifest out of `~/.jcode/builds/` (which is now the
    /// retired layout that `doctor --clean-retired-layout` deletes). Build
    /// history from the old location is migrated forward once rather than
    /// silently dropped; a failed migration is not fatal, since history is
    /// informational.
    pub fn load() -> Result<Self> {
        let path = manifest_path()?;
        if path.exists() {
            return storage::read_json(&path);
        }

        let legacy = storage_helpers::legacy_manifest_path()?;
        if legacy.exists()
            && let Ok(migrated) = storage::read_json::<Self>(&legacy)
        {
            let _ = migrated.save();
            return Ok(migrated);
        }

        Ok(Self::default())
    }

    /// Save manifest to disk
    pub fn save(&self) -> Result<()> {
        let path = manifest_path()?;
        storage::write_json(&path, self)
    }

    /// Add build to history, keeping the most recent 20.
    pub fn add_to_history(&mut self, info: BuildInfo) -> Result<()> {
        self.history.insert(0, info);
        self.history.truncate(20);
        self.save()
    }
}

/// Atomically publish `source` as the single fixed reload target
/// `~/.jcode/current/jcode` (F20b): stage -> fsync -> smoke -> rename, so a
/// concurrent reader only ever observes a complete, smoke-tested binary.
pub fn publish_current_fixed(source: &Path) -> Result<PathBuf> {
    let dest_dir = paths::current_fixed_dir()?;
    atomic_publish_binary(source, &dest_dir)
}

/// Atomically publish `source` into `dest_dir` as `dest_dir/<binary_name>`,
/// staging into a private temp copy, smoke-testing it, then `rename(2)`-ing it
/// into place. This is the ONE atomic-swap primitive behind the single fixed
/// publish target; the source-truncation regression test guards it. A failed
/// publish leaves the previously published binary untouched and removes its
/// staged temp, so a bad build can never be observed as published.
fn atomic_publish_binary(source: &Path, dest_dir: &Path) -> Result<PathBuf> {
    let source_metadata = std::fs::metadata(source)
        .with_context(|| format!("Binary not found at {}", source.display()))?;
    if !source_metadata.is_file() {
        anyhow::bail!("Binary is not a file at {}", source.display());
    }

    storage::ensure_dir(dest_dir)?;

    let dest = dest_dir.join(binary_name());
    let staged = copy_binary_to_staging_path(source, dest_dir)?;

    let install_result = (|| {
        run_after_install_stage_hook(source, &staged);
        smoke_test_staged_binary_for_install(source, &staged)?;
        publish_staged_binary(&staged, dest_dir, &dest)?;
        Ok(dest.clone())
    })();

    if install_result.is_err() {
        // The destination directory is persistent (it may already hold the last
        // good binary), so a failed publish only drops its own staged temp and
        // lets the previously published binary stand.
        let _ = std::fs::remove_file(&staged);
    }

    install_result
}

fn copy_binary_to_staging_path(source: &Path, dest_dir: &Path) -> Result<PathBuf> {
    for attempt in 0..1000_u32 {
        let staged = staged_binary_path(dest_dir, attempt);
        let staged_file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged);

        let mut staged_file = match staged_file {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("failed to create staged binary {}", staged.display())
                });
            }
        };

        let stage_result = (|| {
            let mut source_file = std::fs::File::open(source)
                .with_context(|| format!("failed to open source binary {}", source.display()))?;
            std::io::copy(&mut source_file, &mut staged_file).with_context(|| {
                format!(
                    "failed to copy {} to staged binary {}",
                    source.display(),
                    staged.display()
                )
            })?;
            crate::platform_support::set_permissions_executable(&staged).with_context(|| {
                format!(
                    "failed to mark staged binary executable: {}",
                    staged.display()
                )
            })?;
            staged_file
                .sync_all()
                .with_context(|| format!("failed to fsync staged binary {}", staged.display()))?;

            let staged_len = staged_file
                .metadata()
                .with_context(|| format!("failed to stat staged binary {}", staged.display()))?
                .len();
            if staged_len == 0 {
                anyhow::bail!(
                    "Refusing to publish zero-byte staged binary {}",
                    staged.display()
                );
            }

            Ok(())
        })();

        if let Err(err) = stage_result {
            let _ = std::fs::remove_file(&staged);
            return Err(err).with_context(|| {
                format!(
                    "failed to prepare staged binary {} from {}",
                    staged.display(),
                    source.display()
                )
            });
        }

        sync_directory_best_effort(dest_dir);
        return Ok(staged);
    }

    anyhow::bail!(
        "failed to create a unique staged binary path in {}",
        dest_dir.display()
    )
}

fn staged_binary_path(dest_dir: &Path, attempt: u32) -> PathBuf {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    dest_dir.join(format!(
        ".{}-publish-{}-{}-{}{}",
        binary_stem(),
        std::process::id(),
        now,
        attempt,
        std::env::consts::EXE_SUFFIX
    ))
}

fn publish_staged_binary(staged: &Path, dest_dir: &Path, dest: &Path) -> Result<()> {
    #[cfg(windows)]
    if dest.exists() {
        std::fs::remove_file(dest)
            .with_context(|| format!("failed to remove existing binary {}", dest.display()))?;
    }

    std::fs::rename(staged, dest).with_context(|| {
        format!(
            "failed to atomically publish {} to {}",
            staged.display(),
            dest.display()
        )
    })?;
    sync_directory_best_effort(dest_dir);
    Ok(())
}

fn smoke_test_staged_binary_for_install(_source: &Path, staged: &Path) -> Result<()> {
    #[cfg(test)]
    if source_is_current_test_exe(_source) {
        return Ok(());
    }

    smoke_test_binary(staged)
}

#[cfg(test)]
fn source_is_current_test_exe(source: &Path) -> bool {
    let Ok(current_exe) = std::env::current_exe() else {
        return false;
    };
    std::fs::canonicalize(source).ok() == std::fs::canonicalize(current_exe).ok()
}

fn sync_directory_best_effort(dir: &Path) {
    if let Ok(file) = std::fs::File::open(dir) {
        let _ = file.sync_all();
    }
}

#[cfg(test)]
type InstallStageHook = Box<dyn FnOnce(&Path, &Path) + Send + 'static>;

#[cfg(test)]
static INSTALL_STAGE_HOOK: std::sync::Mutex<Option<InstallStageHook>> = std::sync::Mutex::new(None);

/// One process-global lock serializing EVERY test that touches shared publish
/// state: the `INSTALL_STAGE_HOOK` static and the `JCODE_HOME` env var. The
/// publish path (`publish_current_fixed`) reads the
/// global hook, and resolution reads `JCODE_HOME`, so any two tests exercising
/// those must not overlap under multithreaded runs. All test modules in this
/// crate acquire this before arming the hook or mutating `JCODE_HOME`.
#[cfg(test)]
pub(crate) fn publish_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
fn set_after_install_stage_hook(hook: impl FnOnce(&Path, &Path) + Send + 'static) {
    *INSTALL_STAGE_HOOK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Box::new(hook));
}

#[cfg(test)]
fn run_after_install_stage_hook(source: &Path, staged: &Path) {
    let hook = INSTALL_STAGE_HOOK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();
    if let Some(hook) = hook {
        hook(source, staged);
    }
}

#[cfg(not(test))]
fn run_after_install_stage_hook(_source: &Path, _staged: &Path) {}

#[cfg(all(test, unix))]
#[path = "atomic_publish_tests.rs"]
mod atomic_publish_tests;

fn binary_source_metadata_path(binary: &Path) -> PathBuf {
    let file_name = binary
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| binary_stem().to_string());
    binary.with_file_name(format!("{file_name}.source.json"))
}

pub fn write_dev_binary_source_metadata(binary: &Path, source: &SourceState) -> Result<PathBuf> {
    let path = binary_source_metadata_path(binary);
    storage::write_json(&path, &DevBinarySourceMetadata::from(source))?;
    Ok(path)
}

pub fn write_current_dev_binary_source_metadata(
    repo_dir: &Path,
    source: &SourceState,
) -> Result<PathBuf> {
    let binary = find_dev_binary(repo_dir)
        .ok_or_else(|| anyhow::anyhow!("Binary not found in target/selfdev or target/release"))?;
    write_dev_binary_source_metadata(&binary, source)
}

fn read_binary_version_report(binary: &Path) -> Result<BinaryVersionReport> {
    let output = Command::new(binary)
        .args(["version", "--json"])
        .env("JCODE_NON_INTERACTIVE", "1")
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "Binary smoke test failed for {} with exit code {:?}: {}",
            binary.display(),
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    serde_json::from_slice(&output.stdout).map_err(|err| {
        anyhow::anyhow!(
            "Binary smoke test for {} returned invalid JSON: {}",
            binary.display(),
            err
        )
    })
}

pub fn smoke_test_binary(binary: &Path) -> Result<()> {
    let report = read_binary_version_report(binary)?;
    if report.version.as_deref().unwrap_or_default().is_empty() {
        anyhow::bail!(
            "Binary smoke test for {} returned JSON without a version field",
            binary.display()
        );
    }
    Ok(())
}

fn validate_binary_version_matches_source_report(
    report: &BinaryVersionReport,
    binary: &Path,
    source: &SourceState,
) -> Result<()> {
    let git_hash = report.git_hash.as_deref().unwrap_or_default();
    if git_hash.is_empty() {
        anyhow::bail!(
            "Binary {} version report did not include git_hash; rebuild before publishing {}",
            binary.display(),
            source.version_label
        );
    }
    if git_hash != source.short_hash {
        anyhow::bail!(
            "Refusing to publish {} as {}: binary was built from git hash {}, but source state is {}",
            binary.display(),
            source.version_label,
            git_hash,
            source.short_hash
        );
    }
    Ok(())
}

fn dirty_status_paths(repo_dir: &Path) -> Result<Vec<(PathBuf, bool)>> {
    let output = Command::new("git")
        .args(["status", "--porcelain=v1", "-z", "--untracked-files=all"])
        .current_dir(repo_dir)
        .output()?;
    if !output.status.success() {
        anyhow::bail!(
            "git status failed while validating dirty build freshness with status {:?}",
            output.status.code()
        );
    }

    let mut entries = output.stdout.split(|byte| *byte == 0).peekable();
    let mut paths = Vec::new();
    while let Some(entry) = entries.next() {
        if entry.is_empty() || entry.len() < 4 {
            continue;
        }
        let x = entry[0];
        let y = entry[1];
        let path = String::from_utf8_lossy(&entry[3..]).to_string();
        let deleted = x == b'D' || y == b'D';
        paths.push((PathBuf::from(path), deleted));

        if matches!(x, b'R' | b'C') || matches!(y, b'R' | b'C') {
            let _ = entries.next();
        }
    }

    Ok(paths)
}

fn validate_dirty_binary_freshness_without_metadata(
    repo_dir: &Path,
    binary: &Path,
    source: &SourceState,
) -> Result<()> {
    if !source.dirty {
        return Ok(());
    }

    let binary_mtime = std::fs::metadata(binary)
        .and_then(|metadata| metadata.modified())
        .map_err(|err| {
            anyhow::anyhow!(
                "Could not read binary modification time for {}: {}",
                binary.display(),
                err
            )
        })?;
    let dirty_paths = dirty_status_paths(repo_dir)?;
    let mut unverifiable = Vec::new();
    let mut newer_than_binary = Vec::new();

    for (relative, deleted) in dirty_paths {
        if deleted {
            unverifiable.push(relative.display().to_string());
            continue;
        }
        let path = repo_dir.join(&relative);
        let modified = match std::fs::metadata(&path).and_then(|metadata| metadata.modified()) {
            Ok(modified) => modified,
            Err(_) => {
                unverifiable.push(relative.display().to_string());
                continue;
            }
        };
        if modified > binary_mtime {
            newer_than_binary.push(relative.display().to_string());
        }
    }

    if !unverifiable.is_empty() {
        anyhow::bail!(
            "Refusing to publish dirty build {} without source metadata: these changed paths cannot be checked against the binary timestamp: {}",
            source.version_label,
            unverifiable.join(", ")
        );
    }
    if !newer_than_binary.is_empty() {
        anyhow::bail!(
            "Refusing to publish stale dirty build {}: changed paths are newer than {}: {}",
            source.version_label,
            binary.display(),
            newer_than_binary.join(", ")
        );
    }

    Ok(())
}

fn validate_dev_binary_source_metadata(binary: &Path, source: &SourceState) -> Result<bool> {
    let path = binary_source_metadata_path(binary);
    if !path.exists() {
        return Ok(false);
    }

    let metadata: DevBinarySourceMetadata = storage::read_json(&path)?;
    if metadata.source_fingerprint != source.fingerprint
        || metadata.version_label != source.version_label
        || metadata.short_hash != source.short_hash
        || metadata.full_hash != source.full_hash
        || metadata.dirty != source.dirty
    {
        anyhow::bail!(
            "Refusing to publish {} as {}: source metadata at {} was for {} ({})",
            binary.display(),
            source.version_label,
            path.display(),
            metadata.version_label,
            metadata.source_fingerprint
        );
    }
    Ok(true)
}

/// True only if `binary` carries source metadata that exactly matches `source`.
/// Missing metadata, a read error, or any field mismatch all return false —
/// a stale-check must never fail a launch on its own; worst case it triggers a
/// rebuild. Used by the self-dev launcher to auto-rebuild a stale binary.
pub fn dev_binary_matches_source(binary: &Path, source: &SourceState) -> bool {
    let path = binary_source_metadata_path(binary);
    let metadata: DevBinarySourceMetadata = match storage::read_json(&path) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };
    metadata.source_fingerprint == source.fingerprint
        && metadata.version_label == source.version_label
        && metadata.short_hash == source.short_hash
        && metadata.full_hash == source.full_hash
        && metadata.dirty == source.dirty
}

fn validate_dev_binary_matches_source(
    repo_dir: &Path,
    binary: &Path,
    source: &SourceState,
) -> Result<()> {
    let report = read_binary_version_report(binary)?;
    if report.version.as_deref().unwrap_or_default().is_empty() {
        anyhow::bail!(
            "Binary smoke test for {} returned JSON without a version field",
            binary.display()
        );
    }
    validate_binary_version_matches_source_report(&report, binary, source)?;
    if !validate_dev_binary_source_metadata(binary, source)? {
        validate_dirty_binary_freshness_without_metadata(repo_dir, binary, source)?;
    }
    Ok(())
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SmokeTestReplyKind {
    Ack,
    Pong,
}

#[cfg(unix)]
fn smoke_test_server_request(
    stream: &mut BufReader<std::os::unix::net::UnixStream>,
    request: &serde_json::Value,
    expected_reply_kind: SmokeTestReplyKind,
    expected_reply_id: u64,
) -> Result<()> {
    let payload = serde_json::to_string(request)? + "\n";
    stream.get_mut().write_all(payload.as_bytes())?;
    stream.get_mut().flush()?;

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let mut line = String::new();
        let bytes = stream.read_line(&mut line)?;
        if bytes == 0 {
            anyhow::bail!(
                "server closed the smoke-test socket before sending {:?} {}",
                expected_reply_kind,
                expected_reply_id
            );
        }
        let value: serde_json::Value = serde_json::from_str(line.trim()).map_err(|err| {
            anyhow::anyhow!("server smoke test returned invalid JSON line: {}", err)
        })?;
        let reply_type = value.get("type").and_then(|t| t.as_str());
        let reply_id = value.get("id").and_then(|id| id.as_u64());
        let kind_matches = match expected_reply_kind {
            SmokeTestReplyKind::Ack => reply_type == Some("ack"),
            SmokeTestReplyKind::Pong => reply_type == Some("pong"),
        };
        if kind_matches && reply_id == Some(expected_reply_id) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            anyhow::bail!(
                "timed out waiting for {:?} {} during server smoke test",
                expected_reply_kind,
                expected_reply_id
            );
        }
    }
}

#[cfg(unix)]
fn smoke_test_server_connect(
    path: &Path,
) -> std::io::Result<BufReader<std::os::unix::net::UnixStream>> {
    let stream = std::os::unix::net::UnixStream::connect(path)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    Ok(BufReader::new(stream))
}

#[cfg(unix)]
fn smoke_test_server_protocol(path: &Path, working_dir: &str) -> Result<()> {
    // The server handles an initial Ping on a dedicated lightweight-control
    // connection and closes it after replying, so the subscribed-client probe
    // must use a fresh socket.
    {
        let mut stream = smoke_test_server_connect(path)?;
        smoke_test_server_request(
            &mut stream,
            &serde_json::json!({
                "type": "ping",
                "id": 1
            }),
            SmokeTestReplyKind::Pong,
            1,
        )?;
    }

    let mut stream = smoke_test_server_connect(path)?;
    smoke_test_server_request(
        &mut stream,
        &serde_json::json!({
            "type": "subscribe",
            "id": 2,
            "working_dir": working_dir
        }),
        SmokeTestReplyKind::Ack,
        2,
    )?;
    Ok(())
}

#[cfg(unix)]
pub fn smoke_test_server_binary(binary: &Path) -> Result<()> {
    use std::fs::File;
    use std::process::Stdio;
    use std::thread;

    smoke_test_binary(binary)?;

    let temp = tempfile::tempdir()?;
    let runtime_dir = temp.path().join("runtime");
    storage::ensure_dir(&runtime_dir)?;
    let socket_path = temp.path().join("jcode-smoke.sock");
    let stderr_path = temp.path().join("jcode-smoke.stderr.log");
    let stderr = File::create(&stderr_path)?;

    let mut child = Command::new(binary)
        .arg("serve")
        .arg("--socket")
        .arg(&socket_path)
        .env("JCODE_NON_INTERACTIVE", "1")
        .env("JCODE_RUNTIME_DIR", &runtime_dir)
        .env("JCODE_GATEWAY_ENABLED", "0")
        .env("JCODE_TEMP_SERVER", "1")
        .env("JCODE_SERVER_OWNER_PID", std::process::id().to_string())
        .env("JCODE_TEMP_SERVER_IDLE_SECS", "300")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr))
        .spawn()?;

    let result = (|| -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait()? {
                let stderr = std::fs::read_to_string(&stderr_path).unwrap_or_default();
                anyhow::bail!(
                    "server smoke test process exited early with status {:?}: {}",
                    status.code(),
                    stderr.trim()
                );
            }

            match smoke_test_server_connect(&socket_path) {
                Ok(_) => {
                    smoke_test_server_protocol(&socket_path, env!("CARGO_MANIFEST_DIR"))?;
                    return Ok(());
                }
                Err(err)
                    if matches!(
                        err.kind(),
                        std::io::ErrorKind::NotFound
                            | std::io::ErrorKind::ConnectionRefused
                            | std::io::ErrorKind::WouldBlock
                    ) =>
                {
                    if Instant::now() >= deadline {
                        let stderr = std::fs::read_to_string(&stderr_path).unwrap_or_default();
                        anyhow::bail!(
                            "timed out waiting for server smoke test socket {}: {}",
                            socket_path.display(),
                            stderr.trim()
                        );
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                Err(err) => return Err(err.into()),
            }
        }
    })();

    let _ = child.kill();
    let shutdown_deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if Instant::now() >= shutdown_deadline {
            let _ = child.kill();
            let _ = child.wait();
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }

    result
}

#[cfg(not(unix))]
pub fn smoke_test_server_binary(binary: &Path) -> Result<()> {
    smoke_test_binary(binary)
}

/// Publish a freshly built self-dev binary onto the single fixed reload target.
///
/// F20b made `~/.jcode/current/jcode` the one atomic publish target; F20c
/// removed the versioned store and the stable/current/shared-server channel
/// symlinks that used to shadow it. The publish sequence is therefore:
/// validate the binary really came from `source`, atomically
/// stage->fsync->smoke->rename it into the fixed path, write the source
/// sidecar next to the published binary, and re-verify the *published* copy
/// reports the expected identity.
pub fn publish_local_current_build_for_source(
    repo_dir: &Path,
    source: &SourceState,
) -> Result<PublishedBuild> {
    let binary = find_dev_binary(repo_dir)
        .ok_or_else(|| anyhow::anyhow!("Binary not found in target/selfdev or target/release"))?;
    if !binary.exists() {
        anyhow::bail!("Binary not found at {:?}", binary);
    }

    validate_dev_binary_matches_source(repo_dir, &binary, source)?;
    let published_path = publish_current_fixed(&binary)?;
    write_dev_binary_source_metadata(&published_path, source)?;
    let installed_report = read_binary_version_report(&published_path)?;
    if installed_report
        .version
        .as_deref()
        .unwrap_or_default()
        .is_empty()
    {
        anyhow::bail!(
            "Binary smoke test for {} returned JSON without a version field",
            published_path.display()
        );
    }
    validate_binary_version_matches_source_report(&installed_report, &published_path, source)?;
    let launcher_link = update_launcher_symlink_to_current()?;

    Ok(PublishedBuild {
        version: source.version_label.clone(),
        source_fingerprint: source.fingerprint.clone(),
        published_path,
        launcher_link,
    })
}

/// Build-state-free convenience wrapper: publish whatever is currently built in
/// `repo_dir` for the repo's current source state.
pub fn publish_local_current_build(repo_dir: &std::path::Path) -> Result<PathBuf> {
    let source = current_source_state(repo_dir)?;
    Ok(publish_local_current_build_for_source(repo_dir, &source)?.published_path)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod fixed_path_resolver_tests;
